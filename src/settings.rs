//! Settings handling, ported from `searx/settings_defaults.py`,
//! `searx/settings_loader.py` and `searx/settings.yml`.
//!
//! Differences to upstream are deliberate and documented:
//!
//! * the built-in defaults ship as a TOML file (`settings.toml`); YAML engines
//!   such as the generic `xpath` engine are not ported,
//! * `use_default_settings` can be `true` (merge user engines into the
//!   defaults, the upstream default) or `false` (replace the engine list),
//! * unknown engine specific keys are kept in `EngineConfig::extra` so that a
//!   config written for SearXNG keeps loading.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::locales;

/// The built-in defaults, embedded at compile time.
pub const DEFAULT_SETTINGS: &str = include_str!("../settings.toml");

/// Formats the `/search` endpoint can render (upstream `OUTPUT_FORMATS`).
pub static OUTPUT_FORMATS: &[&str] = &["html", "csv", "json", "rss"];

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not read settings file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid settings file: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid settings: {0}")]
    Validation(String),
}

/// Top level settings object.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    /// When true the engines of the defaults and of the user file are merged.
    pub use_default_settings: Option<bool>,
    pub general: General,
    pub server: Server,
    pub search: Search,
    pub ui: Ui,
    pub outgoing: Outgoing,
    pub engines: Vec<EngineConfig>,
    pub plugins: Plugins,
    /// Categories shown as tabs (upstream `categories_as_tabs`).
    pub categories_as_tabs: Vec<String>,
    /// Accepted for compatibility with SearXNG configs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi_resolvers: Option<toml::Table>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct General {
    pub debug: bool,
    pub instance_name: String,
    pub contact_url: String,
    pub donation_url: String,
    pub privacypolicy_url: String,
    pub enable_metrics: bool,
    pub public_instance: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            debug: false,
            instance_name: "RustXNG".into(),
            contact_url: String::new(),
            donation_url: String::new(),
            privacypolicy_url: String::new(),
            enable_metrics: true,
            public_instance: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Server {
    pub bind_address: String,
    pub port: u16,
    pub base_url: Option<String>,
    pub secret_key: String,
    pub limiter: bool,
    /// Length of the limiter window in seconds (our simplification).
    pub limiter_window: u64,
    /// Requests allowed per client and window (our simplification).
    pub limiter_max: u32,
    pub image_proxy: bool,
    /// HTTP method of the search form: `GET` or `POST`.
    pub method: String,
    pub http_protocol_version: Option<String>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".into(),
            port: 8888,
            base_url: None,
            secret_key: "ultrasecretkey".into(),
            limiter: false,
            limiter_window: 60,
            limiter_max: 30,
            image_proxy: false,
            method: "POST".into(),
            http_protocol_version: None,
        }
    }
}

fn all_locale_tags() -> Vec<String> {
    locales::LOCALE_TAGS
        .iter()
        .map(|tag| (*tag).to_string())
        .collect()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Search {
    pub safe_search: u8,
    /// Autocomplete backend chain, `` or `none` disables autocompletion.
    pub autocomplete: String,
    pub autocomplete_min: usize,
    pub default_lang: String,
    pub languages: Vec<String>,
    pub ban_time_on_fail: u64,
    pub max_ban_time_on_fail: u64,
    pub suspended_times: BTreeMap<String, u64>,
    pub formats: Vec<String>,
    pub max_page: u32,
    pub timeout_limit: f64,
}

impl Default for Search {
    fn default() -> Self {
        let mut suspended_times = BTreeMap::new();
        suspended_times.insert("SearxEngineAccessDenied".into(), 86_400);
        suspended_times.insert("SearxEngineCaptcha".into(), 86_400);
        suspended_times.insert("SearxEngineTooManyRequests".into(), 3_600);
        suspended_times.insert("cf_SearxEngineCaptcha".into(), 1_296_000);
        suspended_times.insert("cf_SearxEngineAccessDenied".into(), 86_400);
        suspended_times.insert("recaptcha_SearxEngineCaptcha".into(), 604_800);
        Self {
            safe_search: 0,
            autocomplete: "duckduckgo".into(),
            autocomplete_min: 4,
            default_lang: String::new(),
            languages: all_locale_tags(),
            ban_time_on_fail: 5,
            max_ban_time_on_fail: 120,
            suspended_times,
            formats: OUTPUT_FORMATS.iter().map(|f| (*f).to_string()).collect(),
            max_page: 0,
            timeout_limit: 3.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Ui {
    pub default_theme: String,
    pub default_locale: String,
    pub center_alignment: bool,
    pub results_on_new_tab: bool,
    pub infinite_scroll: bool,
    pub search_on_category_select: bool,
    pub hotkeys: String,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            default_theme: "simple".into(),
            default_locale: String::new(),
            center_alignment: false,
            results_on_new_tab: false,
            infinite_scroll: false,
            search_on_category_select: true,
            hotkeys: "default".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Outgoing {
    pub request_timeout: f64,
    pub max_request_timeout: f64,
    pub useragent_suffix: String,
    pub pool_connections: usize,
    pub pool_maxsize: usize,
    pub enable_http2: bool,
    pub verify: bool,
    pub retries: u32,
    pub max_redirects: usize,
    pub keepalive_expiry: f64,
    pub proxies: BTreeMap<String, String>,
    pub using_tor_proxy: bool,
}

impl Default for Outgoing {
    fn default() -> Self {
        Self {
            request_timeout: 4.0,
            max_request_timeout: 20.0,
            useragent_suffix: String::new(),
            pool_connections: 100,
            pool_maxsize: 20,
            enable_http2: true,
            verify: true,
            retries: 1,
            max_redirects: 20,
            keepalive_expiry: 5.0,
            proxies: BTreeMap::new(),
            using_tor_proxy: false,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Plugins {
    /// Plugins enabled for every request (upstream `enabled_plugins`).
    pub enabled: Vec<String>,
    /// Plugins disabled for every request (upstream `disabled_plugins`).
    pub disabled: Vec<String>,
}

/// One entry of the `engines` list.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineConfig {
    /// Display name, used by bangs and preferences.
    pub name: String,
    /// Engine implementation id (`searx/engines/<engine>.py` upstream).
    pub engine: String,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub inactive: bool,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub timeout: Option<f64>,
    #[serde(default)]
    pub paging: Option<bool>,
    #[serde(default)]
    pub language_support: Option<bool>,
    #[serde(default)]
    pub time_range_support: Option<bool>,
    #[serde(default)]
    pub safesearch: Option<bool>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub search_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    /// Free-form, engine specific options (kept for SearXNG compatibility).
    #[serde(flatten, default)]
    pub extra: toml::Table,
}

impl EngineConfig {
    /// Build an engine config programmatically (used by the built-in defaults
    /// and by tests).
    pub fn new(name: &str, engine: &str, shortcut: &str, categories: &[&str]) -> Self {
        Self {
            name: name.into(),
            engine: engine.into(),
            shortcut: Some(shortcut.into()),
            categories: categories.iter().map(|c| (*c).to_string()).collect(),
            disabled: false,
            inactive: false,
            weight: None,
            timeout: None,
            paging: None,
            language_support: None,
            time_range_support: None,
            safesearch: None,
            base_url: None,
            search_url: None,
            api_key: None,
            extra: toml::Table::new(),
        }
    }

    pub fn opt_str(&self, key: &str) -> Option<&str> {
        self.extra.get(key).and_then(|value| value.as_str())
    }

    pub fn opt_bool(&self, key: &str) -> Option<bool> {
        self.extra.get(key).and_then(|value| value.as_bool())
    }

    pub fn opt_i64(&self, key: &str) -> Option<i64> {
        self.extra.get(key).and_then(|value| value.as_integer())
    }

    pub fn opt_f64(&self, key: &str) -> Option<f64> {
        self.extra
            .get(key)
            .and_then(|value| value.as_float())
            .or_else(|| {
                self.extra
                    .get(key)
                    .and_then(|value| value.as_integer())
                    .map(|v| v as f64)
            })
    }

    pub fn opt_str_list(&self, key: &str) -> Vec<String> {
        self.extra
            .get(key)
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Shortcut falling back to the engine id, like upstream.
    pub fn shortcut_or_name(&self) -> String {
        self.shortcut.clone().unwrap_or_else(|| self.engine.clone())
    }
}

impl Settings {
    /// The built-in defaults (no user file involved).
    pub fn defaults() -> Settings {
        toml::from_str(DEFAULT_SETTINGS).expect("built-in settings.toml must be valid")
    }

    /// Load settings from `path`, merged onto the built-in defaults, then apply
    /// the `SEARXNG_*` environment overrides.
    pub fn load(path: Option<&Path>) -> Result<Settings, SettingsError> {
        let mut settings = match path {
            Some(path) => {
                let raw = std::fs::read_to_string(path).map_err(|source| SettingsError::Read {
                    path: path.to_path_buf(),
                    source,
                })?;
                let user: Settings = toml::from_str(&raw)?;
                Self::merge(Settings::defaults(), user)?
            }
            None => Settings::defaults(),
        };
        settings.apply_env();
        settings.validate()?;
        Ok(settings)
    }

    /// Resolve the settings path from `SEARXNG_SETTINGS_PATH` (upstream
    /// behaviour) or `--config`.
    pub fn path_from_env() -> Option<PathBuf> {
        std::env::var("SEARXNG_SETTINGS_PATH")
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    /// Merge a user settings object onto `base`.
    ///
    /// Like upstream, scalars and sections are overridden by the user, and the
    /// engine list is merged by engine name unless `use_default_settings` is
    /// `false`.
    fn merge(mut base: Settings, user: Settings) -> Result<Settings, SettingsError> {
        let merge_engines = user.use_default_settings.unwrap_or(true);
        base.use_default_settings = user.use_default_settings;

        // Sections: only overwrite fields the user actually set. TOML gives us
        // concrete values for every field, so we merge on the serialized form.
        base.general = merge_table(&base.general, &user.general)?;
        base.server = merge_table(&base.server, &user.server)?;
        base.search = merge_table(&base.search, &user.search)?;
        base.ui = merge_table(&base.ui, &user.ui)?;
        base.outgoing = merge_table(&base.outgoing, &user.outgoing)?;
        base.plugins = merge_table(&base.plugins, &user.plugins)?;
        if !user.categories_as_tabs.is_empty() {
            base.categories_as_tabs = user.categories_as_tabs;
        }
        if user.doi_resolvers.is_some() {
            base.doi_resolvers = user.doi_resolvers;
        }

        if !user.engines.is_empty() {
            if merge_engines {
                base.engines = merge_engines_by_name(base.engines, user.engines);
            } else {
                base.engines = user.engines;
            }
        }
        Ok(base)
    }

    fn apply_env(&mut self) {
        if let Ok(value) = std::env::var("SEARXNG_BIND_ADDRESS") {
            if !value.is_empty() {
                self.server.bind_address = value;
            }
        }
        if let Ok(value) = std::env::var("SEARXNG_PORT") {
            if let Ok(port) = value.parse::<u16>() {
                self.server.port = port;
            }
        }
        if let Ok(value) = std::env::var("SEARXNG_SECRET") {
            if !value.is_empty() {
                self.server.secret_key = value;
            }
        }
        if let Ok(value) = std::env::var("SEARXNG_BASE_URL") {
            if !value.is_empty() {
                self.server.base_url = Some(value);
            }
        }
        if let Ok(value) = std::env::var("SEARXNG_DEBUG") {
            self.general.debug = matches!(value.as_str(), "1" | "true" | "on");
        }
        if let Ok(value) = std::env::var("SEARXNG_ENABLE_METRICS") {
            self.general.enable_metrics = !matches!(value.as_str(), "0" | "false" | "off");
        }
        if let Ok(value) = std::env::var("SEARXNG_SEARCH_DEFAULT_LANG") {
            if !value.is_empty() {
                self.search.default_lang = value;
            }
        }
    }

    /// Validate the settings the way upstream does before starting.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.server.port == 0 {
            return Err(SettingsError::Validation(
                "server.port must not be 0".into(),
            ));
        }
        if self.server.method != "GET" && self.server.method != "POST" {
            return Err(SettingsError::Validation(format!(
                "server.method must be GET or POST, got {:?}",
                self.server.method
            )));
        }
        if self.search.safe_search > 2 {
            return Err(SettingsError::Validation(
                "search.safe_search must be 0, 1 or 2".into(),
            ));
        }
        if self.outgoing.request_timeout <= 0.0 || self.outgoing.max_request_timeout <= 0.0 {
            return Err(SettingsError::Validation(
                "outgoing timeouts must be positive".into(),
            ));
        }
        if self.outgoing.max_request_timeout < self.outgoing.request_timeout {
            return Err(SettingsError::Validation(
                "outgoing.max_request_timeout must be >= outgoing.request_timeout".into(),
            ));
        }
        for format in &self.search.formats {
            if !OUTPUT_FORMATS.contains(&format.as_str()) {
                return Err(SettingsError::Validation(format!(
                    "unknown search format {format:?} (allowed: {})",
                    OUTPUT_FORMATS.join(", ")
                )));
            }
        }
        let mut names: BTreeMap<&str, &str> = BTreeMap::new();
        let mut shortcuts: BTreeMap<&str, &str> = BTreeMap::new();
        for engine in &self.engines {
            if engine.name.trim().is_empty() {
                return Err(SettingsError::Validation(
                    "engine entries need a name".into(),
                ));
            }
            if engine.engine.trim().is_empty() {
                return Err(SettingsError::Validation(format!(
                    "engine {:?} needs an engine type",
                    engine.name
                )));
            }
            if let Some(previous) = names.insert(engine.name.as_str(), engine.engine.as_str()) {
                return Err(SettingsError::Validation(format!(
                    "duplicate engine name {:?} ({previous} and {})",
                    engine.name, engine.engine
                )));
            }
            if let Some(shortcut) = engine.shortcut.as_deref() {
                if let Some(previous) = shortcuts.insert(shortcut, engine.name.as_str()) {
                    return Err(SettingsError::Validation(format!(
                        "duplicate engine shortcut {shortcut:?} ({previous} and {})",
                        engine.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Engines that are enabled by default, in config order.
    pub fn enabled_engines(&self) -> impl Iterator<Item = &EngineConfig> {
        self.engines
            .iter()
            .filter(|engine| !engine.disabled && !engine.inactive)
    }

    pub fn engine(&self, name: &str) -> Option<&EngineConfig> {
        self.engines.iter().find(|engine| engine.name == name)
    }

    /// Default language list limited to what the UI offers.
    pub fn languages(&self) -> Vec<String> {
        self.search.languages.clone()
    }

    /// The base URL used for absolute links in feeds/opensearch.
    pub fn base_url(&self) -> String {
        self.server.base_url.clone().unwrap_or_else(|| {
            let host = if self.server.bind_address == "0.0.0.0" {
                "localhost"
            } else {
                self.server.bind_address.as_str()
            };
            format!("http://{host}:{}", self.server.port)
        })
    }
}

fn merge_table<T>(base: &T, user: &T) -> Result<T, SettingsError>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let base_value = toml::Value::try_from(base)
        .map_err(|error| SettingsError::Validation(error.to_string()))?;
    let user_value = toml::Value::try_from(user)
        .map_err(|error| SettingsError::Validation(error.to_string()))?;
    let merged = deep_merge(base_value, user_value);
    merged
        .try_into()
        .map_err(|error: toml::de::Error| SettingsError::Parse(error))
}

fn deep_merge(base: toml::Value, user: toml::Value) -> toml::Value {
    match (base, user) {
        (toml::Value::Table(mut base), toml::Value::Table(user)) => {
            for (key, value) in user {
                let merged = match base.remove(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => value,
                };
                base.insert(key, merged);
            }
            toml::Value::Table(base)
        }
        (_, user) => user,
    }
}

/// Merge user engine entries into the default list by engine name.
fn merge_engines_by_name(
    defaults: Vec<EngineConfig>,
    user: Vec<EngineConfig>,
) -> Vec<EngineConfig> {
    let mut merged = defaults;
    for engine in user {
        match merged.iter_mut().find(|entry| entry.name == engine.name) {
            Some(existing) => {
                // User entries win field by field; unset fields keep the default.
                if engine.shortcut.is_some() {
                    existing.shortcut = engine.shortcut.clone();
                }
                if !engine.categories.is_empty() {
                    existing.categories = engine.categories.clone();
                }
                if engine.disabled {
                    existing.disabled = true;
                }
                if engine.inactive {
                    existing.inactive = true;
                }
                if engine.weight.is_some() {
                    existing.weight = engine.weight;
                }
                if engine.timeout.is_some() {
                    existing.timeout = engine.timeout;
                }
                if engine.paging.is_some() {
                    existing.paging = engine.paging;
                }
                if engine.language_support.is_some() {
                    existing.language_support = engine.language_support;
                }
                if engine.time_range_support.is_some() {
                    existing.time_range_support = engine.time_range_support;
                }
                if engine.safesearch.is_some() {
                    existing.safesearch = engine.safesearch;
                }
                if engine.base_url.is_some() {
                    existing.base_url = engine.base_url.clone();
                }
                if engine.search_url.is_some() {
                    existing.search_url = engine.search_url.clone();
                }
                if engine.api_key.is_some() {
                    existing.api_key = engine.api_key.clone();
                }
                for (key, value) in engine.extra {
                    existing.extra.insert(key, value);
                }
            }
            None => merged.push(engine),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_defaults_are_valid() {
        let settings = Settings::defaults();
        settings.validate().expect("defaults must validate");
        assert!(!settings.engines.is_empty());
        assert_eq!(settings.server.port, 8888);
    }

    #[test]
    fn user_engines_merge_by_name() {
        let defaults = Settings::defaults();
        let user: Settings = toml::from_str(
            r#"
            use_default_settings = true
            [[engines]]
            name = "google"
            engine = "google"
            disabled = true
            [[engines]]
            name = "my instance"
            engine = "wikipedia"
            shortcut = "mywiki"
            categories = ["general"]
            "#,
        )
        .unwrap();
        let merged = Settings::merge(defaults, user).unwrap();
        let google = merged.engine("google").expect("google kept");
        assert!(google.disabled);
        assert!(merged.engine("my instance").is_some());
    }

    #[test]
    fn invalid_format_is_rejected() {
        let mut settings = Settings::defaults();
        settings.search.formats.push("yaml".into());
        assert!(settings.validate().is_err());
    }

    #[test]
    fn duplicate_shortcuts_are_rejected() {
        let mut settings = Settings::defaults();
        let mut clone = settings.engines[0].clone();
        clone.name = "clone of the first engine".into();
        clone.shortcut = settings.engines[0].shortcut.clone();
        settings.engines.push(clone);
        assert!(settings.validate().is_err());
    }
}
