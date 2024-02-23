use crate::headless::create_headless_browser_session;
use crate::Error;
use crate::{
    devices::get_device,
    session::{SessionGuard, SessionOption},
    StateRef,
};
use axum::Json;
use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::Response,
};
use chromiumoxide::cdp::browser_protocol::network::{
    EventLoadingFailed, EventLoadingFinished, EventRequestWillBeSent,
};
use chromiumoxide::{
    cdp::browser_protocol::page::{CaptureScreenshotFormat, PrintToPdfParams, Viewport},
    page::ScreenshotParams,
    Browser, Page,
};
use futures::{Future, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::{select, sync::oneshot, time};

#[derive(Deserialize)]
pub struct RenderParams {
    url: String,

    file_name: Option<String>,
    // total timeout in milliseconds
    #[serde(rename = "timeout")]
    timeout: Option<u64>,

    wait_load: Option<u64>,
    #[serde(rename = "selector")]
    wait_selector: Option<String>,

    #[serde(rename = "images")]
    wait_images: Option<bool>,

    #[serde(rename = "network_idle")]
    wait_network_idle: Option<u64>,

    #[serde(rename = "page_ready")]
    wait_page_ready: Option<bool>,
    scroll_bottom: Option<u64>, // scroll to bottom before render, in seconds
    scroll_interval: Option<u64>,

    #[serde(rename = "width")]
    paper_width: Option<f64>,
    #[serde(rename = "height")]
    paper_height: Option<f64>,
    scale: Option<f64>,

    #[serde(rename = "top")]
    margin_top: Option<f64>,
    #[serde(rename = "bottom")]
    margin_bottom: Option<f64>,
    #[serde(rename = "left")]
    margin_left: Option<f64>,
    #[serde(rename = "right")]
    margin_right: Option<f64>,

    #[serde(rename = "background")]
    print_background: Option<bool>,
    landscape: Option<bool>,
    page_ranges: Option<String>,

    #[serde(rename = "device")]
    emulating_device: Option<String>,

    #[serde(rename = "disable_link")]
    disable_link: Option<bool>,

    #[serde(rename = "paper")]
    paper_size: Option<String>,

    #[serde(rename = "header")]
    header_template: Option<String>,
    #[serde(rename = "footer")]
    footer_template: Option<String>,

    format: Option<String>,
    quality: Option<i64>,
    clip: Option<String>,
    full_page: Option<bool>,

    author: Option<String>,
}

impl Into<PrintToPdfParams> for RenderParams {
    fn into(self) -> PrintToPdfParams {
        let mut params = PrintToPdfParams::default();

        match self.paper_size.unwrap_or_default().to_lowercase().as_str() {
            "a3" => {
                params.paper_width = Some(11.69);
                params.paper_height = Some(16.54);
            }
            "a5" => {
                params.paper_width = Some(5.83);
                params.paper_height = Some(8.27);
            }
            "legal" => {
                params.paper_width = Some(8.5);
                params.paper_height = Some(14.0);
            }
            "letter" => {
                params.paper_width = Some(8.5);
                params.paper_height = Some(11.0);
            }
            _ => {
                // A4
                params.paper_width = Some(8.27);
                params.paper_height = Some(11.69);
            }
        }

        params.print_background = self.print_background;
        params.landscape = self.landscape;
        params.page_ranges = self.page_ranges;

        params.margin_top = self.margin_top;
        params.margin_bottom = self.margin_bottom;
        params.margin_left = self.margin_left;
        params.margin_right = self.margin_right;

        params.scale = self.scale;
        params.paper_width = self.paper_width;
        params.paper_height = self.paper_height;

        if self.header_template.is_some() {
            params.display_header_footer = Some(true);
            params.header_template = self.header_template;
        }

        if self.footer_template.is_some() {
            params.display_header_footer = Some(true);
            params.footer_template = self.footer_template;
        }
        params
    }
}
impl Into<ScreenshotParams> for RenderParams {
    fn into(self) -> ScreenshotParams {
        let mut params = ScreenshotParams::builder();
        let format = match self.format.as_deref() {
            Some("jpeg") => CaptureScreenshotFormat::Jpeg,
            Some("png") => CaptureScreenshotFormat::Png,
            Some("webp") => CaptureScreenshotFormat::Webp,
            _ => CaptureScreenshotFormat::Png,
        };

        if let Some(clip) = self.clip {
            let mut parts = clip.split(',').map(str::parse::<f64>).map(Result::unwrap);
            let x = parts.next().unwrap_or(0.0);
            let y = parts.next().unwrap_or(0.0);
            let width = parts.next().unwrap_or(0.0);
            let height = parts.next().unwrap_or(0.0);

            let viewport = Viewport::builder()
                .x(x)
                .y(y)
                .width(width)
                .height(height)
                .build()
                .unwrap();
            params = params.clip(viewport);
        }

        params
            .format(format)
            .quality(self.quality.unwrap_or_default())
            .full_page(self.full_page.unwrap_or_default())
            .build()
    }
}

pub fn can_access(u: url::Url, state: StateRef) -> Result<url::Url, Error> {
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err(Error::new(
            StatusCode::BAD_REQUEST,
            "only support http/https",
        ));
    }

    if state.enable_private_ip {
        return Ok(u);
    }
    let host = u.host_str().map(str::to_lowercase).unwrap_or_default();
    if host == "localhost" {
        return Err(Error::new(StatusCode::BAD_REQUEST, "not allow localhost"));
    }

    match host.parse::<std::net::IpAddr>() {
        Ok(addr) => {
            if addr.is_loopback() {
                return Err(Error::new(
                    StatusCode::BAD_REQUEST,
                    "not allow loopback address",
                ));
            }
            match addr {
                std::net::IpAddr::V4(v4_addr) => {
                    if !state.enable_private_ip && v4_addr.is_private() {
                        return Err(Error::new(StatusCode::BAD_REQUEST, "not allow private ip"));
                    }
                }
                _ => {}
            }
        }
        Err(_) => {}
    }
    Ok(u)
}

// wait for network idle
// 1. inspect network request
// 2. wait for all request finished
// 3. wait for some time
async fn wait_page_network_idle(page: Page, timeout: Duration) -> Result<bool, Error> {
    let mut request_will_be_sent = page.event_listener::<EventRequestWillBeSent>().await?;
    let mut request_loading_finished = page.event_listener::<EventLoadingFinished>().await?;
    let mut request_loading_failed = page.event_listener::<EventLoadingFailed>().await?;
    let mut requests = HashMap::new();

    let mut last_request_time = None;

    loop {
        select! {
            event = request_will_be_sent.next() => {
                last_request_time = None;
                if let Some(event) = event {
                    requests.insert(event.request_id.clone(), event.request.url.clone());
                }
            }
            event = request_loading_finished.next() => {
                last_request_time = Some(SystemTime::now());
                if let Some(event) = event {
                    requests.remove(&event.request_id);
                }
            }
            event = request_loading_failed.next() => {
                last_request_time = Some(SystemTime::now());
                if let Some(event) = event {
                    requests.remove(&event.request_id);
                }
            }
            _ = time::sleep(timeout) => {
            }
        }
        if let Some(last_request_time) = last_request_time {
            if requests.is_empty() && last_request_time.elapsed().unwrap_or_default() > timeout {
                break;
            }
        }
    }
    Ok(requests.is_empty())
}

async fn scroll_to_bottom(page: Page, timeout: u64, scroll_interval: u64) {
    // get scroll height
    match page
        .evaluate("document.body.scrollHeight")
        .await
        .map(|v| v.into_value::<i64>())
    {
        Ok(Ok(scroll_height)) => {
            let mut total_times = ((timeout / scroll_interval) as usize).max(1);
            let scroll_step = scroll_height / total_times as i64;

            while total_times > 0 {
                total_times -= 1;
                let current = scroll_height - scroll_step * total_times as i64;
                page.evaluate(format!("window.scrollTo(0, {});", current))
                    .await
                    .ok();
                time::sleep(time::Duration::from_millis(scroll_interval)).await;
            }
        }
        _ => {}
    }
}

async fn scroll_to_top(page: Page, check_interval: u64) {
    page.evaluate("window.scrollTo(0, 0);").await.ok();
    loop {
        match page
            .evaluate("window.scrollY")
            .await
            .map(|v| v.into_value::<i64>())
        {
            Ok(Ok(scroll_y)) => {
                if scroll_y == 0 {
                    break;
                }
            }
            _ => break,
        }
        time::sleep(time::Duration::from_millis(check_interval)).await;
    }
}

async fn wait_images_loaded(page: Page, check_interval: u64) {
    loop {
        match page
            .evaluate("document.images.length")
            .await
            .map(|r| r.into_value::<i64>())
        {
            Ok(Ok(v)) => {
                if v == 0 {
                    break;
                }
                match page
                    .evaluate("Array.from(document.images).every(i => i.complete)")
                    .await
                    .map(|v| v.into_value::<bool>())
                {
                    Ok(Ok(done)) => {
                        if done {
                            break;
                        }
                    }
                    _ => break,
                }
            }
            _ => break,
        }
        time::sleep(time::Duration::from_millis(check_interval)).await;
    }
}

async fn wait_page_ready(page: Page, check_interval: u64) {
    loop {
        match page.evaluate("document.readyState").await {
            Ok(v) => {
                if v.into_value::<String>().unwrap_or_default() == "complete" {
                    return;
                }
            }
            Err(_) => {}
        }
        time::sleep(time::Duration::from_millis(check_interval)).await;
    }
}

pub async fn extrace_page<C, Fut>(
    cmd: &str,
    params: RenderParams,
    state: StateRef,
    callback: C,
) -> Result<Response, Error>
where
    C: FnOnce(String, RenderParams, StateRef, Page) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(Vec<u8>, String, Option<String>), String>> + Send + 'static,
{
    let u = url::Url::parse(params.url.as_str())
        .map_err(|e| Error::new(StatusCode::BAD_REQUEST, &e.to_string()))
        .and_then(|u| can_access(u, state.clone()))?;

    let host = u.host_str().map(str::to_lowercase).unwrap_or_default();
    let st = SystemTime::now();

    let device = get_device(&params.emulating_device.clone().unwrap_or_default());
    let opt = SessionOption::default();

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session =
        create_headless_browser_session(opt, device, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "window is None")?;
    let mut handler = session
        .headless_handler
        .take()
        .ok_or_else(|| "handler is None")?;
    let launch_usage = st.elapsed().unwrap_or_default();
    let st = SystemTime::now();

    let timeout = params
        .timeout
        .unwrap_or(state.max_timeout)
        .max(state.max_timeout);

    const SLEEP_INTERVAL: u64 = 50;

    let _guard = SessionGuard::new(state.clone(), session);
    let render_loop = async {
        let page = browser
            .new_page(params.url.as_str())
            .await
            .map_err(|e| e.to_string())?;

        let wait_something = async {
            // 1. wait for network idle
            // 2. wait for selector
            // 3. wait for images
            if let Some(timeout) = params.wait_network_idle {
                match wait_page_network_idle(page.clone(), Duration::from_millis(timeout)).await {
                    Ok(done) => {
                        if !done {
                            log::warn!("{} {} wait network idle timeout", cmd, params.url);
                        }
                    }
                    Err(e) => {
                        log::error!("{} {} wait network idle, error: {}", cmd, params.url, e);
                    }
                }
            }

            page.wait_for_navigation().await.ok();

            if params.wait_page_ready.is_some() {
                wait_page_ready(page.clone(), SLEEP_INTERVAL).await;
            }

            if params.scroll_bottom.is_some() || params.wait_images.is_some() {
                let scroll_bottom = params.scroll_bottom.unwrap_or_default();
                scroll_to_bottom(
                    page.clone(),
                    scroll_bottom,
                    params.scroll_interval.unwrap_or(200),
                )
                .await;

                if params.wait_images.is_some() && scroll_bottom == 0 {
                    // reset scroll to top
                    select! {
                        _ = scroll_to_top(page.clone(), 1000) => {}
                        _ = time::sleep(time::Duration::from_millis(500)) => {}
                    }
                }
            }

            if let Some(selector) = &params.wait_selector {
                loop {
                    match page.find_element(selector.as_str()).await {
                        Ok(_) => break,
                        Err(_) => {}
                    }
                    time::sleep(time::Duration::from_millis(SLEEP_INTERVAL)).await;
                }
                log::info!("{} {} wait {selector} done ", cmd, params.url);
            }

            if params.wait_images.unwrap_or_default() {
                wait_images_loaded(page.clone(), SLEEP_INTERVAL).await;
            }
        };

        let wait_timeout = params.wait_load.unwrap_or(15 * 1000).min(state.max_timeout); // 15 seconds
        log::info!(
            "{} {} wait load timeout: {:?} selector:{:?} wait_load:{:?}",
            cmd,
            params.url,
            wait_timeout,
            params.wait_selector,
            params.wait_load
        );
        select! {
            _ = time::sleep(time::Duration::from_millis(wait_timeout)) => {
                log::warn!("{} {} wait load timeout wait_load:{:?} selector:{:?} images:{:?} network_idle:{:?} page_ready:{:?}", cmd,
                params.url, params.wait_load, params.wait_selector,
                params.wait_images, params.wait_network_idle, params.wait_page_ready);
            }
            _ = wait_something => {}
        }
        callback(host.to_string(), params, state, page).await
    };

    let r = select! {
        r = render_loop => {
            r
        },
        _ = async {
            while let Some(_) = handler.next().await {}
        } => { Err("handler exit".to_string()) }
        _ = shutdown_rx => {
            Err("session cancel".to_string())
        },
        _ = async { time::sleep(time::Duration::from_secs(timeout)).await } => {
            Err("timeout".to_string())
        },
    };

    browser.kill().await;

    let (content, content_type, file_name) =
        r.map_err(|e| Error::new(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type);
    let extract_usage = st.elapsed().unwrap_or_default();

    log::info!(
        "{} url: {}, launch: {:?}, extract: {:?}",
        cmd,
        u,
        launch_usage,
        extract_usage
    );

    match file_name {
        Some(name) => resp.header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", name),
        ),
        None => resp,
    }
    .body(Body::from(content))
    .map_err(|e| Error::new(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))
}

pub async fn render_pdf_get(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, Error> {
    render_pdf(params, state).await
}

pub async fn render_pdf_post(
    State(state): State<StateRef>,
    Json(body): Json<RenderParams>,
) -> Result<Response, Error> {
    render_pdf(body, state).await
}

async fn render_pdf(params: RenderParams, state: StateRef) -> Result<Response, Error> {
    let author = match &params.author {
        Some(author) => author.clone(),
        None => match &state.author {
            Some(author) => author.clone(),
            None => "Browserlify".to_string(),
        },
    };

    extrace_page("pdf", params, state, |host, params, _, page| async move {
        let file_name = match &params.file_name {
            Some(name) => name.clone(),
            None => format!("{host}.pdf"),
        };

        if params.disable_link.unwrap_or_default() {
            page.evaluate(
                "document.querySelectorAll('a').forEach((el) => el.setAttribute('href', '#'))",
            )
            .await
            .map_err(|e| e.to_string())?;
        }

        let content = page.pdf(params.into()).await.map_err(|e| e.to_string())?;
        let content = match lopdf::Document::load_mem(&content) {
            Ok(mut doc) => {
                let mut info = lopdf::Dictionary::new();
                info.set(
                    "Author",
                    lopdf::Object::String(author.into(), lopdf::StringFormat::Literal),
                );
                let value = doc.add_object(lopdf::Object::Dictionary(info));
                doc.trailer.set("Info", value);

                let mut new_content = Vec::new();
                match doc.save_to(&mut new_content) {
                    Ok(_) => new_content,
                    Err(e) => {
                        log::error!("pdf save error: {}", e);
                        content
                    }
                }
            }
            Err(_) => content,
        };
        Ok((content, "application/pdf".to_string(), Some(file_name)))
    })
    .await
}

pub async fn render_screenshot_get(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, Error> {
    render_screenshot(params, state).await
}

pub async fn render_screenshot_post(
    State(state): State<StateRef>,
    Json(body): Json<RenderParams>,
) -> Result<Response, Error> {
    render_screenshot(body, state).await
}

async fn render_screenshot(params: RenderParams, state: StateRef) -> Result<Response, Error> {
    extrace_page(
        "screenshot",
        params,
        state,
        |host, params, _, page| async move {
            let file_ext = match &params.format {
                Some(format) => format.clone(),
                None => "png".to_string(),
            };
            let file_name = match &params.file_name {
                Some(name) => name.clone(),
                None => format!("{host}.{file_ext}"),
            };
            let params: ScreenshotParams = params.into();
            let content = page.screenshot(params).await.map_err(|e| e.to_string())?;
            Ok((content, format!("image/{file_ext}"), Some(file_name)))
        },
    )
    .await
}

pub async fn dump_text_get(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, Error> {
    dump_text(params, state).await
}
pub async fn dump_text_post(
    State(state): State<StateRef>,
    Json(body): Json<RenderParams>,
) -> Result<Response, Error> {
    dump_text(body, state).await
}

async fn dump_text(params: RenderParams, state: StateRef) -> Result<Response, Error> {
    extrace_page("text", params, state, |_, _, _, page| async move {
        let content: String = page
            .evaluate("document.documentElement? document.documentElement.innerText:''")
            .await
            .map_err(|e| e.to_string())?
            .into_value()
            .map_err(|e| e.to_string())?;
        Ok((content.into(), format!("plain/text"), None))
    })
    .await
}

pub async fn dump_html_get(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, Error> {
    dump_html(params, state).await
}

pub async fn dump_html_post(
    State(state): State<StateRef>,
    Json(body): Json<RenderParams>,
) -> Result<Response, Error> {
    dump_html(body, state).await
}

async fn dump_html(params: RenderParams, state: StateRef) -> Result<Response, Error> {
    extrace_page("html", params, state, |_, _, _, page| async move {
        let content = page.content_bytes().await.map_err(|e| e.to_string())?;
        Ok((content.into(), format!("text/html"), None))
    })
    .await
}
