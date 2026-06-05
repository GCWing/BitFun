use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

#[derive(Debug, Clone, Copy)]
pub struct FixtureSseServerOptions {
    pub chunk_size: usize,
    pub chunk_delay: Duration,
}

impl Default for FixtureSseServerOptions {
    fn default() -> Self {
        Self {
            chunk_size: 23,
            chunk_delay: Duration::from_millis(1),
        }
    }
}

#[derive(Clone)]
struct FixtureSseState {
    payload: Arc<Vec<u8>>,
    options: FixtureSseServerOptions,
}

pub struct FixtureSseServer {
    url: String,
    server_task: JoinHandle<()>,
}

impl FixtureSseServer {
    pub async fn spawn(payload: Vec<u8>, options: FixtureSseServerOptions) -> Self {
        let state = FixtureSseState {
            payload: Arc::new(payload),
            options,
        };
        let app = Router::new()
            .route("/stream", get(stream_fixture_handler))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture SSE server");
        let addr = listener.local_addr().expect("fixture SSE server addr");
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("fixture SSE server should run");
        });

        Self {
            url: format!("http://{addr}/stream"),
            server_task,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for FixtureSseServer {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

async fn stream_fixture_handler(State(state): State<FixtureSseState>) -> impl IntoResponse {
    let (tx, rx) = mpsc::channel::<Bytes>(8);

    tokio::spawn(async move {
        let chunk_size = state.options.chunk_size.max(1);
        for chunk in state.payload.chunks(chunk_size) {
            if tx.send(Bytes::copy_from_slice(chunk)).await.is_err() {
                break;
            }
            if !state.options.chunk_delay.is_zero() {
                tokio::time::sleep(state.options.chunk_delay).await;
            }
        }
    });

    let mut response = Response::new(Body::from_stream(
        ReceiverStream::new(rx).map(Ok::<Bytes, Infallible>),
    ));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    response
}
