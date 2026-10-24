//! Public domain image archive engine, ported from searx/engines/public_domain_image_archive.py.
//!
//! Upstream discovers the Algolia proxy URL by fetching the Astro config; a
//! synchronous request builder cannot do that, so the URL is read from the
//! api_url setting and defaults to the documented search proxy path.
// engine-id: public_domain_image_archive

use std::sync::Arc;

use crate::engine::util::*;
use crate::engine::{
    Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse,
};
use crate::query::Query;
use crate::results::Template;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["images"],
    shortcut: "pdia",
    paging: true,
    time_range_support: false,
    safesearch: false,
    language_support: false,
    weight: 1.0,
    timeout: 4.0,
};

const BASE_URL: &str = "https://pdimagearchive.org";
const THUMBNAIL_SUFFIX: &str = "?fit=max&h=360&w=360";
const PAGE_SIZE: i64 = 20;

/// Public domain image archive, through its Algolia proxy.
pub struct PublicDomainImageArchive {
    meta: EngineMeta,
    page_size: i64,
}

impl PublicDomainImageArchive {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            page_size: config.opt_i64("page_size").unwrap_or(PAGE_SIZE),
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

    fn api_url(&self) -> String {
        match self.meta.opt_str("api_url") {
            Some(url) => url.trim_end_matches('/').to_string(),
            None => format!("{}/search-proxy", self.base_url()),
        }
    }
}

impl Engine for PublicDomainImageArchive {
    fn meta(&self) -> &EngineMeta {
        &self.meta
    }

    fn build_request(&self, query: &Query, pageno: u32) -> Result<EngineRequest, EngineError> {
        let body = serde_json::json!({
            "page": pageno.saturating_sub(1),
            "query": query.query_text(),
            "hitsPerPage": self.page_size,
            "indexName": "prod_all-images",
        });
        Ok(EngineRequest::post(self.api_url())
            .header("Content-Type", "application/json")
            .json_body(body))
    }

    fn parse(
        &self,
        response: &EngineResponse,
        _query: &Query,
    ) -> Result<Vec<EngineItem>, EngineError> {
        if response.status == 403 {
            return Err(EngineError::AccessDenied);
        }
        if response.status != 200 {
            return Err(EngineError::Message(format!(
                "public domain image archive returned HTTP {}",
                response.status
            )));
        }
        let json = response.json()?;
        let Some(hits) = json_path(&json, "results.0.hits").and_then(serde_json::Value::as_array)
        else {
            return Ok(Vec::new());
        };
        let base = self.base_url();
        let mut results = Vec::new();
        for hit in hits {
            let object_id = json_str(hit, "objectID");
            let thumbnail = json_str(hit, "thumbnail");
            let title = json_str(hit, "title");
            if object_id.is_empty() || thumbnail.is_empty() || title.trim().is_empty() {
                continue;
            }
            let image = thumbnail.split('?').next().unwrap_or_default().to_string();
            let mut content = Vec::new();
            let themes = json_str(hit, "themes");
            if !themes.is_empty() {
                content.push(format!("Themes: {themes}"));
            }
            let work = json_str(hit, "encompassingWork");
            if !work.is_empty() {
                content.push(format!("Encompassing work: {work}"));
            }
            let url = clean_url(&format!("{base}/images/{object_id}"));
            if !url.starts_with("http") {
                continue;
            }
            let title = format!(
                "{} by {} {}",
                title.trim(),
                json_str(hit, "artist"),
                json_str(hit, "displayYear")
            );
            results.push(
                ResultBuilder::new(url, title)
                    .content(content.join(" | "))
                    .template(Template::Images)
                    .image(clean_url(&image))
                    .thumbnail(clean_url(&format!("{image}{THUMBNAIL_SUFFIX}")))
                    .build(),
            );
        }
        Ok(results)
    }
}

/// Drop the imgix tracking parameters, like upstream.
fn clean_url(raw: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(raw) else {
        return raw.to_string();
    };
    let kept: Vec<String> = parsed
        .query_pairs()
        .filter(|(key, _)| key != "ixid" && key != "s")
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    if kept.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.set_query(Some(&kept.join("&")));
    }
    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Method;

    const FIXTURE: &str = r#"{"results":[{"hits":[
      {"objectID":"abc123","title":"A print","artist":"Someone","displayYear":"1658",
       "thumbnail":"https://the-public-domain-review.imgix.net/shop/print.jpg?ixid=XYZ&s=abc",
       "themes":"Animals","encompassingWork":"A book"},
      {"objectID":"","title":"Broken","artist":"Nobody","thumbnail":""}
    ]}]}"#;

    fn engine() -> Arc<PublicDomainImageArchive> {
        let mut config = EngineConfig::new(
            "public domain image archive",
            "public_domain_image_archive",
            "pdia",
            &["images"],
        );
        config.extra.insert(
            "api_url".into(),
            toml::Value::String("https://example.test/search-proxy".into()),
        );
        PublicDomainImageArchive::from_config(&config).unwrap()
    }

    #[test]
    fn parses_the_algolia_hits() {
        let response = EngineResponse::new(200, BASE_URL, FIXTURE.as_bytes().to_vec());
        let query = Query::parse("cat", &crate::engine::EngineRegistry::default(), &[]);
        let items = engine().parse(&response, &query).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EngineItem::Result(result) => {
                assert_eq!(result.url, "https://pdimagearchive.org/images/abc123");
                assert_eq!(result.title, "A print by Someone 1658");
                assert_eq!(result.template, Template::Images);
                assert_eq!(
                    result.content,
                    "Themes: Animals | Encompassing work: A book"
                );
                assert_eq!(
                    result.img_src.as_deref(),
                    Some("https://the-public-domain-review.imgix.net/shop/print.jpg")
                );
                assert_eq!(
                    result.thumbnail.as_deref(),
                    Some(
                        "https://the-public-domain-review.imgix.net/shop/print.jpg?fit=max&h=360&w=360"
                    )
                );
            }
            _ => panic!("expected a result"),
        }
    }

    #[test]
    fn posts_the_algolia_query() {
        let query = Query::parse("cat", &crate::engine::EngineRegistry::default(), &[]);
        let request = engine().build_request(&query, 3).unwrap();
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.url, "https://example.test/search-proxy");
        let body = request.json.clone().unwrap();
        assert_eq!(body["page"], serde_json::json!(2));
        assert_eq!(body["query"], serde_json::json!("cat"));
        assert_eq!(body["hitsPerPage"], serde_json::json!(20));
        assert_eq!(body["indexName"], serde_json::json!("prod_all-images"));
    }
}
