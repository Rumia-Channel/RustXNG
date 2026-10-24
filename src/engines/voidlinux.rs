//! Void Linux engine, ported from searx/engines/voidlinux.py.
// engine-id: voidlinux

use std::sync::Arc;

use crate::engine::util::*;
use crate::engine::{
    Engine, EngineDefaults, EngineError, EngineItem, EngineMeta, EngineRequest, EngineResponse,
};
use crate::query::Query;
use crate::results::Template;
use crate::settings::EngineConfig;

const DEFAULTS: EngineDefaults = EngineDefaults {
    categories: &["packages", "it"],
    shortcut: "void",
    paging: false,
    time_range_support: false,
    safesearch: false,
    language_support: false,
    weight: 1.0,
    timeout: 4.0,
};

const BASE_URL: &str = "https://xq-api.voidlinux.org";
const PKG_REPO_URL: &str = "https://github.com/void-linux/void-packages";
/// Upstream's ARCH_RE, longest first so the -musl variants win.
const ARCHES: [&str; 9] = [
    "aarch64-musl", "armv6l-musl", "armv7l-musl", "x86_64-musl", "aarch64", "armv6l", "armv7l",
    "i686", "x86_64",
];

/// Void Linux binary packages.
pub struct VoidLinux {
    meta: EngineMeta,
}

impl VoidLinux {
    pub fn from_config(config: &EngineConfig) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self { meta: EngineMeta::from_config(config, DEFAULTS) }))
    }

    fn base_url(&self) -> &str {
        self.meta.base_url.as_deref().unwrap_or(BASE_URL).trim_end_matches('/')
    }

    fn arch(&self) -> &str {
        self.meta.opt_str("void_arch").unwrap_or("x86_64")
    }

    /// The architecture of the query and the search terms without it.
    fn split_arch(&self, raw: &str) -> (String, String) {
        let mut arch = None;
        let mut terms = Vec::new();
        for token in raw.split_whitespace() {
            if arch.is_none() && ARCHES.contains(&token) {
                arch = Some(token.to_string());
            } else {
                terms.push(token);
            }
        }
        (arch.unwrap_or_else(|| self.arch().to_string()), terms.join(" "))
    }
}

impl Engine for VoidLinux {
    fn meta(&self) -> &EngineMeta {
        &self.meta
    }

    fn build_request(&self, query: &Query, _pageno: u32) -> Result<EngineRequest, EngineError> {
        let (arch, terms) = self.split_arch(&query.query_text());
        let pairs: Vec<(&str, String)> = vec![("q", terms)];
        Ok(EngineRequest::get(format!(
            "{}/v1/query/{}?{}",
            self.base_url(),
            arch,
            query_string(&pairs)
        )))
    }

    fn parse(
        &self,
        response: &EngineResponse,
        _query: &Query,
    ) -> Result<Vec<EngineItem>, EngineError> {
        let json = response.json()?;
        let Some(entries) = json_path(&json, "data").and_then(serde_json::Value::as_array) else {
            return Ok(Vec::new());
        };
        // Packages sharing one source template and URL are merged.
        let mut packages: Vec<(String, Vec<Package>)> = Vec::new();
        for item in entries {
            let name = json_str(item, "name");
            if name.is_empty() {
                continue;
            }
            let pkg_url = format!("{PKG_REPO_URL}/tree/master/srcpkgs/{}", source_slug(&name));
            let size = json_str(item, "filename_size")
                .parse::<f64>()
                .map(humanize_bytes)
                .unwrap_or_default();
            let package = Package {
                title: name.clone(),
                content: format!("{} - {}", json_str(item, "short_desc"), size),
                package_name: name,
                version: format!("v{}_{}", json_str(item, "version"), json_str(item, "revision")),
                tags: json_str(item, "repository"),
            };
            let existing = packages
                .iter_mut()
                .position(|(url, _)| url.as_str() == pkg_url.as_str());
            match existing {
                Some(index) => packages[index].1.push(package),
                None => packages.push((pkg_url, vec![package])),
            }
        }
        let mut results = Vec::new();
        for (url, list) in packages {
            let title = list.iter().map(|p| p.title.as_str()).collect::<Vec<_>>().join(" | ");
            let names = list.iter().map(|p| p.package_name.as_str()).collect::<Vec<_>>().join(" | ");
            let tags = list.iter().map(|p| p.tags.as_str()).collect::<Vec<_>>().join(", ");
            let first = &list[0];
            results.push(
                ResultBuilder::new(url, title)
                    .content(first.content.clone())
                    .template(Template::Packages)
                    .metadata("package_name", names)
                    .metadata("version", first.version.clone())
                    .metadata("tags", tags)
                    .build(),
            );
        }
        Ok(results)
    }
}

struct Package {
    title: String,
    content: String,
    package_name: String,
    version: String,
    tags: String,
}

/// The source package name: the -32bit and -dbg packages have no template.
fn source_slug(name: &str) -> String {
    for suffix in ["-32bit", "-dbg"] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    name.to_string()
}

/// Upstream's humanize_bytes (base 1024, two decimals).
fn humanize_bytes(mut size: f64) -> String {
    const SUFFIX: [&str; 5] = ["B ", "KB", "MB", "GB", "TB"];
    let mut position = 0usize;
    while size > 1024.0 && position < SUFFIX.len() {
        position += 1;
        size /= 1024.0;
    }
    let position = position.min(SUFFIX.len() - 1);
    format!("{size:.2} {}", SUFFIX[position])
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r##"{"data":[
      {"name":"firefox","short_desc":"Mozilla Firefox","filename_size":73400320,
       "version":"128.0","revision":"1","repository":"current"},
      {"name":"firefox-32bit","short_desc":"Mozilla Firefox","filename_size":73000000,
       "version":"128.0","revision":"1","repository":"multilib"},
      {"name":"htop","short_desc":"Interactive process viewer","filename_size":2048,
       "version":"3.3.0","revision":"2","repository":"current"}]}"##;

    fn engine() -> Arc<VoidLinux> {
        VoidLinux::from_config(&EngineConfig::new("voidlinux", "voidlinux", "void", &["packages"]))
            .unwrap()
    }

    fn query(raw: &str) -> Query {
        Query::parse(raw, &crate::engine::EngineRegistry::default(), &[])
    }

    #[test]
    fn merges_packages_sharing_a_source_template() {
        let response = EngineResponse::new(200, BASE_URL, FIXTURE.as_bytes().to_vec());
        let items = engine().parse(&response, &query("firefox")).unwrap();
        assert_eq!(items.len(), 2);
        let EngineItem::Result(result) = &items[0] else { panic!("expected a package") };
        assert_eq!(result.url, "https://github.com/void-linux/void-packages/tree/master/srcpkgs/firefox");
        assert_eq!(
            (result.title.as_str(), result.content.as_str(), result.template),
            ("firefox | firefox-32bit", "Mozilla Firefox - 70.00 MB", Template::Packages)
        );
        assert_eq!(
            (
                result.metadata.get("package_name").map(String::as_str),
                result.metadata.get("version").map(String::as_str),
                result.metadata.get("tags").map(String::as_str),
            ),
            (Some("firefox | firefox-32bit"), Some("v128.0_1"), Some("current, multilib"))
        );
        assert!(matches!(&items[1], EngineItem::Result(second)
            if second.content == "Interactive process viewer - 2.00 KB"));
    }

    #[test]
    fn extracts_the_architecture_from_the_query() {
        let request = engine().build_request(&query("htop aarch64-musl"), 1).unwrap();
        assert_eq!(request.url, "https://xq-api.voidlinux.org/v1/query/aarch64-musl?q=htop");
        let request = engine().build_request(&query("htop"), 1).unwrap();
        assert_eq!(request.url, "https://xq-api.voidlinux.org/v1/query/x86_64?q=htop");
    }
}
