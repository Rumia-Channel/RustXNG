//! Exa Search API engine, ported from searx/engines/exaapi.py.
// engine-id: exaapi

use std::sync::Arc;

use crate::engine::util::*;
use crate::engine::{
    Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse,
};
use crate::query::Query;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["general", "web"],
    shortcut: "exa",
    paging: false,
    time_range_support: false,
    safesearch: true,
    language_support: false,
    weight: 1.0,
    timeout: 4.0,
};

const BASE_URL: &str = "https://api.exa.ai/search";
const SEARCH_TYPES: &[&str] = &["fast", "auto", "instant", "deep", "deep-lite", "deep-reasoning"];
const CONTENT_MODES: &[&str] = &["highlights", "text"];
const DEFAULT_RESULTS: i64 = 10;
const DEFAULT_CHARACTERS: i64 = 500;

/// Exa, a search API for AI agents. Requires an api_key.
pub struct ExaApi {
    meta: EngineMeta,
}

/// The publishedDate of a result as ISO 8601, empty when unparsable.
fn iso_date(value: &str) -> String {
    let date = chrono::DateTime::parse_from_rfc3339(value).or_else(|_| {
        chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
            .map(|date| date.and_utc().fixed_offset())
    });
    date.map(|date| date.to_rfc3339()).unwrap_or_default()
}

impl ExaApi {
    /// The checks of the upstream setup() hook.
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        let key = config.api_key.as_deref().or_else(|| config.opt_str("api_key"));
        if key.map(str::trim).unwrap_or_default().is_empty() {
            return Err("exaapi requires an api_key".to_string());
        }
        let results = config.opt_i64("results_per_page").unwrap_or(DEFAULT_RESULTS);
        if !(1..=100).contains(&results) {
            return Err("results_per_page must be between 1 and 100".to_string());
        }
        let search_type = config.opt_str("search_type").unwrap_or("auto");
        if !SEARCH_TYPES.contains(&search_type) {
            return Err(format!("unsupported search_type: {search_type}"));
        }
        let content_mode = config.opt_str("content_mode").unwrap_or("highlights");
        if !CONTENT_MODES.contains(&content_mode) {
            return Err(format!("unsupported content_mode: {content_mode}"));
        }
        if config.opt_i64("content_max_characters").unwrap_or(DEFAULT_CHARACTERS) < 1 {
            return Err("content_max_characters must be at least 1".to_string());
        }
        Ok(Arc::new(Self {
            meta: EngineMeta::from_config(config, DEFAULTS),
        }))
    }

    fn api_key(&self) -> Result<&str, EngineError> {
        let key = self.meta.api_key.as_deref().or_else(|| self.meta.opt_str("api_key"));
        key.map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| EngineError::Message("exaapi requires an api_key".into()))
    }
}

impl Engine for ExaApi {
    fn meta(&self) -> &EngineMeta {
        &self.meta
    }

    fn build_request(&self, query: &Query, _pageno: u32) -> Result<EngineRequest, EngineError> {
        let key = self.api_key()?;
        // Upstream _contents_payload: excerpts by default, page text on demand.
        let max_characters = self.meta.opt_i64("content_max_characters")
            .unwrap_or(DEFAULT_CHARACTERS)
            .max(1);
        let contents = match self.meta.opt_str("content_mode") {
            Some("text") => serde_json::json!({"text": {"maxCharacters": max_characters, "stripLinks": true}}),
            _ => serde_json::json!({"highlights": {"maxCharacters": max_characters}}),
        };
        let mut body = serde_json::json!({
            "query": query.query_text(),
            "type": self.meta.opt_str("search_type").unwrap_or("auto"),
            "numResults": self.meta.opt_i64("results_per_page").unwrap_or(DEFAULT_RESULTS),
            "contents": contents,
        });
        if query.safesearch > 0 {
            body["moderation"] = serde_json::Value::Bool(true);
        }
        // Upstream forwards the region of the SearXNG locale as userLocation.
        let region = locale_region(&query.locale);
        if region != "all" {
            body["userLocation"] = serde_json::Value::String(region.to_uppercase());
        }
        Ok(EngineRequest::post(BASE_URL)
            .header("x-api-key", key)
            .header("Accept", "application/json")
            .json_body(body))
    }

    fn parse(&self, response: &EngineResponse, _query: &Query) -> Result<Vec<EngineItem>, EngineError> {
        let json = response.json()?;
        let Some(items) = json_path(&json, "results").and_then(|value| value.as_array()) else {
            return Ok(Vec::new());
        };
        let text_mode = self.meta.opt_str("content_mode") == Some("text");
        let mut results = Vec::new();
        for item in items {
            let url = json_str(item, "url");
            if !url.starts_with("https://") && !url.starts_with("http://") {
                continue;
            }
            let highlights: Vec<&str> = json_path(item, "highlights")
                .and_then(|value| value.as_array())
                .map(|values| values.iter().filter_map(|value| value.as_str()).collect())
                .unwrap_or_default();
            let excerpt = if text_mode { json_str(item, "text") } else { highlights.join(" ") };
            let title = strip_tags(&json_str(item, "title"));
            let title = if title.is_empty() { url.clone() } else { title };
            let mut builder = ResultBuilder::new(url, title)
                .content(strip_tags(&excerpt))
                .author(json_str(item, "author"))
                .published_date(iso_date(&json_str(item, "publishedDate")));
            let image = json_str(item, "image");
            if image.starts_with("https://") || image.starts_with("http://") {
                builder = builder.thumbnail(image);
            }
            results.push(builder.build());
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn try_engine(extra: &[(&str, toml::Value)]) -> Result<Arc<ExaApi>, String> {
        let mut config = EngineConfig::new("exaapi", "exaapi", "exa", &["general", "web"]);
        config.api_key = Some("KEY".to_string());
        for (key, value) in extra {
            config.extra.insert((*key).to_string(), value.clone());
        }
        ExaApi::from_config(&config)
    }

    fn query(raw: &str) -> Query {
        Query::parse(raw, &crate::engine::EngineRegistry::default(), &[])
    }

    #[test]
    fn validates_the_options() {
        let config = EngineConfig::new("exaapi", "exaapi", "exa", &["general"]);
        assert!(ExaApi::from_config(&config).is_err());
        assert!(try_engine(&[]).is_ok());
        assert!(try_engine(&[("search_type", "slow".into())]).is_err());
        assert!(try_engine(&[("content_mode", "page".into())]).is_err());
    }

    #[test]
    fn builds_the_api_body() {
        let mut query = query("rust async");
        query.locale = "en-US".to_string();
        query.safesearch = 1;
        let engine = try_engine(&[]).expect("engine");
        let request = engine.build_request(&query, 1).expect("request");
        assert_eq!(request.url, BASE_URL);
        assert_eq!(request.method, crate::engine::Method::Post);
        let key = request.headers.get("x-api-key").expect("api key header");
        assert_eq!(key.to_str().expect("header value"), "KEY");
        let body = request.json.as_ref().expect("json body");
        assert_eq!(body["query"].as_str(), Some("rust async"));
        assert_eq!(body["numResults"].as_i64(), Some(10));
        assert_eq!(body["userLocation"].as_str(), Some("US"));
        assert_eq!(body["contents"]["highlights"]["maxCharacters"].as_i64(), Some(500));
    }

    #[test]
    fn parses_the_results() {
        let body = r##"{"results": [
          {"url": "https://exa.ai/", "title": "Exa <b>Search</b>", "highlights": ["A <i>search</i> engine.", "For agents."],
           "image": "https://exa.ai/og.png", "publishedDate": "2024-05-01T10:00:00Z", "author": "Exa"},
          {"title": "no url"}, {"url": "ftp://example.com/file", "title": "not http"}
        ]}"##;
        let response = EngineResponse::new(200, BASE_URL, body.as_bytes().to_vec());
        let items = try_engine(&[]).expect("engine").parse(&response, &query("exa")).expect("items");
        assert_eq!(items.len(), 1);
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.url, "https://exa.ai/");
                assert_eq!(result.title, "Exa Search");
                assert_eq!(result.content, "A search engine. For agents.");
                assert_eq!(result.author.as_deref(), Some("Exa"));
                assert_eq!(result.thumbnail.as_deref(), Some("https://exa.ai/og.png"));
                assert_eq!(result.published_date.as_deref(), Some("2024-05-01T10:00:00+00:00"));
            }
            _ => panic!("expected a result"),
        }
        let empty = EngineResponse::new(200, BASE_URL, b"{}".to_vec());
        let items = try_engine(&[]).expect("engine").parse(&empty, &query("exa"));
        assert!(items.expect("items").is_empty());
    }
}
