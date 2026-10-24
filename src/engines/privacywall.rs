//! Privacywall engine, ported from searx/engines/privacywall.py.
//! The general, image and video variants share this engine, like upstream.
// engine-id: privacywall

use std::sync::Arc;

use scraper::ElementRef;

use crate::engine::util::*;
use crate::engine::{
    Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse,
};
use crate::query::Query;
use crate::results::Template;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["general"],
    shortcut: "pw",
    paging: true,
    time_range_support: true,
    safesearch: true,
    language_support: false,
    weight: 1.0,
    timeout: 4.0,
};

const BASE_URL: &str = "https://www.privacywall.org";
const VIDEO_PAGES: [&str; 9] = [
    "CAoQAA", "CBQQAA", "CB4QAA", "CCgQAA", "CDIQAA", "CDwQAA", "CEYQAA", "CFAQAA", "CFoQAA",
];

pub struct Privacywall {
    meta: EngineMeta,
}

impl Privacywall {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            meta: EngineMeta::from_config(config, DEFAULTS),
        }))
    }

    fn base_url(&self) -> &str {
        self.meta
            .base_url
            .as_deref()
            .unwrap_or(BASE_URL)
            .trim_end_matches('/')
    }

    fn category(&self) -> &str {
        self.meta
            .opt_str("privacywall_category")
            .unwrap_or("general")
    }
}

impl Engine for Privacywall {
    fn meta(&self) -> &EngineMeta {
        &self.meta
    }

    fn build_request(&self, query: &Query, pageno: u32) -> Result<EngineRequest, EngineError> {
        let category = self.category();
        if !matches!(category, "general" | "images" | "videos") {
            return Err(EngineError::Message(format!(
                "privacywall: invalid category {category}"
            )));
        }
        if pageno > 10 {
            return Err(EngineError::Message(
                "privacywall: only the first 10 pages exist".into(),
            ));
        }
        let base = self.base_url();
        let mut pairs: Vec<(&str, String)> = vec![
            ("q", query.query_text()),
            (
                "safesearch",
                (if query.safesearch == 0 { "off" } else { "on" }).to_string(),
            ),
        ];
        let region = locale_region(&query.locale);
        if region != "all" {
            pairs.push(("cc", region.to_uppercase()));
        }
        if let Some(range) = query.time_range {
            pairs.push(("time", range.as_str().to_string()));
        }
        if pageno > 1 {
            let page = match category {
                "images" => Some(pageno.to_string()),
                "videos" => video_page(pageno).map(str::to_string),
                _ => None,
            };
            let Some(page) = page else {
                return Err(EngineError::Message(
                    "privacywall: no pagination here".into(),
                ));
            };
            pairs.push(("page", page));
        }
        let path = if category == "general" {
            "search/secure"
        } else {
            category
        };
        Ok(
            EngineRequest::get(format!("{base}/{path}/?{}", query_string(&pairs)))
                .header("Referer", &format!("{base}/")),
        )
    }

    fn parse(
        &self,
        response: &EngineResponse,
        _query: &Query,
    ) -> Result<Vec<EngineItem>, EngineError> {
        let html = parse_html(&response.text());
        let base = self.base_url();
        let category = self.category();
        if !matches!(category, "general" | "images" | "videos") {
            return Err(EngineError::Message(format!(
                "privacywall: invalid category {category}"
            )));
        }
        let pattern = match category {
            "images" => "div#container div[class*='imgcontainer']",
            "videos" => "div[class*='video-container'] > div[class*='video-card']",
            _ => "div#pw-results-main div[class*='result-card']",
        };
        let mut results = Vec::new();
        for element in select_elements(&html, pattern)? {
            let item = match category {
                "images" => image_result(element, base),
                "videos" => video_result(element, base),
                _ => general_result(element, base),
            };
            results.extend(item);
        }
        Ok(results)
    }
}

fn general_result(element: ElementRef<'_>, base: &str) -> Option<EngineItem> {
    let url = http_url(
        &attr_of(element, "a[class*='result-url-anchor']", "href")?,
        base,
    )?;
    let title = text_of(element, "div[class*='result_title']")?;
    let content = text_of(element, "div[class*='result-description']").unwrap_or_default();
    Some(ResultBuilder::new(url, title).content(content).build())
}

fn image_result(element: ElementRef<'_>, base: &str) -> Option<EngineItem> {
    let url = http_url(&attr_of(element, "a", "href")?, base)?;
    let title = attr_of(element, "a", "alt")?;
    let source = text_of(element, "div[class*='image-source-badge']").unwrap_or_default();
    let thumbnail = extract_thumbnail_url(&attr_of(element, "img", "src").unwrap_or_default());
    let builder = ResultBuilder::new(url, title.clone())
        .content(title)
        .template(Template::Images)
        .metadata("source", source);
    let builder = if thumbnail.starts_with("http") {
        builder.image(thumbnail.clone()).thumbnail(thumbnail)
    } else {
        builder
    };
    Some(builder.build())
}

fn video_result(element: ElementRef<'_>, base: &str) -> Option<EngineItem> {
    let url = http_url(&attr_of(element, "a", "href")?, base)?;
    let title = text_of(element, "h2[class*='video-card-title']")?;
    let content = text_of(element, "p").unwrap_or_default();
    let style = attr_of(element, "div[class*='video-img']", "style").unwrap_or_default();
    let mut thumbnail = String::new();
    if let Some(index) = style.find(":url(") {
        thumbnail = extract_thumbnail_url(style[index + 5..].split(')').next().unwrap_or_default());
    }
    let builder = ResultBuilder::new(url, title)
        .content(content)
        .template(Template::Videos);
    let builder = if thumbnail.starts_with("http") {
        builder.thumbnail(thumbnail)
    } else {
        builder
    };
    Some(builder.build())
}

fn http_url(raw: &str, base: &str) -> Option<String> {
    absolute_url(raw, base).filter(|url| url.starts_with("http"))
}

fn extract_thumbnail_url(url: &str) -> String {
    let raw = url
        .find("?id=")
        .map(|index| url[index + 4..].replace('+', " "))
        .unwrap_or_default();
    percent_encoding::percent_decode_str(&raw)
        .decode_utf8_lossy()
        .to_string()
}

fn video_page(pageno: u32) -> Option<&'static str> {
    VIDEO_PAGES.get(pageno.checked_sub(2)? as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::SearchResult;

    const GENERAL: &str = r#"
    <div id="pw-results-main"><div class="result-card">
      <a class="result-url-anchor" href="https://example.com/page">example.com</a>
      <div class="result_title">Example title</div>
      <div class="result-description">An example description.</div>
    </div></div>"#;
    const IMAGES: &str = r#"
    <div id="container"><div class="imgcontainer">
      <a href="https://example.com/photo" alt="A photo"><img src="/videos/video.php?id=https%3A%2F%2Fimg.example%2Fphoto.jpg"></a>
      <div class="image-source-badge">example.com</div>
    </div></div>"#;
    const VIDEOS: &str = r#"
    <div class="video-container"><div class="video-card">
      <a href="https://videos.example/watch/1"><h2 class="video-card-title">A video</h2></a>
      <p>Video description.</p>
      <div class="video-img" style="background-image:url(/videos/video.php?id=https%3A%2F%2Fvideos.example%2Fthumb.jpg)"></div>
    </div></div>"#;
    fn engine(category: &str) -> Arc<Privacywall> {
        let mut config = EngineConfig::new("privacywall", "privacywall", "pw", &["general"]);
        config.extra.insert(
            "privacywall_category".into(),
            toml::Value::String(category.into()),
        );
        Privacywall::from_config(&config).unwrap()
    }

    fn first(category: &str, fixture: &str) -> SearchResult {
        let response = EngineResponse::new(200, BASE_URL, fixture.as_bytes().to_vec());
        let query = Query::parse("rust", &crate::engine::EngineRegistry::default(), &[]);
        let mut items = engine(category).parse(&response, &query).unwrap();
        let EngineItem::Result(result) = items.remove(0) else {
            panic!("expected a result")
        };
        result
    }

    #[test]
    fn parses_each_category() {
        let result = first("general", GENERAL);
        assert_eq!(result.url, "https://example.com/page");
        assert_eq!(result.content, "An example description.");

        let result = first("images", IMAGES);
        assert_eq!(result.url, "https://example.com/photo");
        assert_eq!(result.template, Template::Images);
        assert_eq!(
            result.thumbnail.as_deref(),
            Some("https://img.example/photo.jpg")
        );
        assert_eq!(
            result.metadata.get("source").map(String::as_str),
            Some("example.com")
        );

        let result = first("videos", VIDEOS);
        assert_eq!(result.url, "https://videos.example/watch/1");
        assert_eq!(result.template, Template::Videos);
        assert_eq!(
            result.thumbnail.as_deref(),
            Some("https://videos.example/thumb.jpg")
        );
    }

    #[test]
    fn builds_paged_requests() {
        let query = Query::parse("rust", &crate::engine::EngineRegistry::default(), &[]);
        let general = engine("general").build_request(&query, 1).unwrap();
        assert!(
            general
                .url
                .starts_with("https://www.privacywall.org/search/secure/?")
        );
        assert!(general.url.contains("safesearch=off"));
        assert!(
            engine("images")
                .build_request(&query, 2)
                .unwrap()
                .url
                .contains("page=2")
        );
        assert!(
            engine("videos")
                .build_request(&query, 3)
                .unwrap()
                .url
                .contains("page=CBQQAA")
        );
        assert!(engine("general").build_request(&query, 2).is_err());
    }
}
