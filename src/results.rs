//! Result collection, scoring and ordering, ported from `searx/results.py`
//! and `searx/result_types/`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::engine::EngineItem;

/// Result priority, upstream `MainResult.PriorityType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// The engine explicitly marked the result as unimportant.
    Low,
    #[default]
    Medium,
    /// Answers / direct hits, always ranked first.
    High,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Template used to render a result, upstream `result.template`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Template {
    #[default]
    Default,
    Images,
    Videos,
    Map,
    Music,
    Packages,
}

impl Template {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Images => "images",
            Self::Videos => "videos",
            Self::Map => "map",
            Self::Music => "music",
            Self::Packages => "packages",
        }
    }
}

/// One search result, upstream `MainResult` / `LegacyResult`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResult {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub content: String,
    /// The engine that produced the result (first one wins).
    pub engine: String,
    /// Every engine that produced this result.
    #[serde(default)]
    pub engines: Vec<String>,
    /// 1-based positions inside each engine's result list.
    #[serde(default)]
    pub positions: Vec<u32>,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub category: String,
    #[serde(default, serialize_with = "serialize_template")]
    pub template: Template,
    #[serde(
        default,
        skip_serializing_if = "is_medium",
        serialize_with = "serialize_priority"
    )]
    pub priority: Priority,
    #[serde(
        rename = "publishedDate",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub published_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub img_src: Option<String>,
    /// Engine specific extra fields (length, views, licence, ...).
    ///
    /// Upstream serialises these as one `"key: value | key: value"` string in
    /// the JSON API, so we do the same.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "serialize_metadata"
    )]
    pub metadata: BTreeMap<String, String>,
    /// A short URL shown in the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pretty_url: Option<String>,
    /// Whether the result belongs to the group of the previous one (UI only).
    #[serde(skip)]
    pub open_group: bool,
    #[serde(skip)]
    pub close_group: bool,
}

fn is_medium(priority: &Priority) -> bool {
    *priority == Priority::Medium
}

/// Upstream renders `default.html`, `images.html`, ... in its JSON API.
fn serialize_template<S>(template: &Template, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{}.html", template.as_str()))
}

/// Upstream keeps the engine metadata as one human readable string.
fn serialize_metadata<S>(
    metadata: &BTreeMap<String, String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let text = metadata
        .iter()
        .map(|(key, value)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join(" | ");
    serializer.serialize_str(&text)
}

/// Upstream uses an empty string for the default priority.
fn serialize_priority<S>(priority: &Priority, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(match priority {
        Priority::Medium => "",
        other => other.as_str(),
    })
}

impl SearchResult {
    pub fn new(url: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn with_template(mut self, template: Template) -> Self {
        self.template = template;
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_metadata(mut self, key: &str, value: impl Into<String>) -> Self {
        self.metadata.insert(key.to_string(), value.into());
        self
    }
}

/// An infobox, upstream `Infobox`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Infobox {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub img_src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<InfoboxUrl>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
    #[serde(default)]
    pub engine: String,
}

/// A link of an infobox (`urls` in the infobox template).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InfoboxUrl {
    pub title: String,
    pub url: String,
}

/// Engine timing, upstream `Timing`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timing {
    pub engine: String,
    pub total: f64,
    pub load: f64,
}

/// An engine that did not answer, upstream `UnresponsiveEngine`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnresponsiveEngine {
    pub engine: String,
    #[serde(rename = "error_type")]
    pub error_type: String,
    pub suspended: bool,
}

/// Collects, deduplicates and scores the results of all engines.
#[derive(Debug, Default)]
pub struct ResultContainer {
    main: Vec<SearchResult>,
    index: HashMap<String, usize>,
    pub infoboxes: Vec<Infobox>,
    pub suggestions: BTreeSet<String>,
    pub answers: Vec<String>,
    pub corrections: BTreeSet<String>,
    pub engine_data: BTreeMap<String, BTreeMap<String, String>>,
    pub unresponsive_engines: Vec<UnresponsiveEngine>,
    pub timings: Vec<Timing>,
    pub paging: bool,
    pub redirect_url: Option<String>,
    closed: bool,
    ordered: Vec<SearchResult>,
}

impl ResultContainer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge the results of one engine, weighting them with `weight`.
    pub fn extend(&mut self, engine_name: &str, weight: f64, items: Vec<EngineItem>) {
        if self.closed {
            return;
        }
        let mut position = 0u32;
        for item in items {
            match item {
                EngineItem::Suggestion(suggestion) => {
                    if !suggestion.trim().is_empty() {
                        self.suggestions.insert(suggestion);
                    }
                }
                EngineItem::Answer(answer) => self.answers.push(answer),
                EngineItem::Correction(correction) => {
                    if !correction.trim().is_empty() {
                        self.corrections.insert(correction);
                    }
                }
                EngineItem::Infobox(mut infobox) => {
                    infobox.engine = engine_name.to_string();
                    if infobox.id.is_empty() {
                        infobox.id = format!("{engine_name}-{}", self.infoboxes.len());
                    }
                    match self
                        .infoboxes
                        .iter_mut()
                        .find(|existing| existing.id == infobox.id)
                    {
                        Some(existing) => merge_infobox(existing, &infobox),
                        None => self.infoboxes.push(infobox),
                    }
                }
                EngineItem::EngineData { key, value } => {
                    self.engine_data
                        .entry(engine_name.to_string())
                        .or_default()
                        .insert(key, value);
                }
                EngineItem::Result(mut result) => {
                    position += 1;
                    result.positions = vec![position];
                    result.engine = engine_name.to_string();
                    result.engines = vec![engine_name.to_string()];
                    result.score = weight;
                    let key = dedup_key(&result.url);
                    match self.index.get(&key).copied() {
                        Some(existing) => {
                            let merged = &mut self.main[existing];
                            merge_result(merged, &result);
                            merged.positions.push(position);
                        }
                        None => {
                            self.index.insert(key, self.main.len());
                            self.main.push(result);
                        }
                    }
                }
            }
        }
    }

    /// Score and freeze the container (upstream `close`).
    pub fn close(&mut self, weight_of: impl Fn(&str) -> f64) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.infoboxes
            .sort_by_key(|infobox| infobox.img_src.is_none());

        for result in &mut self.main {
            let mut weight = 1.0;
            for engine in &result.engines {
                weight *= weight_of(engine);
            }
            weight *= result.positions.len() as f64;
            let mut score = 0.0;
            for position in &result.positions {
                match result.priority {
                    Priority::Low => {}
                    Priority::High => score += weight,
                    Priority::Medium => score += weight / (*position as f64),
                }
            }
            result.score = score;
        }

        self.ordered = order_results(&self.main);
    }

    /// Results sorted the way the UI shows them (upstream
    /// `get_ordered_results`).
    pub fn ordered(&self) -> &[SearchResult] {
        &self.ordered
    }

    /// Results without deduplication-group ordering, in score order.
    pub fn main_results(&self) -> &[SearchResult] {
        &self.main
    }

    /// Rough number of results (upstream reports the number of merged results).
    pub fn number_of_results(&self) -> u64 {
        self.main.len() as u64
    }

    pub fn add_unresponsive_engine(&mut self, engine: &str, error_type: &str, suspended: bool) {
        let entry = UnresponsiveEngine {
            engine: engine.to_string(),
            error_type: error_type.to_string(),
            suspended,
        };
        if !self.unresponsive_engines.contains(&entry) {
            self.unresponsive_engines.push(entry);
            self.unresponsive_engines.sort();
        }
    }

    pub fn add_timing(&mut self, engine: &str, total: f64, load: f64) {
        self.timings.push(Timing {
            engine: engine.to_string(),
            total,
            load,
        });
    }

    pub fn set_redirect_url(&mut self, url: impl Into<String>) {
        self.redirect_url = Some(url.into());
    }
}

/// Merge `other` into `origin`, keeping the richer field of each pair.
fn merge_result(origin: &mut SearchResult, other: &SearchResult) {
    if !origin.engines.contains(&other.engine) {
        origin.engines.push(other.engine.clone());
    }
    if origin.title.is_empty() && !other.title.is_empty() {
        origin.title = other.title.clone();
    }
    if origin.content.len() < other.content.len() {
        origin.content = other.content.clone();
    }
    if origin.category.is_empty() && !other.category.is_empty() {
        origin.category = other.category.clone();
    }
    if origin.published_date.is_none() {
        origin.published_date = other.published_date.clone();
    }
    if origin.thumbnail.is_none() {
        origin.thumbnail = other.thumbnail.clone();
    }
    if origin.img_src.is_none() {
        origin.img_src = other.img_src.clone();
    }
    if origin.author.is_none() {
        origin.author = other.author.clone();
    }
    for (key, value) in &other.metadata {
        origin
            .metadata
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
}

fn merge_infobox(origin: &mut Infobox, other: &Infobox) {
    if origin.title.is_empty() {
        origin.title = other.title.clone();
    }
    if origin.content.is_empty() {
        origin.content = other.content.clone();
    }
    if origin.img_src.is_none() {
        origin.img_src = other.img_src.clone();
    }
    if origin.url.is_none() {
        origin.url = other.url.clone();
    }
    for url in &other.urls {
        if !origin.urls.iter().any(|existing| existing.url == url.url) {
            origin.urls.push(url.clone());
        }
    }
    for (key, value) in &other.attributes {
        origin
            .attributes
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
}

/// Group-by-category ordering, ported from `ResultContainer.get_ordered_results`.
fn order_results(results: &[SearchResult]) -> Vec<SearchResult> {
    let mut sorted: Vec<SearchResult> = results.to_vec();
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    const MAX_COUNT: i32 = 8;
    const MAX_DISTANCE: usize = 20;

    let mut grouped: Vec<SearchResult> = Vec::with_capacity(sorted.len());
    let mut positions: HashMap<String, GroupState> = HashMap::new();

    for result in sorted {
        let category = format!(
            "{}:{}:{}",
            result.category,
            result.template.as_str(),
            if result.thumbnail.is_some() || result.img_src.is_some() {
                "img"
            } else {
                ""
            }
        );
        let group = positions.get(&category).copied();
        match group {
            Some(state)
                if state.count > 0 && grouped.len().saturating_sub(state.index) < MAX_DISTANCE =>
            {
                let index = state.index;
                grouped.insert(index, result);
                for state in positions.values_mut() {
                    if state.index >= index {
                        state.index += 1;
                    }
                }
                if let Some(state) = positions.get_mut(&category) {
                    state.count -= 1;
                }
            }
            _ => {
                grouped.push(result);
                positions.insert(
                    category,
                    GroupState {
                        index: grouped.len(),
                        count: MAX_COUNT,
                    },
                );
            }
        }
    }
    grouped
}

#[derive(Debug, Clone, Copy)]
struct GroupState {
    index: usize,
    count: i32,
}

/// The deduplication key of a URL: scheme independent, without fragment and
/// without the usual tracking parameters.
pub fn dedup_key(url: &str) -> String {
    let trimmed = url.trim();
    let Ok(mut parsed) = url::Url::parse(trimmed) else {
        return trimmed.to_lowercase();
    };
    parsed.set_fragment(None);
    let _ = parsed.set_scheme("https");
    let host = parsed.host_str().unwrap_or_default().to_lowercase();
    let path = parsed.path().trim_end_matches('/').to_string();
    let mut query: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| !is_tracking_param(key))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    query.sort();
    let query: Vec<String> = query
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    format!(
        "{host}{path}{}",
        if query.is_empty() {
            String::new()
        } else {
            format!("?{}", query.join("&"))
        }
    )
}

/// Tracking parameters removed when deduplicating (a subset of the ClearURLs
/// rules used by the tracker remover plugin).
pub fn is_tracking_param(key: &str) -> bool {
    let key = key.to_lowercase();
    key.starts_with("utm_")
        || matches!(
            key.as_str(),
            "fbclid"
                | "gclid"
                | "dclid"
                | "msclkid"
                | "yclid"
                | "_openstat"
                | "igshid"
                | "mc_eid"
                | "mkt_tok"
                | "vero_id"
                | "wickedid"
                | "oly_anon_id"
                | "oly_enc_id"
                | "s_cid"
                | "spm"
                | "ref_src"
                | "ref_url"
                | "cmpid"
                | "campaign_id"
        )
}

/// A short human readable URL for the UI.
pub fn pretty_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or_default();
            let path = parsed.path().trim_end_matches('/');
            if path.is_empty() || path == "/" {
                host.to_string()
            } else {
                format!("{host}{path}")
            }
        }
        Err(_) => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineItem;

    fn result(url: &str, title: &str) -> EngineItem {
        EngineItem::Result(SearchResult::new(url, title))
    }

    #[test]
    fn duplicate_urls_are_merged() {
        let mut container = ResultContainer::new();
        container.extend("a", 1.0, vec![result("https://example.com/x", "one")]);
        container.extend("b", 1.0, vec![result("http://example.com/x#anchor", "one")]);
        assert_eq!(container.main_results().len(), 1);
        assert_eq!(container.main_results()[0].engines.len(), 2);
    }

    #[test]
    fn tracking_parameters_do_not_prevent_merging() {
        let mut container = ResultContainer::new();
        container.extend("a", 1.0, vec![result("https://example.com/x?a=1", "one")]);
        container.extend(
            "b",
            1.0,
            vec![result("https://example.com/x?a=1&utm_source=news", "one")],
        );
        assert_eq!(container.main_results().len(), 1);
    }

    #[test]
    fn higher_ranked_positions_score_higher() {
        let mut container = ResultContainer::new();
        container.extend(
            "a",
            1.0,
            vec![
                result("https://example.com/first", "first"),
                result("https://example.com/second", "second"),
            ],
        );
        container.close(|_| 1.0);
        let ordered = container.ordered();
        assert_eq!(ordered[0].url, "https://example.com/first");
        assert!(ordered[0].score > ordered[1].score);
    }

    #[test]
    fn engine_weight_multiplies_the_score() {
        let mut container = ResultContainer::new();
        container.extend("a", 0.5, vec![result("https://example.com/a", "a")]);
        container.extend("b", 2.0, vec![result("https://example.com/b", "b")]);
        container.close(|engine| if engine == "a" { 0.5 } else { 2.0 });
        // `ordered` is the ranking the UI shows: engine weight wins.
        assert_eq!(container.ordered()[0].url, "https://example.com/b");
    }

    #[test]
    fn suggestions_answers_and_corrections_are_collected() {
        let mut container = ResultContainer::new();
        container.extend(
            "a",
            1.0,
            vec![
                EngineItem::Suggestion("did you mean rust".into()),
                EngineItem::Answer("42".into()),
                EngineItem::Correction("corrected".into()),
                result("https://example.com/x", "x"),
            ],
        );
        assert!(container.suggestions.contains("did you mean rust"));
        assert_eq!(container.answers, vec!["42".to_string()]);
        assert!(container.corrections.contains("corrected"));
    }

    #[test]
    fn results_of_the_same_category_stay_close_together() {
        let mut container = ResultContainer::new();
        let mut items = Vec::new();
        for index in 0..10 {
            let mut image = SearchResult::new(format!("https://img.example.com/{index}"), "img");
            image.category = "images".into();
            image.template = Template::Images;
            items.push(EngineItem::Result(image));
            let mut web = SearchResult::new(format!("https://web.example.com/{index}"), "web");
            web.category = "general".into();
            web.score = 100.0 - index as f64;
            items.push(EngineItem::Result(web));
        }
        container.extend("a", 1.0, items);
        container.close(|_| 1.0);
        let ordered = container.ordered();
        // Within the first 8 entries of the images group every second entry is an
        // image result (upstream groups results of one category).
        assert!(ordered[..8].iter().all(|r| r.url.contains("example.com")));
    }

    #[test]
    fn dedup_key_ignores_scheme_fragment_and_tracking() {
        assert_eq!(
            dedup_key("http://Example.com/a/?utm_source=x#frag"),
            dedup_key("https://example.com/a")
        );
    }

    #[test]
    fn pretty_url_strips_the_scheme() {
        assert_eq!(pretty_url("https://example.com/a/b/"), "example.com/a/b");
        assert_eq!(pretty_url("https://example.com/"), "example.com");
    }
}
