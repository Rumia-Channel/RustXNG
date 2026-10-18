//! The declarative `xpath` engine, ported from `searx/engines/xpath.py`.
//!
//! Upstream defines whole engines in `settings.yml` (`engine: xpath`) by giving
//! a search URL plus XPath expressions for the result fields. This port reads
//! the very same settings keys, so the engine definitions can be imported from
//! upstream's configuration (see `tools/import_engines.py`).

use std::sync::Arc;
use std::time::Duration;

use crate::engine::util::*;
use crate::engine::xpath::{Node, XPath};
use crate::engine::{
    Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse,
};
use crate::query::Query;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["general"],
    shortcut: "",
    paging: false,
    time_range_support: false,
    safesearch: false,
    language_support: false,
    weight: 1.0,
    timeout: 4.0,
};

/// A declaratively configured HTML engine.
pub struct XPathEngine {
    meta: EngineMeta,
    search_url: String,
    results_xpath: Option<XPath>,
    url_xpath: Option<XPath>,
    title_xpath: Option<XPath>,
    content_xpath: Option<XPath>,
    thumbnail_xpath: Option<XPath>,
    suggestion_xpath: Option<XPath>,
    url_prefix: String,
    first_page_num: u32,
    method: String,
    request_body: Option<String>,
    headers: Vec<(String, String)>,
    cookies: Vec<(String, String)>,
    time_range_url: String,
    time_range_map: Vec<(String, String)>,
    safesearch_map: Vec<(String, String)>,
}

impl XPathEngine {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        let search_url = config
            .search_url
            .clone()
            .or_else(|| config.opt_str("search_url").map(str::to_string))
            .ok_or_else(|| format!("xpath engine {:?} needs a search_url", config.name))?;
        let opt_xpath = |key: &str| -> Result<Option<XPath>, String> {
            match config.opt_str(key) {
                Some(expression) if !expression.trim().is_empty() => XPath::parse(expression)
                    .map(Some)
                    .map_err(|error| format!("{:?}: {key}: {error}", config.name)),
                _ => Ok(None),
            }
        };
        let first_page_num = config
            .opt_i64("first_page_num")
            .map(|value| value.max(0) as u32)
            .unwrap_or(1);
        let headers = string_pairs(config.extra.get("headers"));
        let cookies = string_pairs(config.extra.get("cookies"));
        let time_range_map = string_pairs(config.extra.get("time_range_map"));
        let safesearch_map = string_pairs(config.extra.get("safesearch_map"));
        Ok(Arc::new(Self {
            meta: EngineMeta::from_config(config, DEFAULTS),
            search_url,
            results_xpath: opt_xpath("results_xpath")?,
            url_xpath: opt_xpath("url_xpath")?,
            title_xpath: opt_xpath("title_xpath")?,
            content_xpath: opt_xpath("content_xpath")?,
            thumbnail_xpath: opt_xpath("thumbnail_xpath")?,
            suggestion_xpath: opt_xpath("suggestion_xpath")?,
            url_prefix: config.opt_str("url_prefix").unwrap_or_default().to_string(),
            first_page_num,
            method: config.opt_str("method").unwrap_or("GET").to_string(),
            request_body: config.opt_str("request_body").map(str::to_string),
            headers,
            cookies,
            time_range_url: config
                .opt_str("time_range_url")
                .unwrap_or_default()
                .to_string(),
            time_range_map,
            safesearch_map,
        }))
    }

    fn template(&self, query: &Query, pageno: u32) -> String {
        let language = locale_language(&query.locale);
        let pageno_value = if pageno <= 1 {
            self.first_page_num
        } else {
            (pageno - 1) + self.first_page_num
        };
        let time_range = query
            .time_range
            .map(|range| range.as_str())
            .and_then(|range| {
                self.time_range_map
                    .iter()
                    .find(|(key, _)| key == range)
                    .map(|(_, value)| value.clone())
            })
            .unwrap_or_default();
        let safe_search = self
            .safesearch_map
            .iter()
            .find(|(key, _)| key == &query.safesearch.to_string())
            .map(|(_, value)| value.clone())
            .unwrap_or_default();
        let time_range_output = if time_range.is_empty() {
            String::new()
        } else {
            self.time_range_url.replace("{time_range_val}", &time_range)
        };
        let query_encoded = encode(&query.query_text());
        self.search_url
            .replace("{query}", &query_encoded)
            .replace("{pageno}", &pageno_value.to_string())
            .replace("{lang}", &language)
            .replace("{language}", &language)
            .replace("{time_range}", &time_range_output)
            .replace("{safe_search}", &safe_search)
    }
}

impl Engine for XPathEngine {
    fn meta(&self) -> &EngineMeta {
        &self.meta
    }

    fn timeout(&self) -> Duration {
        self.meta.timeout
    }

    fn build_request(&self, query: &Query, pageno: u32) -> Result<EngineRequest, EngineError> {
        let url = self.template(query, pageno);
        let mut request = if self.request_body.is_some() || self.method.eq_ignore_ascii_case("POST")
        {
            EngineRequest::post(url)
        } else {
            EngineRequest::get(url)
        };
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        for (name, value) in &self.cookies {
            request = request.cookie(name, value);
        }
        if let Some(body) = &self.request_body {
            let encoded_query = encode(&query.query_text());
            let body = body.replace("{query}", &encoded_query);
            request = request
                .header("Content-Type", "application/x-www-form-urlencoded")
                .form(
                    body.split('&')
                        .filter_map(|pair| pair.split_once('='))
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                );
        }
        Ok(request)
    }

    fn parse(
        &self,
        response: &EngineResponse,
        _query: &Query,
    ) -> Result<Vec<EngineItem>, EngineError> {
        let html = parse_html(&response.text());
        let base = origin_of(&self.search_url);
        let mut items = Vec::new();

        let containers: Vec<Node<'_>> = match &self.results_xpath {
            Some(expression) => expression.select(&html),
            None => html
                .root_element()
                .descendants()
                .filter_map(scraper::ElementRef::wrap)
                .map(Node::Element)
                .collect(),
        };

        for container in containers {
            let Node::Element(element) = container else {
                continue;
            };
            let evaluate = |expression: &Option<XPath>| -> Option<String> {
                let expression = expression.as_ref()?;
                expression
                    .select_in(element)
                    .into_iter()
                    .map(|node| node.text())
                    .find(|text| !text.is_empty())
            };
            let Some(url) = evaluate(&self.url_xpath).map(|url| {
                let with_prefix = format!("{}{}", self.url_prefix, url);
                absolute_url(&with_prefix, &base).unwrap_or(with_prefix)
            }) else {
                continue;
            };
            let title = evaluate(&self.title_xpath)
                .or_else(|| Some(text(element)))
                .unwrap_or_default();
            if title.is_empty() || url.is_empty() {
                continue;
            }
            let content = evaluate(&self.content_xpath).unwrap_or_default();
            let mut builder = ResultBuilder::new(url, title).content(content);
            if let Some(thumbnail) = evaluate(&self.thumbnail_xpath) {
                if let Some(thumbnail) = absolute_url(&thumbnail, &base) {
                    builder = builder.thumbnail(thumbnail);
                }
            }
            items.push(builder.build());
        }

        if let Some(expression) = &self.suggestion_xpath {
            for node in expression.select(&html) {
                let suggestion = node.text();
                if !suggestion.is_empty() {
                    items.push(EngineItem::Suggestion(suggestion));
                }
            }
        }
        Ok(items)
    }
}

/// Turn a `toml::Value` holding a table into name/value pairs.
fn string_pairs(value: Option<&toml::Value>) -> Vec<(String, String)> {
    match value {
        Some(toml::Value::Table(table)) => table
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect(),
        _ => Vec::new(),
    }
}

/// The scheme and host of an engine URL, used to resolve relative links.
fn origin_of(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => format!(
            "{}://{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or_default()
        ),
        Err(_) => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineRegistry;

    fn engine() -> Arc<XPathEngine> {
        let mut config = EngineConfig::new("lobsters", "xpath", "lob", &["general"]);
        config.search_url =
            Some("https://lobste.rs/search?q={query}&what=stories&order=relevance".into());
        config.extra.insert(
            "results_xpath".into(),
            toml::Value::String("//div[@class=\"story\"]".into()),
        );
        config.extra.insert(
            "title_xpath".into(),
            toml::Value::String("./a[@class=\"u-url\"]".into()),
        );
        config.extra.insert(
            "url_xpath".into(),
            toml::Value::String("./a[@class=\"u-url\"]/@href".into()),
        );
        config.extra.insert(
            "content_xpath".into(),
            toml::Value::String("./div[@class=\"story_text\"]".into()),
        );
        XPathEngine::from_config(&config).unwrap()
    }

    const FIXTURE: &str = r#"<html><body>
      <div class="story">
        <a class="u-url" href="/s/abc">Rust in the kernel</a>
        <div class="story_text">A discussion.</div>
      </div>
      <div class="story">
        <a class="u-url" href="https://example.com/x">Second story</a>
        <div class="story_text">More text.</div>
      </div>
    </body></html>"#;

    #[test]
    fn builds_the_url_from_the_template() {
        let query = Query::parse("rust async", &EngineRegistry::default(), &[]);
        let url = engine().build_request(&query, 1).unwrap().url;
        assert_eq!(
            url,
            "https://lobste.rs/search?q=rust%20async&what=stories&order=relevance"
        );
    }

    #[test]
    fn parses_the_configured_fields() {
        let response = EngineResponse::new(200, "https://lobste.rs/", FIXTURE.as_bytes().to_vec());
        let query = Query::parse("rust", &EngineRegistry::default(), &[]);
        let items = engine().parse(&response, &query).unwrap();
        assert_eq!(items.len(), 2);
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.url, "https://lobste.rs/s/abc");
                assert_eq!(result.title, "Rust in the kernel");
                assert_eq!(result.content, "A discussion.");
            }
            _ => panic!("expected a result"),
        }
        match &items[1] {
            EngineItem::Result(result) => assert_eq!(result.url, "https://example.com/x"),
            _ => panic!("expected a result"),
        }
    }
}
