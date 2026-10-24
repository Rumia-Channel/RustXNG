//! Engine implementations, one module per upstream `searx/engines/*.py`.
//!
//! | module                | upstream                    |
//! |-----------------------|-----------------------------|
//! | `arxiv`               | `arxiv.py`                   |
//! | `bing`                | `bing.py`                    |
//! | `brave`               | `brave.py`                   |
//! | `crates`              | `crates.py`                  |
//! | `crossref`            | `crossref.py`                |
//! | `docker_hub`          | `docker_hub.py`              |
//! | `duckduckgo`          | `duckduckgo.py`              |
//! | `github`              | `github.py`                  |
//! | `google`              | `google.py`                  |
//! | `hackernews`          | `hackernews.py`              |
//! | `mojeek`              | `mojeek.py`                  |
//! | `mwmbl`               | `mwmbl.py`                   |
//! | `npm`                 | `npm.py`                     |
//! | `openlibrary`         | `openlibrary.py`             |
//! | `openstreetmap`       | `openstreetmap.py`           |
//! | `photon`              | `photon.py`                  |
//! | `pypi`                | `pypi.py`                    |
//! | `semantic_scholar`    | `semantic_scholar.py`        |
//! | `stackexchange`       | `stackexchange.py`           |
//! | `wikidata`            | `wikidata.py`                |
//! | `wikipedia`           | `wikipedia.py`               |
//! | `youtube_noapi`       | `youtube_noapi.py`           |

pub mod andisearch;
pub mod apkmirror;
pub mod apple_app_store;
pub mod artstation;
pub mod arxiv;
pub mod baidu;
pub mod bandcamp;
pub mod bing;
pub mod bing_news;
pub mod brave;
pub mod crates;
pub mod crossref;
pub mod dailymotion;
pub mod deezer;
pub mod deviantart;
pub mod docker_hub;
pub mod dogpile;
pub mod duckduckgo;
pub mod findborg;
pub mod findfiles;
pub mod fireball;
pub mod five_hundred_px;
pub mod flickr_noapi;
pub mod genius;
pub mod giphy;
pub mod github;
pub mod goodreads;
pub mod google;
pub mod google_news;
pub mod google_play;
pub mod hackernews;
pub mod imdb;
pub mod imgur;
pub mod invidious;
pub mod json_engine;
pub mod lemmy;
pub mod luxxle;
pub mod mastodon;
pub mod mediawiki;
pub mod mixcloud;
pub mod mojeek;
pub mod mwmbl;
pub mod naver;
pub mod nine_gag;
pub mod npm;
pub mod openclipart;
pub mod openlibrary;
pub mod openstreetmap;
pub mod openverse;
pub mod peertube;
pub mod pexels;
pub mod photon;
pub mod piped;
pub mod pixabay;
pub mod pypi;
pub mod qwant;
pub mod semantic_scholar;
pub mod sepiasearch;
pub mod soundcloud;
pub mod stackexchange;
pub mod startpage;
pub mod startpagina;
pub mod tonline;
pub mod tootfinder;
pub mod tusksearch;
pub mod unsplash;
pub mod wallhaven;
pub mod wikicommons;
pub mod wikidata;
pub mod wikipedia;
pub mod xpath;
pub mod youtube_noapi;
pub mod alpinelinux;
pub mod annas_archive;
pub mod discourse;
pub mod duckduckgo_extra;
pub mod duckduckgo_web;
pub mod r1337x;
pub mod r360search;
pub mod artic;
pub mod bing_images;
pub mod bing_videos;
pub mod bitchute;
pub mod bpb;
pub mod braveapi;
pub mod cachy_os;
pub mod ccc_media;
pub mod chinaso;
pub mod cloudflareai;
pub mod core;
pub mod r360search_videos;

use crate::engine::EngineHandle;
use crate::settings::EngineConfig;

/// Build the engine implementation selected by `config.engine`.
pub fn build(config: &EngineConfig) -> Result<EngineHandle, String> {
    let handle: EngineHandle = match config.engine.as_str() {
        "arxiv" => arxiv::Arxiv::from_config(config)?,
        "bing" => bing::Bing::from_config(config)?,
        "bing_news" => bing_news::BingNews::from_config(config)?,
        "brave" => brave::Brave::from_config(config)?,
        "crates" => crates::Crates::from_config(config)?,
        "crossref" => crossref::Crossref::from_config(config)?,
        "dailymotion" => dailymotion::Dailymotion::from_config(config)?,
        "deezer" => deezer::Deezer::from_config(config)?,
        "docker_hub" => docker_hub::DockerHub::from_config(config)?,
        "duckduckgo" => duckduckgo::DuckDuckGo::from_config(config)?,
        "github" => github::GitHub::from_config(config)?,
        "google" => google::Google::from_config(config)?,
        "google_news" => google_news::GoogleNews::from_config(config)?,
        "hackernews" => hackernews::HackerNews::from_config(config)?,
        "mojeek" => mojeek::Mojeek::from_config(config)?,
        "mwmbl" => mwmbl::Mwmbl::from_config(config)?,
        "npm" => npm::Npm::from_config(config)?,
        "openlibrary" => openlibrary::OpenLibrary::from_config(config)?,
        "openstreetmap" => openstreetmap::Openstreetmap::from_config(config)?,
        "openverse" => openverse::Openverse::from_config(config)?,
        "photon" => photon::Photon::from_config(config)?,
        "pypi" => pypi::PyPi::from_config(config)?,
        "semantic_scholar" => semantic_scholar::SemanticScholar::from_config(config)?,
        "stackexchange" => stackexchange::StackExchange::from_config(config)?,
        "wikicommons" => wikicommons::Wikicommons::from_config(config)?,
        "wikidata" => wikidata::Wikidata::from_config(config)?,
        "wikipedia" => wikipedia::Wikipedia::from_config(config)?,
        "json_engine" => json_engine::JsonEngine::from_config(config)?,
        "mediawiki" => mediawiki::MediaWiki::from_config(config)?,
        "xpath" => xpath::XPathEngine::from_config(config)?,
        "youtube_noapi" => youtube_noapi::YoutubeNoapi::from_config(config)?,
        "andisearch" => andisearch::Andisearch::from_config(config)?,
        "apkmirror" => apkmirror::ApkMirror::from_config(config)?,
        "apple_app_store" => apple_app_store::AppleAppStore::from_config(config)?,
        "baidu" => baidu::Baidu::from_config(config)?,
        "dogpile" => dogpile::Dogpile::from_config(config)?,
        "findborg" => findborg::Findborg::from_config(config)?,
        "findfiles" => findfiles::FindFiles::from_config(config)?,
        "fireball" => fireball::Fireball::from_config(config)?,
        "500px" => five_hundred_px::FiveHundredPx::from_config(config)?,
        "google_play" => google_play::GooglePlay::from_config(config)?,
        "invidious" => invidious::Invidious::from_config(config)?,
        "lemmy" => lemmy::Lemmy::from_config(config)?,
        "luxxle" => luxxle::Luxxle::from_config(config)?,
        "mastodon" => mastodon::Mastodon::from_config(config)?,
        "naver" => naver::Naver::from_config(config)?,
        "9gag" => nine_gag::NineGag::from_config(config)?,
        "openclipart" => openclipart::OpenClipart::from_config(config)?,
        "peertube" => peertube::Peertube::from_config(config)?,
        "pexels" => pexels::Pexels::from_config(config)?,
        "piped" => piped::Piped::from_config(config)?,
        "pixabay" => pixabay::Pixabay::from_config(config)?,
        "qwant" => qwant::Qwant::from_config(config)?,
        "sepiasearch" => sepiasearch::Sepiasearch::from_config(config)?,
        "startpage" => startpage::Startpage::from_config(config)?,
        "startpagina" => startpagina::Startpagina::from_config(config)?,
        "tonline" => tonline::TOnline::from_config(config)?,
        "tootfinder" => tootfinder::Tootfinder::from_config(config)?,
        "tusksearch" => tusksearch::Tusksearch::from_config(config)?,
        "unsplash" => unsplash::Unsplash::from_config(config)?,
        "bandcamp" => bandcamp::Bandcamp::from_config(config)?,
        "genius" => genius::Genius::from_config(config)?,
        "goodreads" => goodreads::Goodreads::from_config(config)?,
        "imdb" => imdb::Imdb::from_config(config)?,
        "mixcloud" => mixcloud::Mixcloud::from_config(config)?,
        "soundcloud" => soundcloud::Soundcloud::from_config(config)?,
        "wallhaven" => wallhaven::Wallhaven::from_config(config)?,
        "artstation" => artstation::Artstation::from_config(config)?,
        "deviantart" => deviantart::Deviantart::from_config(config)?,
        "flickr_noapi" => flickr_noapi::FlickrNoapi::from_config(config)?,
        "giphy" => giphy::Giphy::from_config(config)?,
        "imgur" => imgur::Imgur::from_config(config)?,
        "alpinelinux" => alpinelinux::AlpineLinux::from_config(config)?,
        "annas_archive" => annas_archive::AnnasArchive::from_config(config)?,
        "discourse" => discourse::Discourse::from_config(config)?,
        "duckduckgo_extra" => duckduckgo_extra::DuckDuckGoExtra::from_config(config)?,
        "duckduckgo_web" => duckduckgo_web::DuckDuckGoWeb::from_config(config)?,
        "1337x" => r1337x::_1337x::from_config(config)?,
        "360search" => r360search::Search360::from_config(config)?,
        "artic" => artic::Artic::from_config(config)?,
        "bing_images" => bing_images::BingImages::from_config(config)?,
        "bing_videos" => bing_videos::BingVideos::from_config(config)?,
        "bitchute" => bitchute::Bitchute::from_config(config)?,
        "bpb" => bpb::Bpb::from_config(config)?,
        "braveapi" => braveapi::BraveApi::from_config(config)?,
        "cachy_os" => cachy_os::CachyOs::from_config(config)?,
        "ccc_media" => ccc_media::CccMedia::from_config(config)?,
        "chinaso" => chinaso::ChinaSo::from_config(config)?,
        "cloudflareai" => cloudflareai::CloudflareAi::from_config(config)?,
        "core" => core::Core::from_config(config)?,
        "360search_videos" => r360search_videos::Videoscarch360::from_config(config)?,
        other => {
            return Err(format!(
                "engine {:?} uses the unknown engine type {other:?}",
                config.name
            ));
        }
    };
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn every_default_engine_builds() {
        let settings = Settings::defaults();
        for config in settings.enabled_engines() {
            build(config).unwrap_or_else(|error| {
                panic!("engine {:?} could not be built: {error}", config.name)
            });
        }
    }

    #[test]
    fn unknown_engine_types_are_rejected() {
        let config = EngineConfig::new("nope", "not_an_engine", "np", &["general"]);
        assert!(build(&config).is_err());
    }
}
