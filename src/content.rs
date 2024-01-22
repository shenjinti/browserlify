use crate::{
    devices::get_device,
    session::{create_browser_session, SessionGuard, SessionOption},
    StateRef,
};
use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::Response,
};
use chromiumoxide::{
    cdp::browser_protocol::page::{CaptureScreenshotFormat, PrintToPdfParams, Viewport},
    page::ScreenshotParams,
    Browser, Page,
};
use futures::{Future, StreamExt};
use serde::Deserialize;
use std::time::SystemTime;
use tokio::{select, sync::oneshot, time};

#[derive(Deserialize)]
pub struct RenderParams {
    url: String,

    file_name: Option<String>,
    // total timeout in seconds
    #[serde(rename = "timeout")]
    timeout: Option<u64>,

    wait_load: Option<u64>,
    wait_selector: Option<String>,

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

    #[serde(rename = "javascript")]
    enable_javascript: Option<bool>,

    #[serde(rename = "header_footer")]
    display_header_footer: Option<bool>,

    #[serde(rename = "paper")]
    paper_size: Option<String>,

    format: Option<String>,
    quality: Option<i64>,
    clip: Option<String>,
    full_page: Option<bool>,
}

impl Into<PrintToPdfParams> for RenderParams {
    fn into(self) -> PrintToPdfParams {
        let mut params = PrintToPdfParams::default();
        params.display_header_footer = self.display_header_footer;
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

pub fn can_access(u: url::Url, state: StateRef) -> Result<url::Url, String> {
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err("invalid url".to_string());
    }

    if state.enable_private_ip {
        return Ok(u);
    }
    let host = u.host_str().map(str::to_lowercase).unwrap_or_default();
    if host == "localhost" {
        return Err("not allow localhost".to_string());
    }

    match host.parse::<std::net::IpAddr>() {
        Ok(addr) => match addr {
            std::net::IpAddr::V4(v4_addr) => {
                if v4_addr.is_loopback() {
                    return Err("not allow localhost".to_string());
                }
                if !state.enable_private_ip && v4_addr.is_private() {
                    return Err("not allow private ip".to_string());
                }
            }
            std::net::IpAddr::V6(v6_addr) => {
                if v6_addr.is_loopback() {
                    return Err("not allow localhost".to_string());
                }
            }
        },
        Err(_) => {}
    }
    Ok(u)
}

pub async fn extrace_page<C, Fut>(
    cmd: &str,
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
    callback: C,
) -> Result<Response, String>
where
    C: FnOnce(String, RenderParams, StateRef, Page) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(Vec<u8>, String, Option<String>), String>> + Send + 'static,
{
    let u = url::Url::parse(params.url.as_str())
        .map_err(|e| e.to_string())
        .and_then(|u| can_access(u, state.clone()))?;

    let host = u.host_str().map(str::to_lowercase).unwrap_or_default();
    let st = SystemTime::now();

    let device = get_device(&params.emulating_device.clone().unwrap_or_default());
    let opt = SessionOption::default();

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session = create_browser_session(opt, device, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "window is None")?;
    let mut handler = session.handler.take().ok_or_else(|| "handler is None")?;
    let launch_usage = st.elapsed().unwrap_or_default();
    let st = SystemTime::now();

    let timeout = params
        .timeout
        .unwrap_or(state.max_timeout)
        .max(state.max_timeout);

    let _guard = SessionGuard::new(state.clone(), session);
    let render_loop = async {
        let page = browser
            .new_page(params.url.as_str())
            .await
            .map_err(|e| e.to_string())?;

        if let Some(wait_load) = params.wait_load {
            select! {
                _ = time::sleep(time::Duration::from_secs(wait_load)) => {}
                _ = page.wait_for_navigation() => {}
            };
        } else {
            page.wait_for_navigation().await.ok();
        }

        if let Some(selector) = &params.wait_selector {
            let wait_timeout = params.wait_load.unwrap_or(state.max_timeout / 2);
            select! {
                _ = time::sleep(time::Duration::from_secs(wait_timeout)) => {}
                _ = async {
                    loop {
                    match page.find_element(selector.as_str()).await {
                        Ok(_) => break,
                        Err(_) => {}
                    }
                    time::sleep(time::Duration::from_millis(20)).await;
                }
                } => {}
            }
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

    let (content, content_type, file_name) = r?;
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
    .map_err(|e| e.to_string())
}
pub async fn render_pdf(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, String> {
    extrace_page(
        "pdf",
        Query(params),
        State(state),
        |host, params, _, page| async move {
            let file_name = match &params.file_name {
                Some(name) => name.clone(),
                None => format!("{host}.pdf"),
            };
            let content = page.pdf(params.into()).await.map_err(|e| e.to_string())?;
            Ok((content, "application/pdf".to_string(), Some(file_name)))
        },
    )
    .await
}

pub async fn render_screenshot(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, String> {
    extrace_page(
        "screenshot",
        Query(params),
        State(state),
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

pub async fn dump_text(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, String> {
    extrace_page(
        "text",
        Query(params),
        State(state),
        |_, _, _, page| async move {
            let content: String = page
                .evaluate(
                    "{ let retVal = '';
            if (document.documentElement) {
                retVal = document.documentElement.innerText;
            }
            retVal}",
                )
                .await
                .map_err(|e| e.to_string())?
                .into_value()
                .map_err(|e| e.to_string())?;
            Ok((content.into(), format!("plain/text"), None))
        },
    )
    .await
}

pub async fn dump_html(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, String> {
    extrace_page(
        "html",
        Query(params),
        State(state),
        |_, _, _, page| async move {
            let content = page.content_bytes().await.map_err(|e| e.to_string())?;
            Ok((content.into(), format!("text/html"), None))
        },
    )
    .await
}
