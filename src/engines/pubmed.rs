//! PubMed engine, ported from searx/engines/pubmed.py.
// engine-id: pubmed

use std::sync::Arc;

use crate::engine::util::*;
use crate::engine::{Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse};
use crate::query::Query;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["science", "scientific publications"],
    shortcut: "pub",
    paging: true,
    time_range_support: false,
    safesearch: false,
    language_support: false,
    weight: 1.0,
    timeout: 4.0,
};

const SEARCH_URL: &str = "https://pubmed.ncbi.nlm.nih.gov";
const ARTICLE_URL: &str = "https://www.ncbi.nlm.nih.gov/pubmed/";
const PAGE_SIZE: u32 = 10;

type Fields = Vec<(String, String)>;

/// PubMed biomedical literature search.
pub struct PubMed {
    meta: EngineMeta,
}

impl PubMed {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self { meta: EngineMeta::from_config(config, DEFAULTS) }))
    }
}

impl Engine for PubMed {
    fn meta(&self) -> &EngineMeta { &self.meta }

    fn build_request(&self, query: &Query, pageno: u32) -> Result<EngineRequest, EngineError> {
        // Upstream calls esearch for the PMIDs and then efetch for the records.
        // The frozen interface allows a single request, so the MEDLINE export
        // of the PubMed website is used: it carries the efetch fields.
        let base = self.meta.base_url.as_deref().unwrap_or(SEARCH_URL).trim_end_matches('/');
        let pairs: Vec<(&str, String)> = vec![
            ("term", query.query_text()),
            ("format", "pubmed".to_string()),
            ("size", PAGE_SIZE.to_string()),
            ("page", pageno.max(1).to_string()),
        ];
        Ok(EngineRequest::get(format!("{base}/?{}", query_string(&pairs))).header("Accept", "text/html"))
    }

    fn parse(&self, response: &EngineResponse, _query: &Query) -> Result<Vec<EngineItem>, EngineError> {
        let html = response.html()?;
        let mut results = Vec::new();
        for block in select_elements(&html, "pre.search-results-chunk")? {
            let text = block.text().collect::<Vec<_>>().join("");
            for fields in split_records(&text) {
                if let Some(item) = build_item(&fields) {
                    results.push(item);
                }
            }
        }
        Ok(results)
    }
}

/// Split the MEDLINE text into records, joining the continuation lines.
fn split_records(text: &str) -> Vec<Fields> {
    let mut records = Vec::new();
    let mut fields: Fields = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(pmid) = line.strip_prefix("PMID- ") {
            if !fields.is_empty() { records.push(std::mem::take(&mut fields)); }
            fields.push(("PMID".to_string(), pmid.trim().to_string()));
        } else if !fields.is_empty() {
            if let Some(field) = field_line(line) {
                fields.push(field);
            } else if line.starts_with("      ") {
                if let Some((_, value)) = fields.last_mut() { value.push(' '); value.push_str(line.trim()); }
            }
        }
    }
    if !fields.is_empty() { records.push(fields); }
    records
}

/// A "TI  - value" line as tag and value, None for anything else.
fn field_line(line: &str) -> Option<(String, String)> {
    let tag = line.get(..4)?.trim();
    if line.get(4..6)? != "- " || tag.is_empty() || !tag.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    Some((tag.to_string(), line[6..].trim().to_string()))
}

fn first<'a>(fields: &'a Fields, tag: &str) -> Option<&'a str> {
    fields.iter().find(|field| field.0.as_str() == tag).map(|field| field.1.as_str())
}

/// Upstream joins the ForeName and LastName of a PubMed author.
fn author_name(value: &str) -> String {
    let (last, first) = value.split_once(", ").unwrap_or((value, ""));
    clean(&format!("{first} {last}"))
}

/// The ELocationID of type doi (the LID or AID line with a doi suffix).
fn doi(fields: &Fields) -> Option<String> {
    let field = fields.iter().find(|field| (field.0.as_str() == "LID" || field.0.as_str() == "AID") && field.1.ends_with(" [doi]"))?;
    Some(field.1.trim_end_matches(" [doi]").to_string())
}

/// The linking ISSN when present, otherwise the first one of the record.
fn issn(fields: &Fields) -> String {
    let mut fallback = String::new();
    for (tag, value) in fields {
        if tag.as_str() != "IS" { continue; }
        let candidate = value.split(" (").next().unwrap_or(value).trim().to_string();
        if value.contains("(Linking)") { return candidate; }
        if fallback.is_empty() { fallback = candidate; }
    }
    fallback
}

/// The accepted date of the PubMed history (PHST line with an accepted tag).
fn accepted_date(fields: &Fields) -> Option<String> {
    let (_, value) = fields.iter().find(|(tag, value)| tag.as_str() == "PHST" && value.contains("[accepted]"))?;
    value.split_whitespace().next().map(|date| date.replace('/', "-")).filter(|date| date.len() == 10)
}

fn build_item(fields: &Fields) -> Option<EngineItem> {
    let pmid = first(fields, "PMID")?;
    let title = first(fields, "TI").unwrap_or_default();
    if title.is_empty() { return None; }
    let authors: Vec<String> = fields.iter().filter(|field| field.0.as_str() == "FAU")
        .map(|field| author_name(field.1.as_str())).filter(|name| !name.is_empty()).collect();
    let journal = first(fields, "JT").or_else(|| first(fields, "TA")).unwrap_or_default();
    let mut builder = ResultBuilder::new(format!("{ARTICLE_URL}{pmid}"), title)
        .content(first(fields, "AB").unwrap_or_default())
        .author(authors.join(", "))
        .metadata("doi", doi(fields).unwrap_or_default())
        .metadata("journal", journal)
        .metadata("issn", issn(fields));
    if let Some(date) = accepted_date(fields) { builder = builder.published_date(date); }
    Some(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r##"<html><body>
<pre class="search-results-chunk">PMID- 33262490
TI  - Why scientists are turning to Rust.
AB  - Rust guarantees memory safety and
      fearlessly prevents data races.
LID - 10.1038/d41586-020-03382-2 [doi]
IS  - 0028-0836 (Linking)
FAU - Perkel, Jeffrey M
FAU - Doe, Jane
JT  - Nature
PHST- 2020/12/03 06:00 [accepted]
PMID- 39747896
TI  - Second paper.
TA  - Science
</pre>
</body></html>"##;

    fn engine() -> Arc<PubMed> {
        PubMed::from_config(&EngineConfig::new("pubmed", "pubmed", "pub", &["science"])).unwrap()
    }

    fn query() -> Query {
        Query::parse("rust", &crate::engine::EngineRegistry::default(), &[])
    }

    #[test]
    fn parses_medline_records() {
        let response = EngineResponse::new(200, SEARCH_URL, FIXTURE.as_bytes().to_vec());
        let items = engine().parse(&response, &query()).unwrap();
        assert_eq!(items.len(), 2);
        let EngineItem::Result(result) = &items[0] else { panic!("expected a result") };
        assert_eq!(result.url, "https://www.ncbi.nlm.nih.gov/pubmed/33262490");
        assert_eq!(result.title, "Why scientists are turning to Rust.");
        assert_eq!(result.content, "Rust guarantees memory safety and fearlessly prevents data races.");
        assert_eq!(result.author.as_deref(), Some("Jeffrey M Perkel, Jane Doe"));
        assert_eq!(result.published_date.as_deref(), Some("2020-12-03"));
        for (key, value) in [("doi", "10.1038/d41586-020-03382-2"), ("journal", "Nature"), ("issn", "0028-0836")] {
            assert_eq!(result.metadata.get(key).map(String::as_str), Some(value));
        }
    }

    #[test]
    fn handles_partial_records_and_empty_pages() {
        let response = EngineResponse::new(200, SEARCH_URL, FIXTURE.as_bytes().to_vec());
        let items = engine().parse(&response, &query()).unwrap();
        let EngineItem::Result(result) = &items[1] else { panic!("expected a result") };
        assert_eq!(result.metadata.get("journal").map(String::as_str), Some("Science"));
        assert_eq!(result.metadata.get("doi"), None);
        let empty = EngineResponse::new(200, SEARCH_URL, b"<html></html>".to_vec());
        assert!(engine().parse(&empty, &query()).unwrap().is_empty());
    }

    #[test]
    fn builds_the_medline_export_url() {
        let request = engine().build_request(&query(), 2).unwrap();
        for part in ["term=rust", "format=pubmed", "size=10", "page=2"] {
            assert!(request.url.contains(part), "missing {part} in {}", request.url);
        }
    }
}
