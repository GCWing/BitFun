//! Concrete HTTP, SQLite, identity verification, and artifact behavior for the appearance market.

mod artifacts;
mod auth;
pub mod config;
mod db;
mod error;
mod package;
mod request_id;
mod retention;
mod routes;

use artifacts::ArtifactStore;
use auth::IdentityVerifier;
use axum::extract::Request;
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get};
use axum::Router;
use config::SkinMarketConfig;
use db::Database;
use routes::{api_router, SkinMarketState};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tower_http::services::{ServeDir, ServeFile};

pub use error::{SkinMarketError, SkinMarketResult};
pub use package::{validate_appearance_package, ValidatedAppearancePackage, ValidatedPreview};

pub async fn build_skin_market_router(config: SkinMarketConfig) -> anyhow::Result<Router> {
    let database = Database::open(&config.database_path).await?;
    let artifacts = ArtifactStore::open(config.artifact_dir.clone()).await?;
    let cleanup = retention::cleanup(&database, &artifacts).await?;
    if cleanup.submissions_scrubbed > 0
        || cleanup.packages_removed > 0
        || cleanup.previews_removed > 0
        || cleanup.temporary_files_removed > 0
        || cleanup.download_rows_compacted > 0
    {
        tracing::info!(
            submissions_scrubbed = cleanup.submissions_scrubbed,
            packages_removed = cleanup.packages_removed,
            previews_removed = cleanup.previews_removed,
            temporary_files_removed = cleanup.temporary_files_removed,
            download_rows_compacted = cleanup.download_rows_compacted,
            "Appearance market startup retention cleanup completed"
        );
    }
    retention::spawn_cleanup_loop(database.clone(), artifacts.clone());
    let identity = IdentityVerifier::new(config.identity_me_url.clone())?;
    let state = Arc::new(SkinMarketState {
        config: config.clone(),
        database,
        artifacts,
        identity,
        upload_permits: Arc::new(Semaphore::new(1)),
    });
    let index_file = config.web_dir.join("index.html");
    let static_files = ServeDir::new(&config.web_dir).append_index_html_on_directories(true);

    Ok(Router::new()
        .route("/", get(|| async { Redirect::permanent("/skin/") }))
        .nest("/skin/api/v1", api_router(state))
        .route("/skin/api", any(unknown_api_version))
        .route("/skin/api/{*rest}", any(unknown_api_version))
        .route_service(
            "/skin/appearances/{slug}",
            ServeFile::new(index_file.clone()),
        )
        .route_service(
            "/skin/appearances/{slug}/",
            ServeFile::new(index_file.clone()),
        )
        .route_service("/skin/submissions", ServeFile::new(index_file.clone()))
        .route_service("/skin/submissions/", ServeFile::new(index_file.clone()))
        .route_service("/skin/admin", ServeFile::new(index_file.clone()))
        .route_service("/skin/admin/", ServeFile::new(index_file))
        .nest_service("/skin", static_files)
        .layer(axum::middleware::from_fn(normalize_payload_too_large))
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn(request_id::middleware)))
}

async fn normalize_payload_too_large(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE
        && !response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json"))
    {
        SkinMarketError::payload_too_large().into_response()
    } else {
        response
    }
}

async fn unknown_api_version() -> SkinMarketError {
    SkinMarketError::not_found("Unknown Skin market API version or route.")
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: https://avatars.githubusercontent.com; \
             connect-src 'self'; object-src 'none'; base-uri 'self'; \
             frame-ancestors 'self'; form-action 'self'",
        ),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, HeaderMap, HeaderName, Request, StatusCode};
    use axum::routing::get;
    use axum::Json;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::io::{Cursor, Write};
    use tower::ServiceExt;
    use zip::write::SimpleFileOptions;

    async fn test_identity(headers: HeaderMap) -> (StatusCode, Json<Value>) {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let (github_id, login, is_admin) = match token {
            "Bearer owner-token" => (41, "owner", false),
            "Bearer admin-token" => (42, "admin", true),
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "unauthorized"})),
                );
            }
        };
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "user": {
                    "githubId": github_id,
                    "login": login,
                    "avatarUrl": "https://example.invalid/avatar"
                },
                "isAdmin": is_admin
            })),
        )
    }

    async fn identity_url() -> url::Url {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route("/me", get(test_identity).post(test_identity));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        url::Url::parse(&format!("http://{address}/me")).unwrap()
    }

    async fn test_router(temporary: &tempfile::TempDir) -> Router {
        let web_dir = temporary.path().join("web");
        tokio::fs::create_dir_all(&web_dir).await.unwrap();
        tokio::fs::write(web_dir.join("index.html"), "<html>skin market</html>")
            .await
            .unwrap();
        build_skin_market_router(SkinMarketConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            public_base_url: "http://localhost/skin".to_string(),
            database_path: temporary.path().join("market.sqlite"),
            artifact_dir: temporary.path().join("artifacts"),
            web_dir,
            identity_me_url: identity_url().await,
            download_hash_secret: "test-download-secret-at-least-24".to_string(),
            public_browse: true,
        })
        .await
        .unwrap()
    }

    fn request(method: &str, uri: &str, token: Option<&str>, body: Body) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(body).unwrap()
    }

    async fn json_body(response: Response) -> Value {
        serde_json::from_slice(
            &to_bytes(response.into_body(), 2 * 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap()
    }

    fn appearance_package() -> Vec<u8> {
        appearance_package_with_id("example.aurora")
    }

    fn appearance_package_with_id(package_id: &str) -> Vec<u8> {
        let mut preview_output = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(8, 6)
            .write_to(&mut preview_output, image::ImageFormat::Png)
            .unwrap();
        let preview = preview_output.into_inner();
        let manifest = serde_json::json!({
            "schema": "openbitfun.appearance",
            "schemaVersion": 1,
            "id": package_id,
            "name": "Aurora",
            "description": "A safe appearance",
            "version": "1.0.0",
            "mode": "dark",
            "preview": {"kind": "asset", "assetId": "preview"},
            "requiredCapabilities": ["assets.v1"],
            "assets": {
                "preview": {
                    "kind": "image",
                    "mimeType": "image/png",
                    "source": {"kind": "package", "path": "assets/preview.png"}
                }
            },
            "integrity": {
                "sha256": {"preview": hex::encode(Sha256::digest(&preview))}
            }
        });
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("appearance.json", SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        writer
            .start_file("assets/preview.png", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&preview).unwrap();
        writer.finish().unwrap().into_inner()
    }

    async fn create_submission(app: &Router, slug: &str) -> String {
        let body = serde_json::json!({
            "slug": slug,
            "releaseNumber": 1,
            "minOpenBitFunVersion": "1.0.0",
            "changelog": "Initial release",
            "license": {"spdxExpression": "MIT"}
        });
        let mut request = request(
            "POST",
            "/skin/api/v1/submissions",
            Some("owner-token"),
            Body::from(body.to_string()),
        );
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        json_body(response).await["submissionId"]
            .as_str()
            .unwrap()
            .to_string()
    }

    async fn publish_submission(app: &Router, slug: &str) -> (String, String, String) {
        let submission_id = create_submission(app, slug).await;
        let mut upload = request(
            "PUT",
            &format!("/skin/api/v1/submissions/{submission_id}/package"),
            Some("owner-token"),
            Body::from(appearance_package_with_id(&format!(
                "example.{}",
                slug.replace('-', ".")
            ))),
        );
        upload.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(
                openbitfun_product_domains::appearance_market::APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE,
            ),
        );
        assert_eq!(
            app.clone().oneshot(upload).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    &format!("/skin/api/v1/submissions/{submission_id}/submit"),
                    Some("owner-token"),
                    Body::empty(),
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        let mut approve = request(
            "POST",
            &format!("/skin/api/v1/admin/submissions/{submission_id}/decision"),
            Some("admin-token"),
            Body::from(r#"{"decision":"approve","reason":""}"#),
        );
        approve.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        let response = app.clone().oneshot(approve).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let approved = json_body(response).await;
        assert_eq!(approved["submission"]["publicationStatus"], "published");
        let listing_id = approved["submission"]["listingId"]
            .as_str()
            .unwrap()
            .to_string();
        let detail = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("/skin/api/v1/listings/{slug}"),
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        let release_id = json_body(detail).await["releases"][0]["releaseId"]
            .as_str()
            .unwrap()
            .to_string();
        (submission_id, listing_id, release_id)
    }

    #[tokio::test]
    async fn bearer_submission_review_and_public_download_flow() {
        let temporary = tempfile::tempdir().unwrap();
        let app = test_router(&temporary).await;
        let submission_id = create_submission(&app, "aurora-market").await;
        let package = appearance_package();
        let mut upload = request(
            "PUT",
            &format!("/skin/api/v1/submissions/{submission_id}/package"),
            Some("owner-token"),
            Body::from(package.clone()),
        );
        upload.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(
                openbitfun_product_domains::appearance_market::APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE,
            ),
        );
        let response = app.clone().oneshot(upload).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let uploaded = json_body(response).await;
        assert_eq!(uploaded["packageId"], "example.aurora");
        let private_preview_path = uploaded["previewUrl"]
            .as_str()
            .unwrap()
            .strip_prefix("http://localhost")
            .unwrap()
            .to_string();
        assert!(private_preview_path.starts_with("/skin/api/v1/artifacts/previews/"));

        let private_preview = app
            .clone()
            .oneshot(request(
                "GET",
                &private_preview_path,
                Some("owner-token"),
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(private_preview.status(), StatusCode::OK);
        assert_eq!(
            private_preview.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
        let anonymous_preview = app
            .clone()
            .oneshot(request("GET", &private_preview_path, None, Body::empty()))
            .await
            .unwrap();
        assert_eq!(anonymous_preview.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/skin/api/v1/submissions/{submission_id}/submit"),
                Some("owner-token"),
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["status"], "submitted");

        let mut approve = request(
            "POST",
            &format!("/skin/api/v1/admin/submissions/{submission_id}/decision"),
            Some("admin-token"),
            Body::from(r#"{"decision":"approve","reason":""}"#),
        );
        approve.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        let response = app.clone().oneshot(approve).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let approved = json_body(response).await;
        assert_eq!(approved["submission"]["status"], "approved");
        assert_eq!(approved["submitter"]["login"], "owner");
        assert_eq!(approved["submitter"]["githubId"], 41);
        assert!(approved["reviewBundleHash"].as_str().is_some());

        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/skin/api/v1/listings?mode=dark",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let listing = json_body(response).await;
        assert_eq!(listing["items"][0]["slug"], "aurora-market");
        assert_eq!(listing["items"][0]["packageId"], "example.aurora");
        assert!(listing["items"][0]["previewUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://localhost/skin/api/v1/artifacts/previews/"));

        let public_preview = app
            .clone()
            .oneshot(request("GET", &private_preview_path, None, Body::empty()))
            .await
            .unwrap();
        assert_eq!(public_preview.status(), StatusCode::OK);
        assert_eq!(
            public_preview.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );

        let compact_preview = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("{private_preview_path}?variant=compact-v1"),
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(compact_preview.status(), StatusCode::OK);
        assert!(compact_preview.headers()[header::ETAG]
            .to_str()
            .unwrap()
            .ends_with("-compact-v1\""));

        let invalid_preview_variant = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("{private_preview_path}?variant=unbounded"),
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(invalid_preview_variant.status(), StatusCode::BAD_REQUEST);

        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/skin/api/v1/listings/aurora-market",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        let detail = json_body(response).await;
        assert_eq!(detail["releases"][0]["packageVersion"], "1.0.0");
        assert_eq!(detail["releases"][0]["yanked"], false);

        let head = app
            .clone()
            .oneshot(request(
                "HEAD",
                "/skin/api/v1/listings/aurora-market/releases/1/download",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(
            head.headers()[header::CONTENT_LENGTH].to_str().unwrap(),
            package.len().to_string()
        );

        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/skin/api/v1/listings/aurora-market/releases/1/download",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH].to_str().unwrap(),
            package.len().to_string()
        );
        assert_eq!(
            to_bytes(response.into_body(), package.len() + 1)
                .await
                .unwrap()
                .as_ref(),
            package.as_slice()
        );
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/skin/api/v1/listings/aurora-market",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(json_body(response).await["downloadCount"], 1);
    }

    #[tokio::test]
    async fn api_errors_are_bounded_and_use_the_versioned_envelope() {
        let temporary = tempfile::tempdir().unwrap();
        let app = test_router(&temporary).await;
        let health = app
            .clone()
            .oneshot(request("GET", "/skin/api/v1/health", None, Body::empty()))
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let unsafe_license = serde_json::json!({
            "slug": "unsafe-license",
            "releaseNumber": 1,
            "minOpenBitFunVersion": "1.0.0",
            "changelog": "Initial release",
            "license": {"customUrl": "javascript:alert(1)"}
        });
        let mut unsafe_license_request = request(
            "POST",
            "/skin/api/v1/submissions",
            Some("owner-token"),
            Body::from(unsafe_license.to_string()),
        );
        unsafe_license_request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        let unsafe_license_response = app.clone().oneshot(unsafe_license_request).await.unwrap();
        assert_eq!(unsafe_license_response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(unsafe_license_response).await["error"]["code"],
            "invalid_license"
        );

        let mut invalid_mode_request = request(
            "GET",
            "/skin/api/v1/listings?mode=sepia",
            None,
            Body::empty(),
        );
        invalid_mode_request.headers_mut().insert(
            HeaderName::from_static("x-request-id"),
            HeaderValue::from_static("edge-request-123"),
        );
        let invalid_mode = app.clone().oneshot(invalid_mode_request).await.unwrap();
        assert_eq!(invalid_mode.status(), StatusCode::BAD_REQUEST);
        assert_eq!(invalid_mode.headers()["x-request-id"], "edge-request-123");
        let invalid_mode_body = json_body(invalid_mode).await;
        assert_eq!(invalid_mode_body["error"]["code"], "invalid_mode");
        assert_eq!(invalid_mode_body["error"]["requestId"], "edge-request-123");

        let mut malformed_json = request(
            "POST",
            "/skin/api/v1/submissions",
            Some("owner-token"),
            Body::from("{"),
        );
        malformed_json.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        let malformed_json = app.clone().oneshot(malformed_json).await.unwrap();
        assert_eq!(malformed_json.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(malformed_json).await["error"]["code"],
            "invalid_request"
        );

        let method_not_allowed = app
            .clone()
            .oneshot(request(
                "PATCH",
                "/skin/api/v1/listings",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(method_not_allowed.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            json_body(method_not_allowed).await["error"]["code"],
            "method_not_allowed"
        );

        let unknown_version = app
            .clone()
            .oneshot(request("GET", "/skin/api/v2/listings", None, Body::empty()))
            .await
            .unwrap();
        assert_eq!(unknown_version.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            json_body(unknown_version).await["error"]["code"],
            "not_found"
        );

        let deep_link = app
            .clone()
            .oneshot(request(
                "GET",
                "/skin/appearances/aurora-market",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(deep_link.status(), StatusCode::OK);
        for workflow_path in [
            "/skin/appearances/aurora-market/",
            "/skin/submissions",
            "/skin/submissions/",
            "/skin/admin",
            "/skin/admin/",
        ] {
            let response = app
                .clone()
                .oneshot(request("GET", workflow_path, None, Body::empty()))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{workflow_path}");
        }
        let missing_asset = app
            .clone()
            .oneshot(request("GET", "/skin/missing.js", None, Body::empty()))
            .await
            .unwrap();
        assert_eq!(missing_asset.status(), StatusCode::NOT_FOUND);

        let submission_id = create_submission(&app, "large-package").await;
        let mut oversized = request(
            "PUT",
            &format!("/skin/api/v1/submissions/{submission_id}/package"),
            Some("owner-token"),
            Body::empty(),
        );
        oversized.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(
                openbitfun_product_domains::appearance_market::APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE,
            ),
        );
        oversized.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(
                &(openbitfun_product_domains::appearance_market::APPEARANCE_MARKET_MAX_PACKAGE_BYTES
                    + 1)
                .to_string(),
            )
            .unwrap(),
        );
        let response = app.clone().oneshot(oversized).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], "payload_too_large");
        assert!(body["error"]["requestId"].as_str().is_some());

        let missing = app
            .oneshot(request(
                "GET",
                "/skin/api/v1/not-a-route",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(json_body(missing).await["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn submission_history_uses_stable_cursor_pages() {
        let temporary = tempfile::tempdir().unwrap();
        let app = test_router(&temporary).await;
        for index in 0..7 {
            create_submission(&app, &format!("paged-skin-{index}")).await;
        }

        let mut cursor: Option<String> = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let uri = cursor.as_ref().map_or_else(
                || "/skin/api/v1/submissions?limit=3".to_string(),
                |cursor| format!("/skin/api/v1/submissions?limit=3&cursor={cursor}"),
            );
            let response = app
                .clone()
                .oneshot(request("GET", &uri, Some("owner-token"), Body::empty()))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let page = json_body(response).await;
            for item in page["items"].as_array().unwrap() {
                assert!(seen.insert(item["submissionId"].as_str().unwrap().to_string()));
            }
            cursor = page["nextCursor"].as_str().map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 7);
    }

    #[tokio::test]
    async fn submission_history_projects_release_and_listing_moderation() {
        let temporary = tempfile::tempdir().unwrap();
        let app = test_router(&temporary).await;
        let (yanked_submission_id, _, release_id) = publish_submission(&app, "yanked-skin").await;
        let (unpublished_submission_id, listing_id, _) =
            publish_submission(&app, "unpublished-skin").await;

        let mut yank = request(
            "POST",
            &format!("/skin/api/v1/admin/releases/{release_id}/yank"),
            Some("admin-token"),
            Body::from(r#"{"reason":"Unsafe package content"}"#),
        );
        yank.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        assert_eq!(
            app.clone().oneshot(yank).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let mut unpublish = request(
            "POST",
            &format!("/skin/api/v1/admin/listings/{listing_id}/unpublish"),
            Some("admin-token"),
            Body::from(r#"{"reason":"Policy violation"}"#),
        );
        unpublish.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        assert_eq!(
            app.clone().oneshot(unpublish).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/skin/api/v1/submissions",
                Some("owner-token"),
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let history = json_body(response).await;
        let items = history["items"].as_array().unwrap();
        let yanked = items
            .iter()
            .find(|item| item["submissionId"] == yanked_submission_id)
            .unwrap();
        assert_eq!(yanked["status"], "approved");
        assert_eq!(yanked["publicationStatus"], "yanked");
        let unpublished = items
            .iter()
            .find(|item| item["submissionId"] == unpublished_submission_id)
            .unwrap();
        assert_eq!(unpublished["status"], "approved");
        assert_eq!(unpublished["publicationStatus"], "unpublished");
    }
}
