//! Google Custom Search Engine, ported from searx/engines/google_cse.py.
// engine-id: google_cse

use std::sync::Arc;

use crate::engine::util::*;
use crate::engine::{
    Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse,
};
use crate::query::{Query, TimeRange};
use crate::results::Template;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["general", "web"],
    shortcut: "goc",
    paging: true,
    time_range_support: true,
    safesearch: true,
    language_support: true,
    weight: 1.0,
    timeout: 4.0,
};

const ENDPOINT: &str = "https://cse.google.com/cse/element/v1";
const DEFAULT_CX: &str = "partner-pub-8993703457585266:4862972284";
const DEFAULT_PAGE_SIZE: i64 = 20;
const CATEGORIES: &[&str] = &["", "image"];

/// Google CSE, the JSONP element endpoint of a custom search engine.
///
/// Upstream fetches the cse_tok from cse.js in its request helper; engines
/// here do no network I/O, so the token comes from the settings.
pub struct GoogleCse {
    meta: EngineMeta,
}

impl GoogleCse {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        if !CATEGORIES.contains(&config.opt_str("google_categ").unwrap_or("")) {
            return Err(format!("invalid google cse category: {CATEGORIES:?}"));
        }
        Ok(Arc::new(Self {
            meta: EngineMeta::from_config(config, DEFAULTS),
        }))
    }
}

fn is_http(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn web_item(item: &serde_json::Value) -> Option<EngineItem> {
    let url = json_str(item, "unescapedUrl");
    let title = json_str(item, "titleNoFormatting");
    if !is_http(&url) || title.is_empty() {
        return None;
    }
    let mut builder = ResultBuilder::new(url, title).content(json_str(item, "contentNoFormatting"));
    let thumbnail = json_str(item, "richSnippet.cseThumbnail.src");
    if is_http(&thumbnail) {
        builder = builder.thumbnail(thumbnail);
    }
    Some(builder.build())
}

fn image_item(item: &serde_json::Value) -> Option<EngineItem> {
    let img_src = json_str(item, "unescapedUrl");
    let url = json_str(item, "originalContextUrl");
    let title = json_str(item, "titleNoFormatting");
    if !is_http(&url) || !is_http(&img_src) || title.is_empty() {
        return None;
    }
    let mut builder = ResultBuilder::new(url, title)
        .template(Template::Images)
        .content(json_str(item, "contentNoFormatting"))
        .image(img_src);
    let thumbnail = json_str(item, "tbUrl");
    if is_http(&thumbnail) {
        builder = builder.thumbnail(thumbnail);
    }
    let width = json_path(item, "width")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let height = json_path(item, "height")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    if width > 0 && height > 0 {
        builder = builder.metadata("resolution", format!("{width}x{height}"));
    }
    let format = json_str(item, "fileFormat");
    if let Some(format) = format.rsplit('/').next().filter(|part| !part.is_empty()) {
        builder = builder.metadata("img_format", format);
    }
    Some(builder.build())
}

impl Engine for GoogleCse {
    fn meta(&self) -> &EngineMeta {
        &self.meta
    }

    fn build_request(&self, query: &Query, pageno: u32) -> Result<EngineRequest, EngineError> {
        let token = self
            .meta
            .opt_str("cse_tok")
            .map(str::trim)
            .filter(|token| !token.is_empty());
        let token =
            token.ok_or_else(|| EngineError::Message("google_cse needs a cse_tok".into()))?;
        let page_size = self
            .meta
            .opt_i64("page_size")
            .filter(|size| *size > 0)
            .unwrap_or(DEFAULT_PAGE_SIZE);
        let language = locale_language(&query.locale);
        let language = if language == "all" {
            "en".to_string()
        } else {
            language
        };
        let safe = match query.safesearch {
            0 => "off",
            1 => "medium",
            _ => "high",
        };
        let mut pairs: Vec<(&str, String)> = vec![
            ("rsz", "filtered_cse".to_string()),
            ("num", page_size.to_string()),
            ("hl", language),
            (
                "cselibv",
                self.meta.opt_str("cselibv").unwrap_or_default().into(),
            ),
            ("cx", self.meta.opt_str("cx").unwrap_or(DEFAULT_CX).into()),
            ("q", query.query_text()),
            ("safe", safe.to_string()),
            ("cse_tok", token.to_string()),
            ("callback", "_".to_string()),
            ("rurl", String::new()),
            (
                "searchtype",
                self.meta.opt_str("google_categ").unwrap_or("").to_string(),
            ),
        ];
        if let Some(seconds) = TimeRange::seconds(query.time_range) {
            let end = chrono::Utc::now();
            let start = end - chrono::Duration::seconds(seconds);
            pairs.push((
                "sort",
                format!("date:r:{}:{}", start.format("%Y%m%d"), end.format("%Y%m%d")),
            ));
        }
        let region = locale_region(&query.locale);
        if region != "all" {
            pairs.push(("gl", region.to_uppercase()));
        }
        if let Some(exp) = self.meta.opt_str("exp").filter(|exp| !exp.is_empty()) {
            pairs.push(("exp", exp.to_string()));
        }
        if pageno > 1 {
            pairs.push(("start", (i64::from(pageno - 1) * page_size).to_string()));
        }
        Ok(
            EngineRequest::get(format!("{ENDPOINT}?{}", query_string(&pairs)))
                .header("Referer", "https://cse.google.com/"),
        )
    }

    fn parse(
        &self,
        response: &EngineResponse,
        _query: &Query,
    ) -> Result<Vec<EngineItem>, EngineError> {
        // The element endpoint answers with a JSONP callback, not plain JSON.
        let body = response.text();
        let Some(jsonp) = body
            .find('{')
            .zip(body.rfind('}'))
            .and_then(|(f, l)| body.get(f..=l))
        else {
            return Ok(Vec::new());
        };
        let json: serde_json::Value = serde_json::from_str(jsonp)
            .map_err(|error| EngineError::Parse(format!("invalid google cse response: {error}")))?;
        if json.get("error").is_some() {
            if json_path(&json, "error.code").and_then(|value| value.as_i64()) == Some(429) {
                return Err(EngineError::TooManyRequests);
            }
            let message = json_str(&json, "error.message");
            return Err(EngineError::Message(format!("google cse: {message}")));
        }
        let Some(items) = json_path(&json, "results").and_then(|value| value.as_array()) else {
            return Ok(Vec::new());
        };
        let image_categ = self.meta.opt_str("google_categ") == Some("image");
        let mut results = Vec::new();
        for item in items {
            let result = if image_categ {
                image_item(item)
            } else {
                web_item(item)
            };
            if let Some(result) = result {
                results.push(result);
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn try_engine(extra: &[(&str, toml::Value)]) -> Result<Arc<GoogleCse>, String> {
        let mut config = EngineConfig::new("google cse", "google_cse", "goc", &["general"]);
        for (key, value) in extra {
            config.extra.insert((*key).to_string(), value.clone());
        }
        GoogleCse::from_config(&config)
    }

    fn query(raw: &str) -> Query {
        Query::parse(raw, &crate::engine::EngineRegistry::default(), &[])
    }

    #[test]
    fn builds_the_element_request() {
        let mut query = query("rust lang");
        query.locale = "ja-JP".to_string();
        query.time_range = Some(TimeRange::Week);
        let engine = try_engine(&[("cse_tok", "tok".into())]).expect("engine");
        let request = engine.build_request(&query, 2).expect("request");
        assert!(request.url.contains("q=rust%20lang") && request.url.contains("cse_tok=tok"));
        assert!(request.url.contains("hl=ja") && request.url.contains("gl=JP"));
        assert!(
            request.url.contains("safe=off")
                && request.url.contains("start=20")
                && request.url.contains("sort=date")
        );
        assert!(matches!(
            try_engine(&[]).expect("engine").build_request(&query, 1),
            Err(EngineError::Message(_))
        ));
        assert!(try_engine(&[("google_categ", "video".into())]).is_err());
    }

    #[test]
    fn parses_the_answers() {
        let body = br##"_({"results": [{"unescapedUrl": "https://www.rust-lang.org/", "titleNoFormatting": "Rust", "contentNoFormatting": "A language.", "richSnippet": {"cseThumbnail": {"src": "https://rust-lang.org/l.png"}}}, {"titleNoFormatting": "no url"}]});"##;
        let response = EngineResponse::new(200, ENDPOINT, body.to_vec());
        let items = try_engine(&[("cse_tok", "tok".into())]).expect("engine");
        let items = items.parse(&response, &query("rust")).expect("items");
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.url, "https://www.rust-lang.org/");
                assert_eq!(result.title, "Rust");
                assert_eq!(result.content, "A language.");
                assert_eq!(
                    result.thumbnail.as_deref(),
                    Some("https://rust-lang.org/l.png")
                );
            }
            _ => panic!("expected a result"),
        }
        assert_eq!(items.len(), 1);
        let image_engine = try_engine(&[("google_categ", "image".into())]).expect("engine");
        let body = br##"_({"results": [{"unescapedUrl": "https://e.com/cat.jpg", "originalContextUrl": "https://e.com/cat", "titleNoFormatting": "A cat", "tbUrl": "https://e.com/cat_t.jpg", "width": 640, "height": 480, "fileFormat": "image/jpeg"}]});"##;
        let response = EngineResponse::new(200, ENDPOINT, body.to_vec());
        let items = image_engine.parse(&response, &query("cat")).expect("items");
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.template, Template::Images);
                assert_eq!(result.url, "https://e.com/cat");
                assert_eq!(result.img_src.as_deref(), Some("https://e.com/cat.jpg"));
                assert_eq!(
                    result.metadata.get("resolution").map(String::as_str),
                    Some("640x480")
                );
                assert_eq!(
                    result.metadata.get("img_format").map(String::as_str),
                    Some("jpeg")
                );
            }
            _ => panic!("expected a result"),
        }
        let body = br##"_({"error": {"code": 429, "message": "quota"}});"##;
        let response = EngineResponse::new(200, ENDPOINT, body.to_vec());
        assert!(matches!(
            image_engine.parse(&response, &query("rust")),
            Err(EngineError::TooManyRequests)
        ));
        let response = EngineResponse::new(200, ENDPOINT, b"not json".to_vec());
        assert!(
            image_engine
                .parse(&response, &query("rust"))
                .expect("items")
                .is_empty()
        );
    }
}
