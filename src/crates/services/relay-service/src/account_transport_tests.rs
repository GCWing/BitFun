//! End-to-end shared-router contract at the local and public path layouts.
use crate::{build_relay_router, db, identity::IdentityVerifier, MemoryAssetStore};
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Request, StatusCode},
    Extension, Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap()
}

#[tokio::test]
async fn official_and_local_layouts_share_authenticated_directory_and_rpc() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let authority_url = format!("http://{}/me", listener.local_addr().unwrap());
    let authority = Router::new().route(
        "/me",
        axum::routing::get(|headers: HeaderMap| async move {
            let id = match headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
            {
                Some("Bearer account-a") => 123,
                Some("Bearer account-b") => 456,
                _ => return (StatusCode::UNAUTHORIZED, Json(json!({}))),
            };
            (
                StatusCode::OK,
                Json(json!({"user":{"githubId":id,"login":format!("user-{id}")}})),
            )
        }),
    );
    let authority_task = tokio::spawn(async move {
        axum::serve(listener, authority).await.unwrap();
    });
    for prefix in ["", "/v/1.0.0"] {
        let db = Arc::new(db::connect(":memory:").await.unwrap());
        let shared = build_relay_router(
            Arc::new(MemoryAssetStore::new()),
            Instant::now(),
            db,
            "test",
        )
        .layer(Extension(
            IdentityVerifier::with_url(&authority_url).unwrap(),
        ));
        let app = if prefix.is_empty() {
            shared
        } else {
            Router::new().nest(prefix, shared)
        };
        let route = |path: &str| format!("{prefix}{path}");
        assert_eq!(
            request(&app, "GET", &route("/api/devices"), "", json!(null))
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        for retired in ["/api/rooms", "/api/pair", "/api/auth/login/challenge"] {
            assert_eq!(
                request(&app, "POST", &route(retired), "", json!({}))
                    .await
                    .status(),
                StatusCode::NOT_FOUND
            );
        }
        let mut tokens = Vec::new();
        for (device, account) in [
            ("desktop", "account-a"),
            ("mobile", "account-a"),
            ("outsider", "account-b"),
        ] {
            let response = request(&app, "POST", &route("/api/auth/login"), "", json!({
                "access_token":account, "user_id":"untrusted", "device_id":device,
                "device_name":device, "device_kind":"desktop", "public_key":BASE64.encode([9u8;32]),
                "request_id":uuid::Uuid::new_v4().to_string()
            })).await;
            assert_eq!(response.status(), StatusCode::OK);
            let response = json_body(response).await;
            assert_eq!(
                response["user_id"],
                if account == "account-a" { "123" } else { "456" }
            );
            tokens.push(response["token"].as_str().unwrap().to_owned());
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_app = app.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, server_app).await.unwrap();
        });
        let (mut socket, _) =
            tokio_tungstenite::connect_async(format!("ws://{address}{prefix}/ws"))
                .await
                .unwrap();
        socket.send(Message::Text(json!({"type":"auth_connect","token":tokens[0],"device_name":"desktop","device_kind":"desktop"}).to_string().into())).await.unwrap();
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(3), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let value: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
            if value["type"] == "auth_ok" {
                assert_eq!(value["user_id"], "123");
                break;
            }
        }
        let devices =
            json_body(request(&app, "GET", &route("/api/devices"), &tokens[1], json!(null)).await)
                .await;
        assert!(devices
            .as_array()
            .unwrap()
            .iter()
            .any(|device| device["device_id"] == "desktop" && device["online"] == true));
        assert!(!devices
            .as_array()
            .unwrap()
            .iter()
            .any(|device| device["device_id"] == "outsider"));
        assert_eq!(
            request(
                &app,
                "GET",
                &route("/api/devices/desktop/key"),
                &tokens[1],
                json!(null)
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            request(
                &app,
                "GET",
                &route("/api/devices/desktop/key"),
                &tokens[2],
                json!(null)
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &app,
                "POST",
                &route("/api/devices/desktop/rpc"),
                &tokens[2],
                json!({"encrypted_data":"YQ==","nonce":"bg=="})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let rpc_app = app.clone();
        let rpc_token = tokens[1].clone();
        let rpc_path = route("/api/devices/desktop/rpc");
        let rpc = tokio::spawn(async move {
            request(
                &rpc_app,
                "POST",
                &rpc_path,
                &rpc_token,
                json!({"encrypted_data":"YQ==","nonce":"bg=="}),
            )
            .await
        });
        let incoming = loop {
            let msg = tokio::time::timeout(Duration::from_secs(3), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let value: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
            if value["type"] == "incoming_device_message" {
                break value;
            }
        };
        assert_eq!(incoming["source_device_id"], "mobile");
        let payload = BASE64.encode(vec![7u8; 32 * 1024]);
        let response = request(&app,"POST",&route("/api/devices/mobile/messages"),&tokens[0],json!({
            "correlation_id":incoming["correlation_id"], "encrypted_data":payload, "nonce":"bg=="
        })).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = tokio::time::timeout(Duration::from_secs(3), rpc)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["encrypted_data"], payload);
        assert_eq!(
            request(
                &app,
                "POST",
                &route("/api/auth/logout"),
                &tokens[1],
                json!({})
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            request(&app, "GET", &route("/api/devices"), &tokens[1], json!(null))
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        socket.close(None).await.unwrap();
        server.abort();
    }
    authority_task.abort();
}
