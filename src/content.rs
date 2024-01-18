use crate::{
    session::{create_browser_session, SessionGuard, SessionOption},
    StateRef,
};
use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chromiumoxide::{
    cdp::browser_protocol::page::{CaptureScreenshotFormat, PrintToPdfParams, Viewport},
    page::ScreenshotParams,
    Browser,
};
use futures::StreamExt;
use serde::Deserialize;
use tokio::{select, sync::oneshot};

#[allow(unused)]
#[derive(Deserialize)]
pub struct RenderParams {
    url: String,

    file_name: Option<String>,
    // total timeout in seconds
    #[serde(rename = "timeout")]
    timeout: Option<u32>,

    wait_load: Option<u32>,
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
            let mut parts = clip.split(',');
            let x = parts
                .next()
                .unwrap_or_default()
                .parse::<f64>()
                .unwrap_or_default();
            let y = parts
                .next()
                .unwrap_or_default()
                .parse::<f64>()
                .unwrap_or_default();
            let width = parts
                .next()
                .unwrap_or_default()
                .parse::<f64>()
                .unwrap_or_default();
            let height = parts
                .next()
                .unwrap_or_default()
                .parse::<f64>()
                .unwrap_or_default();

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

pub async fn render_pdf(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<Response, String> {
    log::warn!("render pdf: {:?}", params.url);
    let u = url::Url::parse(params.url.as_str())
        .map_err(|e| e.to_string())
        .and_then(|u| can_access(u, state.clone()))?;

    let host = u.host_str().unwrap_or_default();

    let opt = SessionOption::default();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session = create_browser_session(opt, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "window is None")?;
    let mut handler = session.handler.take().ok_or_else(|| "handler is None")?;

    let id = session.id.clone();
    let _guard = SessionGuard::new(state.clone(), id.clone(), session);

    let file_name = match &params.file_name {
        Some(name) => name.clone(),
        None => format!("{}.pdf", host),
    };

    let file_name_ref = file_name.clone();
    let render_loop = async {
        let page = browser
            .new_page(params.url.as_str())
            .await
            .map_err(|e| e.to_string())?;

        page.wait_for_navigation()
            .await
            .map_err(|e| e.to_string())?;

        page.save_pdf(params.into(), file_name_ref)
            .await
            .map_err(|e| e.to_string())
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
        }
    };

    browser.kill().await;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/pdf")
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", file_name),
        )
        .body(Body::from(r.unwrap()))
        .map_err(|e| e.to_string())
}

pub async fn render_screenshot(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<impl IntoResponse, String> {
    let u = url::Url::parse(params.url.as_str())
        .map_err(|e| e.to_string())
        .and_then(|u| can_access(u, state.clone()))?;

    let opt = SessionOption::default();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session = create_browser_session(opt, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "window is None")?;
    let mut handler = session.handler.take().ok_or_else(|| "handler is None")?;

    let id = session.id.clone();
    let _guard = SessionGuard::new(state.clone(), id.clone(), session);

    let file_format = match &params.format {
        Some(format) => format.clone(),
        None => "png".to_string(),
    };

    let host = u.host_str().unwrap_or_default();
    let file_name = match &params.file_name {
        Some(name) => name.clone(),
        None => format!("{host}.{file_format}"),
    };

    let render_loop = async {
        let page = browser
            .new_page(params.url.as_str())
            .await
            .map_err(|e| e.to_string())?;

        page.wait_for_navigation()
            .await
            .map_err(|e| e.to_string())?;

        let params: ScreenshotParams = params.into();
        page.screenshot(params).await.map_err(|e| e.to_string())
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
        }
    };

    browser.kill().await;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", format!("image/{}", file_format))
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", file_name),
        )
        .body(Body::from(r.unwrap()))
        .map_err(|e| e.to_string())
}

pub async fn dump_text(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<impl IntoResponse, String> {
    let _ = url::Url::parse(params.url.as_str())
        .map_err(|e| e.to_string())
        .and_then(|u| can_access(u, state.clone()))?;

    let opt = SessionOption::default();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session = create_browser_session(opt, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "window is None")?;
    let mut handler = session.handler.take().ok_or_else(|| "handler is None")?;

    let id = session.id.clone();
    let _guard = SessionGuard::new(state.clone(), id.clone(), session);

    let render_loop = async {
        let page = browser
            .new_page(params.url.as_str())
            .await
            .map_err(|e| e.to_string())?;

        page.wait_for_navigation()
            .await
            .map_err(|e| e.to_string())?;

        page.content().await.map_err(|e| e.to_string())
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
        }
    };

    browser.kill().await;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "plain/text")
        .body(Body::from(r.unwrap_or_default()))
        .map_err(|e| e.to_string())
}

pub async fn dump_html(
    Query(params): Query<RenderParams>,
    State(state): State<StateRef>,
) -> Result<impl IntoResponse, String> {
    let _ = url::Url::parse(params.url.as_str())
        .map_err(|e| e.to_string())
        .and_then(|u| can_access(u, state.clone()))?;

    let opt = SessionOption::default();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session = create_browser_session(opt, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "window is None")?;
    let mut handler = session.handler.take().ok_or_else(|| "handler is None")?;

    let id = session.id.clone();
    let _guard = SessionGuard::new(state.clone(), id.clone(), session);

    let render_loop = async {
        let page = browser
            .new_page(params.url.as_str())
            .await
            .map_err(|e| e.to_string())?;

        page.wait_for_navigation()
            .await
            .map_err(|e| e.to_string())?;

        page.content().await.map_err(|e| e.to_string())
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
        }
    };

    browser.kill().await;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(r.unwrap_or_default()))
        .map_err(|e| e.to_string())
}
