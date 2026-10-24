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
/// The CX of the blackle.com custom search engine (upstream default).
const DEFAULT_CX: &str = "partner-pub-8993703457585266:4862972284";
const DEFAULT_PAGE_SIZE: i64 = 20;
const CATEGORIES: &[&str] = &["", "image"];

/// Google CSE, the JSONP element endpoint of a custom search engine.
///
/// Upstream fetches the cse_tok from cse.js in a request helper; engines here
/// must not do network I/O, so the token is read from the settings instead.
pub struct GoogleCse {
    meta: EngineMeta,
}

impl GoogleCse {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        let categ = config.opt_str("google_categ").unwrap_or("");
        if !CATEGORIES.contains(&categ) {
            return Err(format!("invalid google cse category: {categ}"));
        }
        Ok(Arc::new(Self {
            meta: EngineMeta::from_config(config, DEFAULTS),
        }))
    }

    fn google_categ(&self) -> &str {
        self.meta.opt_str("google_categ").unwrap_or("")
    }

    fn page_size(&self) -> i64 {
        self.meta
            .opt_i64("page_size")
            .filter(|size| *size > 0)
            .unwrap_or(DEFAULT_PAGE_SIZE)
    }
}

fn is_http(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// The start/end dates of the sort parameter, formatted like upstream.
fn date_range(range: TimeRange) -> (String, String) {
    let days = match range {
        TimeRange::Day => 1,
        TimeRange::Week => 7,
        TimeRange::Month => 30,
        TimeRange::Year => 365,
    };
    let end = chrono::Utc::now();
    let start = end - chrono::Duration::days(days);
    (start.format("%Y%m%d").to_string(), end.format("%Y%m%d").to_string())
}

/// Upstream web_item: a web result of the element endpoint.
fn web_item(item: &serde_json::Value) -> Option<EngineItem> {
    let url = json_str(item, "unescapedUrl");
    let title = json_str(item, "titleNoFormatting");
    if url.is_empty() || title.is_empty() || !is_http(&url) {
        return None;
    }
    let mut builder = ResultBuilder::new(url, title).content(json_str(item, "contentNoFormatting"));
    let thumbnail = json_str(item, "richSnippet.cseThumbnail.src");
    if is_http(&thumbnail) {
        builder = builder.thumbnail(thumbnail);
    }
    Some(builder.build())
}

/// Upstream img_item: an image result (google_categ = image).
fn image_item(item: &serde_json::Value) -> Option<EngineItem> {
    let img_src = json_str(item, "unescapedUrl");
    let url = json_str(item, "originalContextUrl");
    let title = json_str(item, "titleNoFormatting");
    if url.is_empty() || title.is_empty() || !is_http(&url) || !is_http(&img_src) {
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
    let width = json_path(item, "width").and_then(|value| value.as_i64()).unwrap_or(0);
    let height = json_path(item, "height").and_then(|value| value.as_i64()).unwrap_or(0);
    if width > 0 && height > 0 {
        builder = builder.metadata("resolution", format!("{width}x{height}"));
    }
    if let Some(format) = json_str(item, "fileFormat").rsplit('/').next() {
        if !format.is_empty() {
            builder = builder.metadata("img_format", format);
        }
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
            .filter(|token| !token.is_empty())
            .ok_or_else(|| EngineError::Message("google_cse requires a cse_tok".into()))?;
        let page_size = self.page_size();
        let language = locale_language(&query.locale);
        let language = if language == "all" { "en".to_string() } else { language };
        let safe = match query.safesearch {
            0 => "off",
            1 => "medium",
            _ => "high",
        };
        let mut pairs: Vec<(&str, String)> = vec![
            ("rsz", "filtered_cse".to_string()),
            ("num", page_size.to_string()),
            ("hl", language),
            ("cselibv", self.meta.opt_str("cselibv").unwrap_or_default().into()),
            ("cx", self.meta.opt_str("cx").unwrap_or(DEFAULT_CX).into()),
            ("q", query.query_text()),
            ("safe", safe.to_string()),
            ("cse_tok", token.to_string()),
            ("callback", "_".to_string()),
            ("rurl", String::new()),
            ("searchtype", self.google_categ().to_string()),
        ];
        if let Some(range) = query.time_range {
            let (start, end) = date_range(range);
            pairs.push(("sort", format!("date:r:{start}:{end}")));
        }
        let region = locale_region(&query.locale);
        if region != "all" {
            pairs.push(("gl", region.to_uppercase()));
        }
        let exp = self.meta.opt_str("exp").unwrap_or_default();
        if !exp.is_empty() {
            pairs.push(("exp", exp.to_string()));
        }
        let start = i64::from(pageno.saturating_sub(1)) * page_size;
        if start > 0 {
            pairs.push(("start", start.to_string()));
        }
        Ok(EngineRequest::get(format!("{ENDPOINT}?{}", query_string(&pairs)))
            .header("Referer", "https://cse.google.com/")
            .header("Accept", "application/json"))
    }

    fn parse(&self, response: &EngineResponse, _query: &Query) -> Result<Vec<EngineItem>, EngineError> {
        // The element endpoint answers with a JSONP callback, not plain JSON.
        let body = response.text();
        let (Some(first), Some(last)) = (body.find('{'), body.rfind('}')) else {
            return Ok(Vec::new());
        };
        if last <= first {
            return Ok(Vec::new());
        }
        let json: serde_json::Value = serde_json::from_str(&body[first..=last])
            .map_err(|error| EngineError::Parse(format!("invalid google cse response: {error}")))?;
        if json.get("error").is_some() {
            let message = json_str(&json, "error.message");
            let message = if message.is_empty() { "unknown error".to_string() } else { message };
            if json_path(&json, "error.code").and_then(|value| value.as_i64()) == Some(429) {
                return Err(EngineError::TooManyRequests);
            }
            return Err(EngineError::Message(format!("google cse: {message}")));
        }
        let Some(items) = json_path(&json, "results").and_then(|value| value.as_array()) else {
            return Ok(Vec::new());
        };
        let image_categ = self.google_categ() == "image";
        let mut results = Vec::new();
        for item in items {
            let result = if image_categ { image_item(item) } else { web_item(item) };
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
        let mut config = EngineConfig::new("google cse", "google_cse", "goc", &["general", "web"]);
        for (key, value) in extra {
            config.extra.insert((*key).to_string(), value.clone());
        }
        GoogleCse::from_config(&config)
    }

    fn engine() -> Arc<GoogleCse> {
        try_engine(&[("cse_tok", "tok".into())]).expect("google_cse engine")
    }

    fn query(raw: &str) -> Query {
        Query::parse(raw, &crate::engine::EngineRegistry::default(), &[])
    }

    const FIXTURE: &str = r##"/* element endpoint */ _({"results": [
      {"unescapedUrl": "https://www.rust-lang.org/", "titleNoFormatting": "Rust",
       "contentNoFormatting": "A language empowering everyone.",
       "richSnippet": {"cseThumbnail": {"src": "https://www.rust-lang.org/logo.png"}}},
      {"unescapedUrl": "", "titleNoFormatting": "no url"},
      {"unescapedUrl": "ftp://example.com/file", "titleNoFormatting": "not http"}
    ]});"##;

    #[test]
    fn builds_the_element_request() {
        let mut query = query("rust lang");
        query.locale = "ja-JP".to_string();
        query.time_range = Some(TimeRange::Week);
        let request = engine().build_request(&query, 2).expect("request");
        assert!(request.url.starts_with(ENDPOINT));
        assert!(request.url.contains("q=rust%20lang"));
        assert!(request.url.contains("cse_tok=tok"));
        assert!(request.url.contains("cx=partner-pub-8993703457585266"));
        assert!(request.url.contains("num=20"));
        assert!(request.url.contains("hl=ja"));
        assert!(request.url.contains("gl=JP"));
        assert!(request.url.contains("safe=off"));
        assert!(request.url.contains("sort=date%3Ar"));
        assert!(request.url.contains("start=20"));
        assert_eq!(
            request.headers.get("Referer").and_then(|value| value.to_str().ok()),
            Some("https://cse.google.com/")
        );
        assert!(matches!(
            try_engine(&[]).expect("engine").build_request(&query, 1),
            Err(EngineError::Message(_))
        ));
        assert!(try_engine(&[("google_categ", "video".into())]).is_err());
    }

    #[test]
    fn parses_the_jsonp_answer() {
        let response = EngineResponse::new(200, ENDPOINT, FIXTURE.as_bytes().to_vec());
        let items = engine().parse(&response, &query("rust")).expect("items");
        assert_eq!(items.len(), 1);
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.url, "https://www.rust-lang.org/");
                assert_eq!(result.title, "Rust");
                assert_eq!(result.content, "A language empowering everyone.");
                assert_eq!(
                    result.thumbnail.as_deref(),
                    Some("https://www.rust-lang.org/logo.png")
                );
            }
            _ => panic!("expected a result"),
        }
        let empty = EngineResponse::new(200, ENDPOINT, b"not json".to_vec());
        assert!(engine().parse(&empty, &query("rust")).expect("items").is_empty());
    }

    #[test]
    fn parses_the_image_answer() {
        let engine = try_engine(&[("google_categ", "image".into()), ("cse_tok", "tok".into())])
            .expect("engine");
        let body = r##"_({"results": [
          {"unescapedUrl": "https://example.com/cat.jpg", "originalContextUrl": "https://example.com/cat",
           "titleNoFormatting": "A cat", "tbUrl": "https://example.com/cat_t.jpg",
           "width": 640, "height": 480, "fileFormat": "image/jpeg"}
        ]});"##;
        let response = EngineResponse::new(200, ENDPOINT, body.as_bytes().to_vec());
        let items = engine.parse(&response, &query("cat")).expect("items");
        assert_eq!(items.len(), 1);
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.template, Template::Images);
                assert_eq!(result.url, "https://example.com/cat");
                assert_eq!(result.img_src.as_deref(), Some("https://example.com/cat.jpg"));
                assert_eq!(result.thumbnail.as_deref(), Some("https://example.com/cat_t.jpg"));
                assert_eq!(result.metadata.get("resolution").map(String::as_str), Some("640x480"));
                assert_eq!(result.metadata.get("img_format").map(String::as_str), Some("jpeg"));
            }
            _ => panic!("expected a result"),
        }
    }

    #[test]
    fn api_errors_are_reported() {
        let body = r##"_({"error": {"code": 429, "message": "quota"}});"##;
        let response = EngineResponse::new(200, ENDPOINT, body.as_bytes().to_vec());
        assert!(matches!(
            engine().parse(&response, &query("rust")),
            Err(EngineError::TooManyRequests)
        ));
    }
}
