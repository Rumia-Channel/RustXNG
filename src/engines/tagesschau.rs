//! Tagesschau engine, ported from searx/engines/tagesschau.py.
//!
//! The ARD search API is used, stories and videos are mapped to the same
//! result fields as upstream.
// engine-id: tagesschau

use std::sync::Arc;

use regex::Regex;

use crate::engine::util::*;
use crate::engine::{
    Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse,
};
use crate::query::Query;
use crate::results::Template;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["general", "news"],
    shortcut: "ts",
    paging: true,
    time_range_support: false,
    safesearch: false,
    language_support: false,
    weight: 1.0,
    timeout: 4.0,
};

const BASE_URL: &str = "https://www.tagesschau.de";
const RESULTS_PER_PAGE: i64 = 10;

/// The Tagesschau, the news program of the ARD.
pub struct Tagesschau {
    meta: EngineMeta,
}

impl Tagesschau {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            meta: EngineMeta::from_config(config, DEFAULTS),
        }))
    }

    fn base(&self) -> &str {
        self.meta
            .base_url
            .as_deref()
            .unwrap_or(BASE_URL)
            .trim_end_matches('/')
    }
}

impl Engine for Tagesschau {
    fn meta(&self) -> &EngineMeta {
        &self.meta
    }

    fn build_request(&self, query: &Query, pageno: u32) -> Result<EngineRequest, EngineError> {
        let page_size = self
            .meta
            .opt_i64("results_per_page")
            .filter(|value| *value > 0)
            .unwrap_or(RESULTS_PER_PAGE);
        let pairs: Vec<(&str, String)> = vec![
            ("searchText", query.query_text()),
            ("pageSize", page_size.to_string()),
            ("resultPage", pageno.saturating_sub(1).to_string()),
        ];
        Ok(EngineRequest::get(format!(
            "{}/api2u/search?{}",
            self.base(),
            query_string(&pairs)
        )))
    }

    fn parse(
        &self,
        response: &EngineResponse,
        _query: &Query,
    ) -> Result<Vec<EngineItem>, EngineError> {
        let json = response.json()?;
        let Some(items) = json_path(&json, "searchResults").and_then(serde_json::Value::as_array)
        else {
            return Ok(Vec::new());
        };
        let use_source_url = self.meta.opt_bool("use_source_url").unwrap_or(true);
        let base = self.base();
        let mut results = Vec::new();
        for item in items {
            let entry = match json_str(item, "type").as_str() {
                "story" | "webview" => story(item, use_source_url),
                "video" => video(item, base),
                _ => None,
            };
            if let Some(entry) = entry {
                results.push(entry);
            }
        }
        Ok(results)
    }
}

/// A story result, upstream's _story.
fn story(item: &serde_json::Value, use_source_url: bool) -> Option<EngineItem> {
    let title = json_str(item, "title");
    let url = if use_source_url {
        json_str(item, "shareURL")
    } else {
        json_str(item, "detailsweb")
    };
    if title.is_empty() || !is_http(&url) {
        return None;
    }
    let mut builder = ResultBuilder::new(url, title).content(json_str(item, "firstSentence"));
    builder = builder.published_date(short_date(&json_str(item, "date")));
    let thumbnail = json_str(item, "teaserImage.imageVariants.16x9-256");
    if is_http(&thumbnail) {
        builder = builder.thumbnail(thumbnail);
    }
    Some(builder.build())
}

/// A video result, upstream's _video.
fn video(item: &serde_json::Value, base: &str) -> Option<EngineItem> {
    let title = video_title(&json_str(item, "title"));
    if title.is_empty() {
        return None;
    }
    let streams = ["h264s", "h264m", "h264l", "h264xl"]
        .iter()
        .map(|key| json_str(item, &format!("streams.{key}")))
        .find(|value| is_http(value));
    let page = format!(
        "{base}/multimedia/video/{}.html",
        json_str(item, "sophoraId")
    );
    let url = streams.clone().unwrap_or(page);
    if !is_http(&url) {
        return None;
    }
    let mut builder = ResultBuilder::new(url, title)
        .content(json_str(item, "firstSentence"))
        .template(Template::Videos)
        .published_date(short_date(&json_str(item, "date")));
    if let Some(stream) = streams {
        builder = builder.metadata("iframe_src", stream);
    }
    let thumbnail = json_str(item, "teaserImage.imageVariants.16x9-256");
    if is_http(&thumbnail) {
        builder = builder.thumbnail(thumbnail);
    }
    Some(builder.build())
}

/// The title cleanup of upstream: drop the _vapp.mxf suffix and the technical
/// APP prefix.
fn video_title(raw: &str) -> String {
    if !raw.contains("_vapp.mxf") {
        return raw.to_string();
    }
    let stripped = raw.replace("_vapp.mxf", "");
    let pattern = match Regex::new(r"APP\d+ (FC-)?") {
        Ok(pattern) => pattern,
        Err(_) => return stripped,
    };
    pattern.replacen(&stripped, 1, "").to_string()
}

/// Upstream slices the ISO date to its first 19 characters.
fn short_date(value: &str) -> String {
    value.chars().take(19).collect()
}

fn is_http(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r##"{"searchResults":[
      {"type":"story","title":"Story title","date":"2024-05-06T10:00:00.000Z","firstSentence":"First sentence.",
       "shareURL":"https://www.tagesschau.de/story-100.html","detailsweb":"https://www.tagesschau.de/web/story-100.html",
       "teaserImage":{"imageVariants":{"16x9-256":"https://www.tagesschau.de/img/story.jpg"}}},
      {"type":"video","title":"APP1 FC- Clip_vapp.mxf","date":"2024-05-06T11:00:00.000Z","firstSentence":"Video sentence.",
       "sophoraId":"abc","streams":{"h264m":"https://media.example.com/clip.mp4"}},
      {"type":"other","title":"Ignored"}
    ]}"##;

    fn engine() -> Arc<Tagesschau> {
        Tagesschau::from_config(&EngineConfig::new(
            "tagesschau",
            "tagesschau",
            "ts",
            &["news"],
        ))
        .unwrap()
    }

    fn query() -> Query {
        Query::parse("news", &crate::engine::EngineRegistry::default(), &[])
    }

    #[test]
    fn parses_stories_and_videos() {
        let response = EngineResponse::new(200, BASE_URL, FIXTURE.as_bytes().to_vec());
        let items = engine().parse(&response, &query()).unwrap();
        assert_eq!(items.len(), 2);
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.title, "Story title");
                assert_eq!(result.url, "https://www.tagesschau.de/story-100.html");
                assert_eq!(result.content, "First sentence.");
                assert_eq!(
                    result.published_date.as_deref(),
                    Some("2024-05-06T10:00:00")
                );
                assert_eq!(
                    result.thumbnail.as_deref(),
                    Some("https://www.tagesschau.de/img/story.jpg")
                );
            }
            _ => panic!("expected a story result"),
        }
        match &items[1] {
            EngineItem::Result(result) => {
                assert_eq!(result.title, "Clip");
                assert_eq!(result.url, "https://media.example.com/clip.mp4");
                assert_eq!(result.template, Template::Videos);
                assert_eq!(
                    result.metadata.get("iframe_src").map(String::as_str),
                    Some("https://media.example.com/clip.mp4")
                );
            }
            _ => panic!("expected a video result"),
        }
    }

    #[test]
    fn source_url_can_be_disabled() {
        let mut config = EngineConfig::new("tagesschau", "tagesschau", "ts", &["news"]);
        config
            .extra
            .insert("use_source_url".to_string(), toml::Value::Boolean(false));
        let engine = Tagesschau::from_config(&config).unwrap();
        let response = EngineResponse::new(200, BASE_URL, FIXTURE.as_bytes().to_vec());
        let items = engine.parse(&response, &query()).unwrap();
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(
                    result.url,
                    "https://www.tagesschau.de/web/story-100.html"
                );
            }
            _ => panic!("expected a story result"),
        }
    }

    #[test]
    fn missing_results_yield_no_items() {
        let response = EngineResponse::new(200, BASE_URL, b"{}".to_vec());
        assert!(engine().parse(&response, &query()).unwrap().is_empty());
    }

    #[test]
    fn builds_the_paged_request() {
        let request = engine().build_request(&query(), 3).unwrap();
        assert_eq!(
            request.url,
            "https://www.tagesschau.de/api2u/search?searchText=news&pageSize=10&resultPage=2"
        );
    }
}
