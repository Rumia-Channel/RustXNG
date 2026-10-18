//! Binary entry point: `rustxng [--config FILE] [--bind ADDRESS] [--port PORT]`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use rustxng::aggregator::Searcher;
use rustxng::settings::Settings;
use rustxng::version::version_string;
use rustxng::web::{self, AppState};

const HELP: &str = "rustxng - a Rust rewrite of the SearXNG metasearch engine

USAGE:
    rustxng [OPTIONS]

OPTIONS:
    -c, --config <FILE>    Settings file (default: $SEARXNG_SETTINGS_PATH)
    -b, --bind <ADDRESS>   Address to listen on (default: server.bind_address)
    -p, --port <PORT>      Port to listen on (default: server.port)
    -v, --version          Print the version
    -h, --help             Print this help

ENVIRONMENT:
    SEARXNG_SETTINGS_PATH, SEARXNG_BIND_ADDRESS, SEARXNG_PORT, SEARXNG_SECRET,
    SEARXNG_BASE_URL, SEARXNG_DEBUG, SEARXNG_ENABLE_METRICS, RUST_LOG
";

struct Args {
    config: Option<PathBuf>,
    bind: Option<String>,
    port: Option<u16>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        config: None,
        bind: None,
        port: None,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(argument) = argv.next() {
        match argument.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(None);
            }
            "-v" | "--version" => {
                println!("{}", version_string());
                return Ok(None);
            }
            "-c" | "--config" => {
                args.config = Some(PathBuf::from(
                    argv.next().ok_or("--config needs a file path")?,
                ));
            }
            "-b" | "--bind" => args.bind = Some(argv.next().ok_or("--bind needs an address")?),
            "-p" | "--port" => {
                let value = argv.next().ok_or("--port needs a number")?;
                args.port = Some(value.parse().map_err(|_| "--port needs a number")?);
            }
            other if other.starts_with("--config=") => {
                args.config = Some(PathBuf::from(&other["--config=".len()..]));
            }
            other if other.starts_with("--port=") => {
                args.port = Some(
                    other["--port=".len()..]
                        .parse()
                        .map_err(|_| "--port needs a number")?,
                );
            }
            other => return Err(format!("unknown argument {other:?} (try --help)")),
        }
    }
    Ok(Some(args))
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,rustxng=debug")),
        )
        .init();

    let args = match parse_args() {
        Ok(Some(args)) => args,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let settings_path = args.config.or_else(Settings::path_from_env);
    let settings = match Settings::load(settings_path.as_deref()) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let bind_address = args
        .bind
        .unwrap_or_else(|| settings.server.bind_address.clone());
    let port = args.port.unwrap_or(settings.server.port);
    let address: SocketAddr = match format!("{bind_address}:{port}").parse() {
        Ok(address) => address,
        Err(error) => {
            eprintln!("error: invalid listen address {bind_address}:{port}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let searcher = match Searcher::new(settings) {
        Ok(searcher) => searcher,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(engines = searcher.registry.len(), "{}", version_string());

    let state = AppState::new(searcher);
    let app = web::router(state).layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("error: could not listen on {address}: {error}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!("listening on http://{address}");

    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("error: server failed: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
