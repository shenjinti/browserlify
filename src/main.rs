use clap::Parser;
use log;
use session::ChromeSession;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

mod debugger;
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

    #[clap(long, default_value = "/api")]
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
pub struct State {
    sessions: Arc<Mutex<Vec<ChromeSession>>>,
    max_sessions: usize,
    data_root: String,
}

impl State {
    pub fn new(data_root: String, max_sessions: usize) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            max_sessions,
            data_root,
        }
    }
}

type Request = tide::Request<State>;

#[async_std::main]
async fn main() -> tide::Result<()> {
    let args = Cli::parse();
    let addr = args.addr.unwrap_or_default();
    let prefix = args.prefix.unwrap_or_default();

    init_log(args.log_level.unwrap_or_default(), false);

    let mut app = tide::with_state(State::new(
        args.data_root.unwrap_or_default(),
        args.max_sessions.unwrap_or_default(),
    ));
    let mut r = app.at(&prefix.trim_end_matches('/'));

    r.get(session::new_session);
    r.at("/list").get(session::list_session);
    r.at("/kill/:id").post(session::kill_session);
    r.at("/kill_all").post(session::killall_session);

    log::warn!("Starting server on {} -> {}", addr, prefix);
    app.listen(addr).await?;

    Ok(())
}
