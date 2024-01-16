use tide::{
    convert::{Deserialize, Serialize},
    http::url,
};

use crate::Request;
use async_std::{io::WriteExt, net::TcpStream};
use std::{sync::Arc, time::SystemTime};
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ChromeSession {
    token: String,
    ws_url: String,
    created_at: SystemTime,
    data_dir: String,
}

impl ChromeSession {
    pub async fn new_async(data_root: &str) -> Self {
        let data_dir = format!("{}/{}", data_root, uuid::Uuid::new_v4());
        Self {
            token: "".to_string(),
            ws_url: "".to_string(),
            created_at: SystemTime::now(),
            data_dir,
        }
    }
}

/// Workflow
/// 1. render index.html when GET / and request not websocket
/// 2. create an chrome session when websocket request    
///     2.1 bridge websocket to chrome session
pub(crate) async fn new_session(req: Request) -> tide::Result {
    match req.header("Upgrade").map(|values| {
        values
            .iter()
            .any(|value| value.as_str().eq_ignore_ascii_case("websocket"))
    }) {
        Some(true) => {}
        _ => {
            // render index.html
            return Ok("/api/list to show sessions".into());
        }
    };
    let state = req.state();
    let session = ChromeSession::new_async(&state.data_root).await;
    let ws_url = url::Url::parse(&session.ws_url)?;

    let mut upstream = match ws_url.socket_addrs(|| None)?.first() {
        Some(addr) => TcpStream::connect(addr).await?,
        None => return Ok("invalid ws_url".into()),
    };
    state.sessions.lock().unwrap().push(session);

    // incoming_req -> upstream -> ws_url
    // dump req to upstream
    let buf = dump_request(req);
    upstream.write_all(buf).await?;

    let server_to_client = async {
        while let Some(msg) = server_ws.next().await {
            let msg = msg?;
            client_ws.send(msg).await?;
        }
        Ok(())
    };

    let client_to_server = async {
        while let Some(msg) = client_ws.next().await {
            let msg = msg?;
            server_ws.send(msg).await?;
        }
        Ok(())
    };

    futures::try_join!(server_to_client, client_to_server)
}

fn dump_request(req: tide::Request<crate::State>) -> &[u8] {
    let mut buf = Vec::new();
    let mut headers = Vec::new();
    for (name, value) in req.iter() {
        headers.push((name.as_str(), value.as_str()));
    }
    let _ = write!(
        buf,
        "{} {} {:?}\r\n",
        req.method(),
        req.url(),
        req.version()
    );
    for (name, value) in headers {
        let _ = write!(buf, "{}: {}\r\n", name, value);
    }
    let _ = write!(buf, "\r\n");
    buf.as_slice()
}

pub(crate) async fn list_session(req: Request) -> tide::Result {
    let state = req.state();
    let sessions = state.sessions.lock().unwrap();
    let body = serde_json::to_string(&*sessions)?;
    Ok(body.into())
}

pub(crate) async fn kill_session(req: Request) -> tide::Result {
    let session_id = req.param("id")?;
    Ok(format!("kill_session {session_id}").into())
}

pub(crate) async fn killall_session(_req: Request) -> tide::Result {
    Ok("killall_session".into())
}
