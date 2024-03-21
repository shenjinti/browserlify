use axum::{
    http::{Request, Response},
    routing::{get, post},
    Router,
};
use clap::Parser;
use log;
use session::Session;
use std::{
    io::Write,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::select;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::Span;
#[cfg(feature = "content")]
mod content;
mod devices;
mod error;
#[cfg(feature = "headless")]
mod headless;
#[cfg(feature = "remote")]
mod remote;
mod session;
#[cfg(test)]
mod tests;
pub use error::Error;

#[derive(Parser, Debug)]
#[command(version)]
struct Cli {
    #[clap(long, default_value = "0.0.0.0:9000")]
    addr: String,

    #[clap(long, short, default_value = "0")]
    max_sessions: usize,

    #[clap(long, default_value = "/tmp/browserlify")]
    data_root: String,

    #[clap(long, default_value = "/")]
    prefix: String,

    #[clap(long, default_value = "info")]
    log_level: String,

    #[clap(long, help = "log file path")]
    log_file: Option<String>,

    #[clap(long, default_value = "false", help = "enable private ip access")]
    enable_private_ip: bool,

    #[clap(
        long,
        default_value = "60000",
        help = "max session timeout in milliseconds"
    )]
    max_timeout: u64,

    #[clap(long, default_value = "false", help = "disable CORS access control")]
    disable_cors: bool,

    #[clap(long, help = "default author")]
    author: Option<String>,

    #[clap(long, help = "disable background cleanup", default_value = "false")]
    disable_background_cleanup: bool,
}

fn init_log(level: String, is_test: bool, log_file_name: Option<String>) {
    let target = match log_file_name
        .map(|log_file| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
                .ok()
        })
        .unwrap_or_default()
    {
        Some(log_file) => Box::new(log_file),
        None => Box::new(std::io::stdout()) as Box<dyn std::io::Write + Send>,
    };

    let _ = env_logger::builder()
        .is_test(is_test)
        .format(|buf, record| {
            let short_file_name = record
                .file()
                .unwrap_or("unknown")
                .split('/')
                .last()
                .unwrap_or("unknown");

            writeln!(
                buf,
                "{} [{}] {}:{} - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                short_file_name,
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(target))
        .format_timestamp(None)
        .filter_level(level.parse().unwrap())
        .try_init();
}

#[derive(Clone)]
pub struct AppState {
    sessions: Arc<Mutex<Vec<Session>>>,
    max_sessions: usize,
    data_root: String,
    enable_private_ip: bool,
    max_timeout: u64,
    author: Option<String>,
}

impl AppState {
    pub fn new(data_root: String, max_sessions: usize) -> Self {
        AppState {
            sessions: Arc::new(Mutex::new(Vec::new())),
            max_sessions,
            data_root,
            enable_private_ip: false,
            max_timeout: 60 * 1000, // 60 seconds
            author: None,
        }
    }

    pub fn allow_private_ip(&self) -> Self {
        let mut state = self.clone();
        state.enable_private_ip = true;
        state
    }

    pub fn is_full(&self) -> bool {
        if self.max_sessions <= 0 {
            return false;
        }
        self.sessions.lock().unwrap().len() >= self.max_sessions
    }
}
type StateRef = Arc<AppState>;

fn create_router(state: StateRef) -> Router {
    let router = Router::new()
        .route("/list", get(session::list_session))
        .route("/kill/:session_id", post(session::kill_session))
        .route("/screen/:session_id", get(session::screen_session))
        .route("/kill_all", post(session::killall_session))
        .nest_service("/assets", ServeDir::new("dist/assets"));

    #[cfg(feature = "headless")]
    let router = router.route("/", get(headless::create_headless_session));

    #[cfg(not(feature = "headless"))]
    let router = router.route("/", get(handle_index_page));
    #[cfg(feature = "content")]
    let router = router
        .route(
            "/pdf",
            get(content::render_pdf_get).post(content::render_pdf_post),
        )
        .route(
            "/screenshot",
            get(content::render_screenshot_get).post(content::render_screenshot_post),
        )
        .route(
            "/text",
            get(content::dump_text_get).post(content::dump_text_post),
        )
        .route(
            "/html",
            get(content::dump_html_get).post(content::dump_html_post),
        );

    #[cfg(feature = "remote")]
    let router = router
        .nest(
            "/remote",
            Router::new()
                .route("/connect/:remote_id", get(remote::connect_remote))
                .route("/edit/:remote_id", post(remote::edit_remote))
                .route("/stop/:remote_id", post(remote::stop_remote))
                .route("/start/:remote_id", post(remote::start_remote))
                .route("/remove/:remote_id", post(remote::remove_remote))
                .route("/list", post(remote::list_remote))
                .route("/create", post(remote::create_remote)),
        )
        .with_state(state.clone());
    router.with_state(state)
}
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Cli::parse();
    let addr = args.addr;
    let prefix = args.prefix;

    init_log(args.log_level, false, args.log_file);

    let state = Arc::new(AppState {
        data_root: args.data_root,
        max_sessions: args.max_sessions,
        sessions: Arc::new(Mutex::new(Vec::new())),
        enable_private_ip: args.enable_private_ip,
        max_timeout: args.max_timeout,
        author: args.author,
    });

    let mut router = create_router(state.clone());

    if !args.disable_cors {
        router = router.layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any),
        )
    }

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init()
        .ok();

    let app = Router::new().nest(&prefix, router).layer(
        tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                let remote_ip = request
                    .headers()
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.split(',').next())
                    .and_then(|v| v.parse().ok())
                    .or_else(|| {
                        request
                            .extensions()
                            .get::<SocketAddr>()
                            .copied()
                            .map(|v| v.to_string())
                    })
                    .unwrap_or_default();
                tracing::info_span!(
                    "HTTP",
                    method = %request.method(),
                    uri = %request.uri(),
                    remote = %remote_ip,
                )
            })
            .on_request(())
            .on_response(|response: &Response<_>, latency: Duration, _span: &Span| {
                let content_length = response
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or_default();

                tracing::info!(
                    latency = ?latency,
                    content_length = %content_length,
                    status = %response.status().as_u16(),
                );
            }),
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::warn!("Starting server on {} -> {}", addr, prefix);

    if args.disable_background_cleanup {
        axum::serve(listener, app).await
    } else {
        select! {
            r = session::background_cleanup_task(state.clone()) => {r}
            r = async { axum::serve(listener, app).await } => {r}
        }
    }
}
