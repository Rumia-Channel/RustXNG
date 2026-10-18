//! Search orchestration, ported from `searx/search/processors/online.py` and
//! `searx/search/__init__.py`: engine selection, concurrent execution,
//! per engine timeouts, suspensions, caching and result merging.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::cache::ResponseCache;
use crate::engine::state::{EngineSnapshot, EngineStates};
use crate::engine::{EngineHandle, EngineItem, EngineRegistry, FailureKind};
use crate::metrics::Metrics;
use crate::network::Network;
use crate::plugins::{PluginContext, PluginManager, UserInfo};
use crate::query::{Query, TimeRange};
use crate::results::{Infobox, ResultContainer, SearchResult, Timing, UnresponsiveEngine};
use crate::settings::Settings;

/// Everything the searcher needs to know about one search request.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: Query,
    pub pageno: u32,
    pub categories: Vec<String>,
    pub engines: Vec<String>,
    pub disabled_engines: Vec<String>,
    pub time_range: Option<TimeRange>,
    pub safesearch: u8,
    pub language: String,
    pub timeout_limit: Option<f64>,
    pub enabled_plugins: Vec<String>,
    pub disabled_plugins: Vec<String>,
    pub locale: String,
    pub user: UserInfo,
}

impl SearchRequest {
    /// A request with the defaults for a query, used by the API and tests.
    pub fn new(query: Query) -> Self {
        Self {
            pageno: 1,
            categories: query.selected_categories(),
            engines: Vec::new(),
            disabled_engines: query.disabled_engines.clone(),
            time_range: None,
            safesearch: query.safesearch,
            language: query.engine_language(),
            timeout_limit: query.timeout_limit,
            enabled_plugins: Vec::new(),
            disabled_plugins: Vec::new(),
            locale: query.locale.clone(),
            user: UserInfo::default(),
            query,
        }
    }
}

/// The result of a search, mirroring the fields of upstream's JSON response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub search_query: String,
    pub number_of_results: u64,
    pub results: Vec<SearchResult>,
    /// Upstream's JSON API emits an array of `{"answer": ...}` objects.
    #[serde(serialize_with = "serialize_answers")]
    pub answers: Vec<String>,
    pub corrections: Vec<String>,
    pub infoboxes: Vec<Infobox>,
    pub suggestions: Vec<String>,
    /// Upstream's JSON API emits `[engine, translated error]` pairs.
    #[serde(serialize_with = "serialize_unresponsive")]
    pub unresponsive_engines: Vec<UnresponsiveEngine>,
    pub timings: Vec<Timing>,
    pub redirect_url: Option<String>,
    pub engine_data: BTreeMap<String, BTreeMap<String, String>>,
    pub engines: Vec<String>,
    pub pageno: u32,
    pub paging: bool,
    /// Set when the request itself could not be served (no engine, ...).
    pub error: Option<String>,
}

impl SearchResponse {
    pub fn new(query: &Query) -> Self {
        Self {
            query: query.full_query(),
            search_query: query.query_text(),
            number_of_results: 0,
            results: Vec::new(),
            answers: Vec::new(),
            corrections: Vec::new(),
            infoboxes: Vec::new(),
            suggestions: Vec::new(),
            unresponsive_engines: Vec::new(),
            timings: Vec::new(),
            redirect_url: None,
            engine_data: BTreeMap::new(),
            engines: Vec::new(),
            pageno: 1,
            paging: false,
            error: None,
        }
    }
}

/// The outcome of one engine run.
struct EngineOutcome {
    engine: String,
    weight: f64,
    items: Result<Vec<EngineItem>, FailureKind>,
    elapsed: Duration,
    suspended: bool,
}

/// Runs searches against the configured engines.
pub struct Searcher {
    pub settings: Arc<Settings>,
    pub registry: Arc<EngineRegistry>,
    pub network: Arc<Network>,
    pub cache: Arc<ResponseCache>,
    pub states: Arc<EngineStates>,
    pub metrics: Arc<Metrics>,
    pub plugins: Arc<PluginManager>,
}

impl Searcher {
    pub fn new(settings: Settings) -> Result<Self, String> {
        let registry = EngineRegistry::from_settings(&settings)?;
        let network = Network::from_settings(&settings).map_err(|error| error.to_string())?;
        Ok(Self {
            plugins: Arc::new(PluginManager::from_settings(&settings)),
            settings: Arc::new(settings),
            registry: Arc::new(registry),
            network: Arc::new(network),
            cache: Arc::new(ResponseCache::default()),
            states: Arc::new(EngineStates::new()),
            metrics: Arc::new(Metrics::new()),
        })
    }

    /// Engines that answer a request, in registry order.
    pub fn select_engines(&self, request: &SearchRequest) -> Vec<EngineHandle> {
        let explicit: Vec<String> = if !request.engines.is_empty() {
            request.engines.clone()
        } else {
            request.query.engine_ref_names()
        };
        let wanted_categories: Vec<String> = if !request.categories.is_empty() {
            request.categories.clone()
        } else {
            vec![crate::categories::GENERAL.to_string()]
        };

        let mut selected = Vec::new();
        for engine in self.registry.iter() {
            let name = engine.name().to_string();
            if request.disabled_engines.contains(&name) {
                continue;
            }
            if !explicit.is_empty() {
                if explicit.contains(&name) {
                    selected.push(engine.clone());
                }
                continue;
            }
            let matches = engine.categories().iter().any(|category| {
                let normalised = crate::categories::normalize(category);
                wanted_categories
                    .iter()
                    .any(|wanted| crate::categories::normalize(wanted) == normalised)
            });
            if matches {
                selected.push(engine.clone());
            }
        }
        selected
    }

    /// Run a search: this is the entry point used by the web layer.
    pub async fn search(&self, mut request: SearchRequest) -> SearchResponse {
        Metrics::incr(&self.metrics.searches, 1);
        let context = PluginContext {
            search_query: request.query.query_text(),
            locale: if request.locale.is_empty() {
                self.settings.search.default_lang.clone()
            } else {
                request.locale.clone()
            },
            base_url: self.settings.base_url(),
            user: request.user.clone(),
        };
        self.plugins.pre_search(
            &mut request.query,
            &context,
            &request.enabled_plugins,
            &request.disabled_plugins,
        );

        let mut response = SearchResponse::new(&request.query);
        response.pageno = request.pageno;

        // External bang: hand the query over to another service.
        if let Some(bang) = request.query.external_bang.clone() {
            if let Some(definition) = crate::external_bangs::lookup(&bang) {
                response.redirect_url = Some(crate::external_bangs::expand_url(
                    definition.url,
                    &request.query.query_text(),
                ));
                response.engines = Vec::new();
                return response;
            }
        }

        let engines = self.select_engines(&request);
        if engines.is_empty() {
            response.error = Some("no engine selected for this request".to_string());
            return response;
        }

        let outcomes = futures::future::join_all(
            engines
                .iter()
                .map(|engine| self.run_engine(engine, &request)),
        )
        .await;

        let mut container = ResultContainer::new();
        for outcome in outcomes {
            response.engines.push(outcome.engine.clone());
            match outcome.items {
                Ok(items) => container.extend(&outcome.engine, outcome.weight, items),
                Err(kind) => {
                    container.add_unresponsive_engine(
                        &outcome.engine,
                        kind.error_type(),
                        outcome.suspended,
                    );
                }
            }
            container.add_timing(&outcome.engine, outcome.elapsed.as_secs_f64(), 0.0);
        }

        container.close(|engine| self.registry.weight_of(engine));

        let mut results = container.ordered().to_vec();
        for result in &mut results {
            self.plugins.on_result(
                result,
                &context,
                &request.enabled_plugins,
                &request.disabled_plugins,
            );
        }

        response.number_of_results = container.number_of_results();
        response.paging = container.paging;
        response.answers = container.answers.clone();
        response.corrections = container.corrections.iter().cloned().collect();
        response.infoboxes = container.infoboxes.clone();
        response.suggestions = container.suggestions.iter().cloned().collect();
        response.unresponsive_engines = container.unresponsive_engines.clone();
        response.timings = container.timings.clone();
        response.engine_data = container.engine_data.clone();
        response.results = results;

        if request.query.redirect_to_first_result {
            if let Some(first) = response.results.first() {
                response.redirect_url = Some(first.url.clone());
            }
        }

        self.plugins.post_search(
            &mut response,
            &context,
            &request.enabled_plugins,
            &request.disabled_plugins,
        );
        // Upstream answerers (statistics, random, weather) answer in front of
        // the results; they run after the plugins like upstream's answerers.
        let answer_context = crate::answerers::AnswerContext {
            locale: context.locale.clone(),
            base_url: context.base_url.clone(),
            network: Some(self.network.clone()),
        };
        let answers = crate::answerers::ask(
            &crate::answerers::all(),
            &response.search_query,
            &answer_context,
        )
        .await;
        response.answers.extend(answers);
        Metrics::incr(&self.metrics.results, response.results.len() as u64);
        response
    }

    async fn run_engine(&self, engine: &EngineHandle, request: &SearchRequest) -> EngineOutcome {
        let name = engine.name().to_string();
        let weight = engine.weight();

        if let Some((remaining, reason)) = self.states.suspension(&name) {
            let kind = failure_kind_from_error_type(reason.as_deref().unwrap_or("suspended"));
            tracing::debug!(engine = %name, remaining = ?remaining, "engine suspended, skipping");
            return EngineOutcome {
                engine: name,
                weight,
                items: Err(kind),
                elapsed: Duration::ZERO,
                suspended: true,
            };
        }

        let started = Instant::now();
        let request_limit = request
            .timeout_limit
            .unwrap_or(self.settings.search.timeout_limit)
            .max(0.1);
        let timeout = engine.timeout().min(Duration::from_secs_f64(request_limit));

        Metrics::incr(&self.metrics.engine_requests, 1);
        let items = self.run_engine_inner(engine, request, timeout).await;
        let elapsed = started.elapsed();

        match &items {
            Ok(items) => self.states.record_success(&name, items.len(), elapsed),
            Err(kind) => {
                Metrics::incr(&self.metrics.engine_errors, 1);
                if *kind == FailureKind::Timeout {
                    Metrics::incr(&self.metrics.engine_timeouts, 1);
                }
                self.states
                    .record_failure(&name, *kind, &self.settings.search);
            }
        }

        EngineOutcome {
            engine: name,
            weight,
            items,
            elapsed,
            suspended: false,
        }
    }

    async fn run_engine_inner(
        &self,
        engine: &EngineHandle,
        request: &SearchRequest,
        timeout: Duration,
    ) -> Result<Vec<EngineItem>, FailureKind> {
        let engine_request = engine
            .build_request(&request.query, request.pageno)
            .map_err(|error| FailureKind::from(&error))?;
        let response = self
            .network
            .fetch(engine_request, timeout, Some(&self.cache))
            .await
            .map_err(|error| error.failure_kind())?;
        engine
            .parse(&response, &request.query)
            .map_err(|error| FailureKind::from(&error))
    }

    /// Engine statistics for `/stats`.
    pub fn engine_snapshots(&self) -> Vec<EngineSnapshot> {
        self.states.snapshot()
    }
}

/// `[{"answer": "..."}]`, the shape upstream uses for answers.
fn serialize_answers<S>(answers: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    #[derive(Serialize)]
    struct Answer<'a> {
        answer: &'a str,
    }
    answers
        .iter()
        .map(|answer| Answer { answer })
        .collect::<Vec<_>>()
        .serialize(serializer)
}

/// `[[engine, "Translated error"]]`, the shape upstream uses.
fn serialize_unresponsive<S>(
    engines: &[UnresponsiveEngine],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    let mut pairs: Vec<(String, String)> = engines
        .iter()
        .map(|engine| {
            let mut text = match engine.error_type.as_str() {
                "timeout" => "Timeout".to_string(),
                "access denied" => "Access denied".to_string(),
                "captcha" => "CAPTCHA".to_string(),
                "too many requests" => "Too many requests".to_string(),
                "http error" => "HTTP error".to_string(),
                "parsing error" => "Parsing error".to_string(),
                "suspended" => "Suspended".to_string(),
                _ => "Unexpected error".to_string(),
            };
            if engine.suspended && text != "Suspended" {
                text = format!("Suspended: {text}");
            }
            (engine.engine.clone(), text)
        })
        .collect();
    pairs.sort();
    pairs.serialize(serializer)
}

fn failure_kind_from_error_type(error_type: &str) -> FailureKind {
    match error_type {
        "timeout" => FailureKind::Timeout,
        "access denied" => FailureKind::AccessDenied,
        "captcha" => FailureKind::Captcha,
        "too many requests" => FailureKind::TooManyRequests,
        "http error" => FailureKind::Http,
        "parsing error" => FailureKind::Parse,
        _ => FailureKind::Other,
    }
}

#[cfg(test)]
mod orchestration_tests {
    use super::*;
    use crate::engine::{Engine, EngineError, EngineMeta, EngineRequest, EngineResponse};

    /// An engine that never touches the network: it either fails while
    /// building the request or returns synthetic results.
    struct FakeEngine {
        meta: EngineMeta,
        results: usize,
        build_error: bool,
    }

    impl FakeEngine {
        fn build(
            name: &str,
            categories: &[&str],
            results: usize,
            build_error: bool,
        ) -> EngineHandle {
            let config = crate::settings::EngineConfig::new(name, "fake", name, categories);
            let mut meta = EngineMeta::from_config(
                &config,
                crate::engine::EngineDefaults {
                    categories: &["general"],
                    shortcut: "fake",
                    ..Default::default()
                },
            );
            meta.categories = categories.iter().map(|c| (*c).to_string()).collect();
            std::sync::Arc::new(Self {
                meta,
                results,
                build_error,
            })
        }
    }

    impl Engine for FakeEngine {
        fn meta(&self) -> &EngineMeta {
            &self.meta
        }
        fn build_request(&self, query: &Query, _pageno: u32) -> Result<EngineRequest, EngineError> {
            if self.build_error {
                return Err(EngineError::Message("boom".into()));
            }
            Ok(EngineRequest::get(format!(
                "https://fake.invalid/?q={}",
                crate::engine::util::encode(&query.query_text())
            )))
        }
        fn parse(
            &self,
            _response: &EngineResponse,
            _query: &Query,
        ) -> Result<Vec<EngineItem>, EngineError> {
            Ok((0..self.results)
                .map(|index| {
                    EngineItem::Result(SearchResult::new(
                        format!("https://example.com/{}/{index}", self.meta.name),
                        format!("{} result {index}", self.meta.name),
                    ))
                })
                .collect())
        }
    }

    fn searcher(engines: Vec<EngineHandle>) -> Searcher {
        let settings = Settings::defaults();
        let mut registry = EngineRegistry::default();
        for handle in engines {
            registry.push(handle);
        }
        let network = Network::from_settings(&settings).unwrap();
        Searcher {
            plugins: Arc::new(PluginManager::new()),
            settings: Arc::new(settings),
            registry: Arc::new(registry),
            network: Arc::new(network),
            cache: Arc::new(ResponseCache::default()),
            states: Arc::new(EngineStates::new()),
            metrics: Arc::new(Metrics::new()),
        }
    }

    fn query(text: &str, registry: &EngineRegistry) -> Query {
        Query::parse(text, registry, &[])
    }

    #[test]
    fn engines_of_the_selected_category_are_used() {
        let searcher = searcher(vec![
            FakeEngine::build("web engine", &["general"], 1, true),
            FakeEngine::build("image engine", &["images"], 1, true),
        ]);
        let request = SearchRequest::new(query("cats", &searcher.registry));
        let selected: Vec<String> = searcher
            .select_engines(&request)
            .iter()
            .map(|engine| engine.name().to_string())
            .collect();
        assert_eq!(selected, vec!["web engine".to_string()]);
    }

    #[test]
    fn a_bang_selects_a_single_engine() {
        let searcher = searcher(vec![
            FakeEngine::build("web engine", &["general"], 1, true),
            FakeEngine::build("image engine", &["images"], 1, true),
        ]);
        let registry = searcher.registry.clone();
        let request = SearchRequest::new(query("!image_engine cats", &registry));
        let selected: Vec<String> = searcher
            .select_engines(&request)
            .iter()
            .map(|engine| engine.name().to_string())
            .collect();
        assert_eq!(selected, vec!["image engine".to_string()]);
    }

    #[tokio::test]
    async fn failing_engines_are_reported_as_unresponsive() {
        let searcher = searcher(vec![FakeEngine::build("broken", &["general"], 0, true)]);
        let request = SearchRequest::new(query("cats", &searcher.registry));
        let response = searcher.search(request).await;
        assert!(response.results.is_empty());
        assert_eq!(response.unresponsive_engines.len(), 1);
        assert_eq!(response.unresponsive_engines[0].engine, "broken");
    }

    #[tokio::test]
    async fn no_engine_selected_is_an_error() {
        let searcher = searcher(vec![FakeEngine::build(
            "image engine",
            &["images"],
            1,
            true,
        )]);
        let mut request = SearchRequest::new(query("cats", &searcher.registry));
        request.categories = vec!["videos".to_string()];
        let response = searcher.search(request).await;
        assert!(response.error.is_some());
    }

    #[tokio::test]
    async fn external_bangs_redirect() {
        let searcher = searcher(vec![FakeEngine::build("web engine", &["general"], 1, true)]);
        let request = SearchRequest::new(query("!!yt cats", &searcher.registry));
        let response = searcher.search(request).await;
        assert_eq!(
            response.redirect_url.as_deref(),
            Some("https://www.youtube.com/results?search_query=cats")
        );
    }

    #[tokio::test]
    async fn suspended_engines_are_skipped() {
        let searcher = searcher(vec![FakeEngine::build("broken", &["general"], 0, true)]);
        searcher.states.record_failure(
            "broken",
            FailureKind::TooManyRequests,
            &searcher.settings.search,
        );
        let request = SearchRequest::new(query("cats", &searcher.registry));
        let response = searcher.search(request).await;
        assert_eq!(response.unresponsive_engines.len(), 1);
        assert!(response.unresponsive_engines[0].suspended);
        assert_eq!(
            response.unresponsive_engines[0].error_type,
            "too many requests"
        );
    }
}
