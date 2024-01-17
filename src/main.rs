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

mod session;

#[derive(Parser, Debug)]
#[command(version)]
struct Cli {
    #[clap(long, default_value = "127.0.0.1:9000")]
    addr: Option<String>,

    #[clap(long, short)]
    max_sessions: Option<usize>,

    #[clap(long, default_value = "/tmp/chrome_server")]
    data_root: Option<String>,

    #[clap(long, default_value = "/")]
    prefix: Option<String>,

    #[clap(long, default_value = "info")]
    log_level: Option<String>,
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
}

impl AppState {
    pub fn new(data_root: String, max_sessions: usize) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            max_sessions,
            data_root,
        }
    }
    pub fn is_full(&self) -> bool {
        if self.max_sessions <= 0 {
            return false;
        }
        self.sessions.lock().unwrap().len() >= self.max_sessions
    }
}
type StateRef = Arc<AppState>;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Cli::parse();
    let addr = args.addr.unwrap_or_default();
    let prefix = args.prefix.unwrap_or_default();

    init_log(args.log_level.unwrap_or_default(), false);

    let state = Arc::new(AppState::new(
        args.data_root.unwrap_or_default(),
        args.max_sessions.unwrap_or_default(),
    ));
    let router = Router::new()
        .route("/", get(session::create_session))
        .route("/list", get(session::list_session))
        .route("/kill/:session_id", post(session::kill_session))
        .route("/kill_all", post(session::killall_session))
        .with_state(state);

    let app = Router::new().nest(&prefix, router);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::warn!("Starting server on {} -> {}", addr, prefix);
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
