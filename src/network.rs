//! HTTP layer, ported from `searx/network/`.
//!
//! Responsibilities kept from upstream: rotating user agents, per request
//! timeout, proxies, a redirect limit, retries on transport errors and a
//! response cache.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue};
use tracing::debug;

use crate::cache::{Cache, CachedResponse};
use crate::engine::{EngineRequest, EngineResponse, FailureKind, Method};
use crate::settings::Settings;

const USER_AGENT_OS: &[&str] = &[
    "Windows NT 10.0; Win64; x64",
    "X11; Linux x86_64",
    "Macintosh; Intel Mac OS X 10.15",
    "X11; Ubuntu; Linux x86_64",
];
const USER_AGENT_VERSIONS: &[&str] = &["157.0", "156.0", "155.0", "154.0"];
const USER_AGENT_TEMPLATE: &str =
    "Mozilla/5.0 ({os}; rv:{version}) Gecko/20100101 Firefox/{version}";

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("request timed out")]
    Timeout,
    #[error("http status {status}")]
    Status {
        status: u16,
        body: String,
        response: Box<EngineResponse>,
    },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("request could not be built: {0}")]
    Build(String),
}

impl NetworkError {
    /// Map a network error onto the framework failure kinds.
    pub fn failure_kind(&self) -> FailureKind {
        match self {
            Self::Timeout => FailureKind::Timeout,
            Self::Build(_) => FailureKind::Other,
            Self::Transport(_) => FailureKind::Other,
            Self::Status { status, body, .. } => classify_status(*status, body),
        }
    }
}

/// Classify an HTTP status (with a body excerpt) the way upstream does.
pub fn classify_status(status: u16, body: &str) -> FailureKind {
    let lowered = body.to_lowercase();
    let captcha = lowered.contains("captcha")
        || lowered.contains("anomaly")
        || lowered.contains("unusual traffic")
        || lowered.contains("are you a robot");
    match status {
        403 if captcha => FailureKind::Captcha,
        401..=403 => FailureKind::AccessDenied,
        429 => FailureKind::TooManyRequests,
        500..=599 => FailureKind::Http,
        _ => FailureKind::Http,
    }
}

/// The HTTP client used by every engine.
pub struct Network {
    client: reqwest::Client,
    user_agents: Vec<String>,
    suffix: String,
    retries: u32,
    counter: AtomicUsize,
}

impl Network {
    pub fn from_settings(settings: &Settings) -> Result<Self, NetworkError> {
        let outgoing = &settings.outgoing;
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(outgoing.max_redirects))
            .timeout(Duration::from_secs_f64(
                outgoing.max_request_timeout.max(1.0),
            ))
            .connect_timeout(Duration::from_secs_f64(
                (outgoing.request_timeout / 2.0).clamp(1.0, 5.0),
            ))
            .pool_max_idle_per_host(outgoing.pool_maxsize)
            .danger_accept_invalid_certs(!outgoing.verify)
            .user_agent(default_user_agent());
        if !outgoing.enable_http2 {
            builder = builder.http1_only();
        }
        for (scheme, proxy) in &outgoing.proxies {
            // One proxy per scheme (upstream's \`outgoing.proxies\`); the scheme key
            // selects which requests it applies to in SearXNG, reqwest proxies all.
            let proxy = reqwest::Proxy::all(proxy)
                .map_err(|error| NetworkError::Build(error.to_string()))?;
            tracing::debug!(scheme, "configured proxy");
            builder = builder.proxy(proxy);
        }
        let client = builder
            .build()
            .map_err(|error| NetworkError::Build(error.to_string()))?;
        Ok(Self {
            client,
            user_agents: build_user_agents(),
            suffix: outgoing.useragent_suffix.clone(),
            retries: outgoing.retries,
            counter: AtomicUsize::new(0),
        })
    }

    /// A different user agent on every call, like upstream's rotation.
    pub fn user_agent(&self) -> String {
        if self.user_agents.is_empty() {
            return default_user_agent();
        }
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % self.user_agents.len();
        format!("{} {}", self.user_agents[index], self.suffix)
            .trim()
            .to_string()
    }

    fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.7"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("dnt"),
            HeaderValue::from_static("1"),
        );
        if let Ok(value) = HeaderValue::from_str(&self.user_agent()) {
            headers.insert(reqwest::header::USER_AGENT, value);
        }
        headers
    }

    /// Fetch a request, using the cache when one is given.
    pub async fn fetch(
        &self,
        request: EngineRequest,
        timeout: Duration,
        cache: Option<&Cache>,
    ) -> Result<EngineResponse, NetworkError> {
        let body_repr = if let Some(json) = &request.json {
            json.to_string()
        } else {
            request
                .form
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&")
        };
        let method = match request.method {
            Method::Get => "GET",
            Method::Post => "POST",
        };
        let cache_key = Cache::key(method, &request.url, &body_repr);
        if let Some(cache) = cache {
            if let Some(cached) = cache.get(&cache_key).await {
                debug!(url = %request.url, "cache hit");
                return Ok(EngineResponse {
                    status: cached.status,
                    final_url: cached.final_url,
                    headers: cached.headers,
                    body: cached.body.as_ref().clone(),
                });
            }
        }

        let deadline = tokio::time::Instant::now() + timeout;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(NetworkError::Timeout);
            }
            let outcome = self.send(&request, remaining).await;
            match outcome {
                Ok(response) => {
                    if let Some(cache) = cache {
                        if response.status == 200 && !response.body.is_empty() {
                            cache
                                .set(
                                    &cache_key,
                                    CachedResponse {
                                        status: response.status,
                                        final_url: response.final_url.clone(),
                                        headers: response.headers.clone(),
                                        body: std::sync::Arc::new(response.body.clone()),
                                    },
                                )
                                .await;
                        }
                    }
                    if response.status >= 400 {
                        return Err(NetworkError::Status {
                            status: response.status,
                            body: String::from_utf8_lossy(
                                &response.body[..response.body.len().min(4096)],
                            )
                            .to_string(),
                            response: Box::new(response),
                        });
                    }
                    return Ok(response);
                }
                Err(error) if attempt <= self.retries && is_retryable(&error) => {
                    // never retry a timeout: the engine deadline already elapsed
                    if matches!(error, NetworkError::Timeout) {
                        return Err(error);
                    }
                    let backoff = Duration::from_millis(250 * attempt as u64);
                    if tokio::time::Instant::now() + backoff >= deadline {
                        return Err(error);
                    }
                    debug!(url = %request.url, %error, "retrying request");
                    tokio::time::sleep(backoff).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn send(
        &self,
        request: &EngineRequest,
        timeout: Duration,
    ) -> Result<EngineResponse, NetworkError> {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
        };
        let mut headers = self.default_headers();
        for (name, value) in request.headers.iter() {
            headers.insert(name.clone(), value.clone());
        }
        if !request.cookies.is_empty() {
            let cookie = request
                .cookies
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("; ");
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                headers.insert(reqwest::header::COOKIE, value);
            }
        }

        let mut builder = self.client.request(method, &request.url).headers(headers);
        if let Some(json) = &request.json {
            builder = builder.json(json);
        } else if request.method == Method::Post {
            builder = builder.form(&request.form);
        }

        let send = async {
            let response = builder.send().await.map_err(|error| {
                if error.is_timeout() {
                    NetworkError::Timeout
                } else {
                    NetworkError::Transport(error.to_string())
                }
            })?;
            let status = response.status().as_u16();
            let final_url = response.url().to_string();
            let mut headers = BTreeMap::new();
            for (name, value) in response.headers().iter() {
                if let Ok(value) = value.to_str() {
                    headers.insert(name.as_str().to_lowercase(), value.to_string());
                }
            }
            let body = response
                .bytes()
                .await
                .map_err(|error| NetworkError::Transport(error.to_string()))?
                .to_vec();
            Ok(EngineResponse {
                status,
                final_url,
                headers,
                body,
            })
        };

        match tokio::time::timeout(timeout, send).await {
            Ok(result) => result,
            Err(_) => Err(NetworkError::Timeout),
        }
    }
}

fn is_retryable(error: &NetworkError) -> bool {
    match error {
        NetworkError::Transport(_) => true,
        // a timeout already consumed the whole engine budget
        NetworkError::Timeout => false,
        NetworkError::Status { status, .. } => *status == 429 || *status >= 500,
        NetworkError::Build(_) => false,
    }
}

/// The default user agent, mirroring upstream's `searxng_useragent`.
pub fn default_user_agent() -> String {
    build_user_agents()
        .into_iter()
        .next()
        .unwrap_or_else(|| "rustxng".to_string())
}

/// Build the user agent pool like upstream's `data/useragents.json`.
fn build_user_agents() -> Vec<String> {
    let mut agents = Vec::new();
    for os in USER_AGENT_OS {
        for version in USER_AGENT_VERSIONS {
            agents.push(
                USER_AGENT_TEMPLATE
                    .replace("{os}", os)
                    .replace("{version}", version),
            );
        }
    }
    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agents_are_diverse_and_plausible() {
        let network = Network {
            client: reqwest::Client::new(),
            user_agents: build_user_agents(),
            suffix: String::new(),
            retries: 0,
            counter: AtomicUsize::new(0),
        };
        let first = network.user_agent();
        let second = network.user_agent();
        assert!(first.starts_with("Mozilla/5.0"));
        assert_ne!(first, second);
    }

    #[test]
    fn captcha_pages_are_recognised() {
        assert_eq!(
            classify_status(403, "please solve the captcha"),
            FailureKind::Captcha
        );
        assert_eq!(classify_status(403, "denied"), FailureKind::AccessDenied);
        assert_eq!(classify_status(429, ""), FailureKind::TooManyRequests);
        assert_eq!(classify_status(500, ""), FailureKind::Http);
    }
}
