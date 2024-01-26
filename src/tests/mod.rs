use crate::{content, session, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
mod test_browser;
mod test_client;
mod test_content;

async fn serve_test_server(addr: String) {
    let state = Arc::new(AppState::new("/tmp/browserlify_unittest".to_string(), 0));

    let app = Router::new()
        .route("/", get(session::create_session))
        .route("/list", get(session::list_session))
        .route("/kill/:session_id", post(session::kill_session))
        .route("/kill_all", post(session::killall_session))
        .route("/pdf", get(content::render_pdf_get))
        .route("/screenshot", get(content::render_screenshot_get))
        .route("/text", get(content::dump_text_get))
        .route("/html", get(content::dump_html_get))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
