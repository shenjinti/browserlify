use axum::{
    routing::{get, post},
    Router,
};
use clap::Parser;
use log;
use session::Session;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tower_http::cors::{Any, CorsLayer};
mod content;
mod devices;
mod error;
mod remote;
mod session;
#[cfg(test)]
mod tests;

#[derive(Parser, Debug)]
#[command(version)]
struct Cli {
    #[clap(long, default_value = "127.0.0.1:9000")]
    addr: String,

    #[clap(long, short, default_value = "0")]
    max_sessions: usize,

    #[clap(long, default_value = "/tmp/browserlify")]
    data_root: String,

    #[clap(long, default_value = "/")]
    prefix: String,

    #[clap(long, default_value = "info")]
    log_level: String,

    #[clap(long, default_value = "false", help = "enable private ip access")]
    enable_private_ip: bool,

    #[clap(long, default_value = "60", help = "max timeout in seconds")]
    max_timeout: u64,

    #[clap(long, default_value = "false", help = "disable cors")]
    disable_cors: bool,

    #[clap(long, help = "default author")]
    author: Option<String>,
}

fn init_log(level: String, is_test: bool) {
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
    Router::new()
        .route("/", get(session::create_session))
        .route("/list", get(session::list_session))
        .route("/kill/:session_id", post(session::kill_session))
        .route("/kill_all", post(session::killall_session))
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
        )
        .route("/firefox/:session_id", get(remote::firefox_remote))
        .route("/chrome/:session_id", get(remote::chrome_remote))
        .with_state(state)
}
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Cli::parse();
    let addr = args.addr;
    let prefix = args.prefix;

    init_log(args.log_level, false);

    let state = Arc::new(AppState {
        data_root: args.data_root,
        max_sessions: args.max_sessions,
        sessions: Arc::new(Mutex::new(Vec::new())),
        enable_private_ip: args.enable_private_ip,
        max_timeout: args.max_timeout,
        author: args.author,
    });

    let mut router = create_router(state);

    if !args.disable_cors {
        router = router.layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any),
        )
    }

    let app = Router::new().nest(&prefix, router);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::warn!("Starting server on {} -> {}", addr, prefix);
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
