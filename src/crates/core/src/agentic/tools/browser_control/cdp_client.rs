//! Lightweight CDP (Chrome DevTools Protocol) client over WebSocket.

use crate::util::errors::{BitFunError, BitFunResult};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

/// Information about a single browser page/tab from the CDP `/json` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpPageInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
    #[serde(rename = "type")]
    pub page_type: Option<String>,
}

/// Version info returned by `/json/version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpVersionInfo {
    #[serde(rename = "Browser")]
    pub browser: Option<String>,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: Option<String>,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
}

/// A CDP WebSocket client connected to a single page target.
pub struct CdpClient {
    sink: Arc<Mutex<WsSink>>,
    pending: Arc<RwLock<HashMap<i64, tokio::sync::oneshot::Sender<Value>>>>,
    next_id: AtomicI64,
    _reader_handle: tokio::task::JoinHandle<()>,
}

impl CdpClient {
    /// Discover browser version on the given debug port.
    pub async fn get_version(port: u16) -> BitFunResult<CdpVersionInfo> {
        let url = format!("http://127.0.0.1:{}/json/version", port);
        let resp = reqwest::get(&url).await.map_err(|e| {
            BitFunError::tool(format!("Cannot reach browser CDP on port {}: {}", port, e))
        })?;
        let info: CdpVersionInfo = resp
            .json()
            .await
            .map_err(|e| BitFunError::tool(format!("Invalid CDP version response: {}", e)))?;
        Ok(info)
    }

    /// List all pages/tabs on the given debug port.
    pub async fn list_pages(port: u16) -> BitFunResult<Vec<CdpPageInfo>> {
        let url = format!("http://127.0.0.1:{}/json", port);
        let resp = reqwest::get(&url).await.map_err(|e| {
            BitFunError::tool(format!("Cannot list CDP pages on port {}: {}", port, e))
        })?;
        let pages: Vec<CdpPageInfo> = resp
            .json()
            .await
            .map_err(|e| BitFunError::tool(format!("Invalid CDP pages response: {}", e)))?;
        Ok(pages)
    }

    /// Connect to a specific page by its WebSocket debugger URL.
    pub async fn connect(ws_url: &str) -> BitFunResult<Self> {
        info!("CDP connecting to {}", ws_url);
        let (ws_stream, _) = connect_async(ws_url)
            .await
            .map_err(|e| BitFunError::tool(format!("CDP WebSocket connect failed: {}", e)))?;

        let (sink, stream) = ws_stream.split();
        let sink = Arc::new(Mutex::new(sink));
        let pending: Arc<RwLock<HashMap<i64, tokio::sync::oneshot::Sender<Value>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let pending_clone = pending.clone();
        let reader_handle = tokio::spawn(Self::reader_loop(stream, pending_clone));

        Ok(Self {
            sink,
            pending,
            next_id: AtomicI64::new(1),
            _reader_handle: reader_handle,
        })
    }

    /// Connect to the first available page on a debug port.
    pub async fn connect_to_first_page(port: u16) -> BitFunResult<Self> {
        let pages = Self::list_pages(port).await?;
        let page = pages
            .iter()
            .find(|p| p.page_type.as_deref() == Some("page") && p.web_socket_debugger_url.is_some())
            .or_else(|| pages.first())
            .ok_or_else(|| BitFunError::tool("No browser pages found via CDP".to_string()))?;

        let ws_url = page
            .web_socket_debugger_url
            .as_ref()
            .ok_or_else(|| BitFunError::tool("Page has no WebSocket debugger URL".to_string()))?;

        Self::connect(ws_url).await
    }

    /// Send a CDP method call and wait for the response.
    pub async fn send(&self, method: &str, params: Option<Value>) -> BitFunResult<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = json!({
            "id": id,
            "method": method,
            "params": params.unwrap_or(json!({})),
        });

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(id, tx);
        }

        debug!("CDP send id={} method={}", id, method);
        {
            let mut sink = self.sink.lock().await;
            sink.send(Message::Text(msg.to_string()))
                .await
                .map_err(|e| BitFunError::tool(format!("CDP send failed: {}", e)))?;
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| BitFunError::tool(format!("CDP timeout for method {}", method)))?
            .map_err(|_| BitFunError::tool("CDP response channel closed".to_string()))?;

        if let Some(error) = result.get("error") {
            return Err(BitFunError::tool(format!("CDP error: {}", error)));
        }

        Ok(result.get("result").cloned().unwrap_or(json!({})))
    }

    async fn reader_loop(
        mut stream: WsStream,
        pending: Arc<RwLock<HashMap<i64, tokio::sync::oneshot::Sender<Value>>>>,
    ) {
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    if let Ok(val) = serde_json::from_str::<Value>(&text) {
                        if let Some(id) = val.get("id").and_then(|v| v.as_i64()) {
                            let sender = {
                                let mut pending = pending.write().await;
                                pending.remove(&id)
                            };
                            if let Some(tx) = sender {
                                let _ = tx.send(val);
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    debug!("CDP WebSocket closed by server");
                    break;
                }
                Err(e) => {
                    warn!("CDP WebSocket read error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    }
}

impl Drop for CdpClient {
    fn drop(&mut self) {
        self._reader_handle.abort();
    }
}
