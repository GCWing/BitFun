//! BitFun Page: versioned publish + Page Functions serve.
//!
//! Flow: upload draft → freeze version → deploy production pointer.
//! Preview: `/p/{user}/{slug}/@v/{version}/...`
//! Production: `/p/{user}/{slug}/...` (deployed version only).

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bitfun_page_function_runtime::{
    run_fetch, FetchRequest, PageMeta, DEFAULT_TIMEOUT, WORKER_ENTRY_PATH,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::{
    new_page_version_id, page_draft_asset_key, page_legacy_asset_key, page_version_asset_key,
    PageRow, PageVersionRow, PageVisibility, PageWithUsername, UserRow,
};
use crate::page_data::RelayPageHost;
use crate::routes::api::AppState;
use crate::routes::sync::{extract_bearer_token, validate_auth, validate_token, AuthUser};
use crate::WebAssetStore;

pub const MAX_PAGES_PER_USER: i64 = 50;
pub const MAX_VERSIONS_PER_PAGE: i64 = 30;
pub const MAX_PAGE_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const PAGE_UPLOAD_BODY_LIMIT: usize = 12 * 1024 * 1024;

fn is_valid_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    let Some(first) = bytes.first() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn require_db(state: &AppState) -> Result<&crate::db::DbPool, StatusCode> {
    state
        .db
        .as_ref()
        .map(|db| db.as_ref())
        .ok_or(StatusCode::NOT_IMPLEMENTED)
}

pub fn pages_router() -> Router<AppState> {
    Router::new()
        .route("/api/pages", get(list_pages))
        .route(
            "/api/pages/check-files",
            post(check_page_files).layer(DefaultBodyLimit::max(PAGE_UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/pages/upload-files",
            post(upload_page_files).layer(DefaultBodyLimit::max(PAGE_UPLOAD_BODY_LIMIT)),
        )
        .route(
            "/api/pages/{slug}/versions",
            get(list_versions).post(freeze_version),
        )
        .route("/api/pages/{slug}/deploy", post(deploy_version))
        .route(
            "/api/pages/{slug}/versions/{version_id}",
            axum::routing::delete(delete_version),
        )
        .route(
            "/api/pages/{slug}",
            axum::routing::patch(update_page).delete(delete_page),
        )
        // Preview routes (more specific first).
        .route(
            "/p/{username}/{slug}/@v/{version_id}",
            get(serve_preview_root).post(serve_preview_root),
        )
        .route(
            "/p/{username}/{slug}/@v/{version_id}/{*path}",
            get(serve_preview_path)
                .post(serve_preview_path)
                .put(serve_preview_path)
                .delete(serve_preview_path)
                .patch(serve_preview_path),
        )
        .route(
            "/p/{username}/{slug}",
            get(serve_prod_root).post(serve_prod_root),
        )
        .route(
            "/p/{username}/{slug}/{*path}",
            get(serve_prod_path)
                .post(serve_prod_path)
                .put(serve_prod_path)
                .delete(serve_prod_path)
                .patch(serve_prod_path),
        )
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct FileManifestEntry {
    pub path: String,
    pub hash: String,
    pub size: u64,
}

#[derive(Deserialize)]
pub struct CheckPageFilesRequest {
    pub slug: String,
    pub files: Vec<FileManifestEntry>,
}

#[derive(Serialize)]
pub struct CheckPageFilesResponse {
    pub needed: Vec<String>,
    pub existing_count: usize,
    pub total_count: usize,
}

#[derive(Deserialize)]
pub struct UploadFileEntry {
    pub content: String,
    pub hash: String,
}

#[derive(Deserialize)]
pub struct UploadPageFilesRequest {
    pub slug: String,
    #[serde(default)]
    pub title: String,
    pub visibility: String,
    pub files: HashMap<String, UploadFileEntry>,
    #[serde(default)]
    pub finalize: bool,
}

#[derive(Deserialize)]
pub struct FreezeVersionRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize)]
pub struct DeployRequest {
    pub version_id: String,
}

#[derive(Serialize)]
pub struct PageInfo {
    pub slug: String,
    pub visibility: String,
    pub title: String,
    pub file_count: i64,
    pub total_bytes: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub url_path: String,
    pub preview_url_path: Option<String>,
    pub deployed_version_id: Option<String>,
}

#[derive(Serialize)]
pub struct VersionInfo {
    pub version_id: String,
    pub title: String,
    pub file_count: i64,
    pub total_bytes: i64,
    pub has_worker: bool,
    pub note: String,
    pub created_at: i64,
    pub deployed: bool,
    pub preview_url_path: String,
}

#[derive(Deserialize)]
pub struct UpdatePageRequest {
    pub visibility: Option<String>,
    pub title: Option<String>,
}

// ── Management ──────────────────────────────────────────────────────────

async fn list_pages(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PageInfo>>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    let pages = PageRow::list_for_user(db, &auth.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let username = UserRow::find_by_username_for_user_id(db, &auth.user_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    let mut infos = Vec::new();
    for p in pages {
        // One-time legacy migration: old room without versions.
        maybe_migrate_legacy_page(&state, &p).await;
        let page = PageRow::get(db, &p.user_id, &p.slug)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .unwrap_or(p);
        infos.push(page_to_info(&page, &username));
    }
    Ok(Json(infos))
}

async fn check_page_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CheckPageFilesRequest>,
) -> Result<Json<CheckPageFilesResponse>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&body.slug) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut total_bytes: u64 = 0;
    for entry in &body.files {
        if crate::validated_asset_relative_path(&entry.path).is_err()
            || !crate::is_valid_content_hash(&entry.hash)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        if entry.size > MAX_FILE_BYTES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        total_bytes = total_bytes.saturating_add(entry.size);
    }
    if total_bytes > MAX_PAGE_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let existing = PageRow::get(db, &auth.user_id, &body.slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if existing.is_none() {
        let count = PageRow::count_for_user(db, &auth.user_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if count >= MAX_PAGES_PER_USER {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    let draft_key = page_draft_asset_key(&auth.user_id, &body.slug);
    let asset_store = Arc::clone(&state.asset_store);
    let response = tokio::task::spawn_blocking(move || {
        process_check_page_files(asset_store, &draft_key, body.files)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn upload_page_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UploadPageFilesRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&body.slug) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let visibility = PageVisibility::parse(&body.visibility).ok_or(StatusCode::BAD_REQUEST)?;

    for (rel_path, entry) in &body.files {
        if crate::validated_asset_relative_path(rel_path).is_err()
            || !crate::is_valid_content_hash(&entry.hash)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        let approx = (entry.content.len() as u64).saturating_mul(3) / 4;
        if approx > MAX_FILE_BYTES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
    }

    let existing = PageRow::get(db, &auth.user_id, &body.slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if existing.is_none() {
        let count = PageRow::count_for_user(db, &auth.user_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if count >= MAX_PAGES_PER_USER {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    let title = if body.title.is_empty() {
        existing
            .as_ref()
            .map(|p| p.title.clone())
            .unwrap_or_else(|| body.slug.clone())
    } else {
        body.title.clone()
    };
    PageRow::ensure(db, &auth.user_id, &body.slug, visibility, &title)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let draft_key = page_draft_asset_key(&auth.user_id, &body.slug);
    let asset_store = Arc::clone(&state.asset_store);
    let files = body.files;
    let stored = tokio::task::spawn_blocking(move || {
        process_upload_page_files(asset_store, &draft_key, files)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "files_stored": stored,
        "slug": body.slug,
        "draft": true,
        "finalize": body.finalize,
    })))
}

async fn freeze_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(body): Json<FreezeVersionRequest>,
) -> Result<Json<VersionInfo>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&slug) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let page = PageRow::get(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let count = PageVersionRow::count_for_page(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if count >= MAX_VERSIONS_PER_PAGE {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let draft_key = page_draft_asset_key(&auth.user_id, &slug);
    if !state.asset_store.has_room_files(&draft_key) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let version_id = new_page_version_id();
    let version_key = page_version_asset_key(&auth.user_id, &slug, &version_id);
    let asset_store = Arc::clone(&state.asset_store);
    let draft_key_c = draft_key.clone();
    let version_key_c = version_key.clone();
    tokio::task::spawn_blocking(move || asset_store.copy_room(&draft_key_c, &version_key_c))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let entries = state.asset_store.list_room_entries(&version_key);
    let has_worker = entries.iter().any(|(p, _)| p == WORKER_ENTRY_PATH);
    let has_index = entries.iter().any(|(p, _)| p == "index.html");
    if !has_worker && !has_index {
        state.asset_store.cleanup_room(&version_key);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Derive version stats from the actual stored files; never trust
    // client-reported file_count/total_bytes for quota accounting.
    let file_count = entries.len() as i64;
    let total_bytes = state.asset_store.room_total_bytes(&version_key);
    if total_bytes > MAX_PAGE_BYTES {
        state.asset_store.cleanup_room(&version_key);
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let total_bytes = total_bytes as i64;
    let title = if body.title.is_empty() {
        page.title.clone()
    } else {
        body.title.clone()
    };

    PageVersionRow::insert(
        db,
        &auth.user_id,
        &slug,
        &version_id,
        &title,
        file_count,
        total_bytes,
        has_worker,
        &body.note,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Clear draft after successful freeze.
    state.asset_store.cleanup_room(&draft_key);

    let username = UserRow::find_by_username_for_user_id(db, &auth.user_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    Ok(Json(VersionInfo {
        version_id: version_id.clone(),
        title,
        file_count,
        total_bytes,
        has_worker,
        note: body.note,
        created_at: chrono::Utc::now().timestamp(),
        deployed: false,
        preview_url_path: format!("/p/{username}/{slug}/@v/{version_id}"),
    }))
}

async fn list_versions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Json<Vec<VersionInfo>>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&slug) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let page = PageRow::get(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    maybe_migrate_legacy_page(&state, &page).await;

    let versions = PageVersionRow::list_for_page(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let username = UserRow::find_by_username_for_user_id(db, &auth.user_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let deployed = page.deployed_version_id.clone();

    Ok(Json(
        versions
            .into_iter()
            .map(|v| VersionInfo {
                preview_url_path: format!("/p/{username}/{slug}/@v/{}", v.version_id),
                deployed: deployed.as_deref() == Some(v.version_id.as_str()),
                version_id: v.version_id,
                title: v.title,
                file_count: v.file_count,
                total_bytes: v.total_bytes,
                has_worker: v.has_worker != 0,
                note: v.note,
                created_at: v.created_at,
            })
            .collect(),
    ))
}

async fn deploy_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(body): Json<DeployRequest>,
) -> Result<Json<PageInfo>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&slug) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let version = PageVersionRow::get(db, &auth.user_id, &slug, &body.version_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    PageRow::set_deployed_version(
        db,
        &auth.user_id,
        &slug,
        &version.version_id,
        version.file_count,
        version.total_bytes,
        &version.title,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let page = PageRow::get(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let username = UserRow::find_by_username_for_user_id(db, &auth.user_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    Ok(Json(page_to_info(&page, &username)))
}

async fn delete_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slug, version_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&slug) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let page = PageRow::get(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if page.deployed_version_id.as_deref() == Some(version_id.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let deleted = PageVersionRow::delete(db, &auth.user_id, &slug, &version_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !deleted {
        return Err(StatusCode::NOT_FOUND);
    }
    let key = page_version_asset_key(&auth.user_id, &slug, &version_id);
    state.asset_store.cleanup_room(&key);
    Ok(Json(
        serde_json::json!({ "status": "ok", "version_id": version_id }),
    ))
}

async fn update_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(body): Json<UpdatePageRequest>,
) -> Result<Json<PageInfo>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&slug) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let visibility = match body.visibility.as_deref() {
        Some(v) => Some(PageVisibility::parse(v).ok_or(StatusCode::BAD_REQUEST)?),
        None => None,
    };
    let updated = PageRow::update_meta(db, &auth.user_id, &slug, visibility, body.title.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }
    let page = PageRow::get(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let username = UserRow::find_by_username_for_user_id(db, &auth.user_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    Ok(Json(page_to_info(&page, &username)))
}

async fn delete_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = require_db(&state)?;
    if !is_valid_slug(&slug) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let versions = PageVersionRow::list_for_page(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    for v in &versions {
        state.asset_store.cleanup_room(&page_version_asset_key(
            &auth.user_id,
            &slug,
            &v.version_id,
        ));
    }
    state
        .asset_store
        .cleanup_room(&page_draft_asset_key(&auth.user_id, &slug));
    state
        .asset_store
        .cleanup_room(&page_legacy_asset_key(&auth.user_id, &slug));
    if let Some(store) = &state.page_data {
        store.cleanup_page(&auth.user_id, &slug);
    }
    let deleted = PageRow::delete(db, &auth.user_id, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !deleted {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(serde_json::json!({ "status": "ok", "slug": slug })))
}

// ── Serve ───────────────────────────────────────────────────────────────

async fn serve_prod_root(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((username, slug)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, StatusCode> {
    serve_page(state, method, headers, &username, &slug, None, "", body).await
}

async fn serve_prod_path(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((username, slug, path)): Path<(String, String, String)>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, StatusCode> {
    // Guard against treating `@v/...` as production path when catch-all matches wrongly.
    if path.starts_with("@v/") || path == "@v" {
        return Err(StatusCode::NOT_FOUND);
    }
    serve_page(state, method, headers, &username, &slug, None, &path, body).await
}

async fn serve_preview_root(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((username, slug, version_id)): Path<(String, String, String)>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, StatusCode> {
    serve_page(
        state,
        method,
        headers,
        &username,
        &slug,
        Some(version_id.as_str()),
        "",
        body,
    )
    .await
}

async fn serve_preview_path(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((username, slug, version_id, path)): Path<(String, String, String, String)>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, StatusCode> {
    serve_page(
        state,
        method,
        headers,
        &username,
        &slug,
        Some(version_id.as_str()),
        &path,
        body,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn serve_page(
    state: AppState,
    method: Method,
    headers: HeaderMap,
    username: &str,
    slug: &str,
    version_override: Option<&str>,
    path: &str,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, StatusCode> {
    let db = require_db(&state)?;
    if !is_valid_slug(slug) || username.is_empty() || username.contains("..") {
        return Err(StatusCode::NOT_FOUND);
    }
    if path.contains("..") {
        return Err(StatusCode::NOT_FOUND);
    }

    let page = PageRow::get_by_username(db, username, slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    enforce_visibility(&state, &headers, &page).await?;

    // Resolve version.
    let version_id = if let Some(v) = version_override {
        v.to_string()
    } else {
        maybe_migrate_legacy_page_by_ids(&state, &page.user_id, slug).await;
        let refreshed = PageRow::get(db, &page.user_id, slug)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        refreshed.deployed_version_id.ok_or(StatusCode::NOT_FOUND)?
    };

    let version = PageVersionRow::get(db, &page.user_id, slug, &version_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let asset_key = page_version_asset_key(&page.user_id, slug, &version_id);
    let lookup_path = if path.is_empty() || path.ends_with('/') {
        if path.is_empty() {
            "index.html".to_string()
        } else {
            format!("{path}index.html")
        }
    } else {
        path.to_string()
    };

    if version.has_worker != 0 {
        return serve_with_worker(
            state,
            method,
            headers,
            &page,
            &version,
            &asset_key,
            &lookup_path,
            path,
            body,
        )
        .await;
    }

    // Static-only version.
    if method != Method::GET && method != Method::HEAD {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }
    let asset_store = Arc::clone(&state.asset_store);
    let asset_key_io = asset_key.clone();
    let lookup_io = lookup_path.clone();
    let content =
        tokio::task::spawn_blocking(move || asset_store.get_file(&asset_key_io, &lookup_io))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    let mime = mime_from_path(&lookup_path);
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (
                header::HeaderName::from_static("x-content-type-options"),
                "nosniff",
            ),
        ],
        axum::body::Body::from(content),
    )
        .into_response())
}

#[allow(clippy::too_many_arguments)]
async fn serve_with_worker(
    state: AppState,
    method: Method,
    headers: HeaderMap,
    page: &PageWithUsername,
    version: &PageVersionRow,
    asset_key: &str,
    lookup_path: &str,
    raw_path: &str,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, StatusCode> {
    let page_data = state.page_data.clone().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let db = state.db.clone().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let asset_store = Arc::clone(&state.asset_store);
    let asset_store_fallback = Arc::clone(&state.asset_store);
    let asset_key = asset_key.to_string();
    let lookup_path_owned = lookup_path.to_string();
    let worker_source = {
        let store = Arc::clone(&asset_store);
        let key = asset_key.clone();
        tokio::task::spawn_blocking(move || store.get_file_exact(&key, WORKER_ENTRY_PATH))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
    };
    let worker_source = String::from_utf8(worker_source).map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Never forward credential headers into the page worker: a malicious page
    // owner could persist viewer tokens via env.KV/BLOBS and harvest them.
    let mut req_headers = HashMap::new();
    for (k, v) in headers.iter() {
        if matches!(
            k.as_str(),
            "authorization" | "cookie" | "proxy-authorization"
        ) {
            continue;
        }
        if let Ok(val) = v.to_str() {
            req_headers.insert(k.as_str().to_string(), val.to_string());
        }
    }
    let path_for_req = if raw_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{raw_path}")
    };
    let url = format!(
        "/p/{}/{}/@v/{}{}",
        page.username,
        page.slug,
        version.version_id,
        if raw_path.is_empty() {
            String::new()
        } else {
            format!("/{raw_path}")
        }
    );

    let host = Arc::new(RelayPageHost {
        db,
        page_data,
        user_id: page.user_id.clone(),
        slug: page.slug.clone(),
        meta: PageMeta {
            username: page.username.clone(),
            slug: page.slug.clone(),
            version_id: version.version_id.clone(),
            visibility: page.visibility.clone(),
        },
        asset_store,
        asset_key: asset_key.clone(),
    });

    let fetch_req = FetchRequest {
        method: method.as_str().to_string(),
        url,
        path: path_for_req,
        headers: req_headers,
        body: if body.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&body).into_owned())
        },
    };

    let response = tokio::task::spawn_blocking(move || {
        run_fetch(&worker_source, &fetch_req, host, DEFAULT_TIMEOUT)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|e| {
        tracing::warn!("Page function error: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    // If worker returns 404 for document GET, fall back to static assets.
    if response.status == 404 && (method == Method::GET || method == Method::HEAD) {
        let key = asset_key.clone();
        let lookup = lookup_path_owned.clone();
        if let Ok(Some(bytes)) =
            tokio::task::spawn_blocking(move || asset_store_fallback.get_file(&key, &lookup)).await
        {
            let mime = mime_from_path(&lookup_path_owned);
            return Ok((
                [
                    (header::CONTENT_TYPE, mime),
                    (
                        header::HeaderName::from_static("x-content-type-options"),
                        "nosniff",
                    ),
                ],
                axum::body::Body::from(bytes),
            )
                .into_response());
        }
    }

    let mut builder = axum::http::Response::builder()
        .status(response.status)
        .header("x-content-type-options", "nosniff");
    for (k, v) in response.headers {
        if let (Ok(name), Ok(val)) = (
            axum::http::HeaderName::try_from(k),
            axum::http::HeaderValue::try_from(v),
        ) {
            builder = builder.header(name, val);
        }
    }
    builder
        .body(axum::body::Body::from(response.body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn page_to_info(page: &PageRow, username: &str) -> PageInfo {
    let url_path = if username.is_empty() || page.deployed_version_id.is_none() {
        String::new()
    } else {
        format!("/p/{username}/{}", page.slug)
    };
    let preview_url_path = page.deployed_version_id.as_ref().map(|v| {
        if username.is_empty() {
            String::new()
        } else {
            format!("/p/{username}/{}/@v/{v}", page.slug)
        }
    });
    PageInfo {
        slug: page.slug.clone(),
        visibility: page.visibility.clone(),
        title: page.title.clone(),
        file_count: page.file_count,
        total_bytes: page.total_bytes,
        created_at: page.created_at,
        updated_at: page.updated_at,
        url_path,
        preview_url_path,
        deployed_version_id: page.deployed_version_id.clone(),
    }
}

async fn maybe_migrate_legacy_page(state: &AppState, page: &PageRow) {
    maybe_migrate_legacy_page_by_ids(state, &page.user_id, &page.slug).await;
}

async fn maybe_migrate_legacy_page_by_ids(state: &AppState, user_id: &str, slug: &str) {
    let Some(db) = state.db.as_ref() else {
        return;
    };
    let Ok(Some(page)) = PageRow::get(db, user_id, slug).await else {
        return;
    };
    if page.deployed_version_id.is_some() {
        return;
    }
    let Ok(count) = PageVersionRow::count_for_page(db, user_id, slug).await else {
        return;
    };
    if count > 0 {
        return;
    }
    let legacy = page_legacy_asset_key(user_id, slug);
    if !state.asset_store.has_room_files(&legacy) {
        return;
    }
    let version_id = "v1".to_string();
    let version_key = page_version_asset_key(user_id, slug, &version_id);
    if state.asset_store.copy_room(&legacy, &version_key).is_err() {
        return;
    }
    let entries = state.asset_store.list_room_entries(&version_key);
    let has_worker = entries.iter().any(|(p, _)| p == WORKER_ENTRY_PATH);
    let _ = PageVersionRow::insert(
        db,
        user_id,
        slug,
        &version_id,
        &page.title,
        page.file_count,
        page.total_bytes,
        has_worker,
        "migrated",
    )
    .await;
    let _ = PageRow::set_deployed_version(
        db,
        user_id,
        slug,
        &version_id,
        page.file_count,
        page.total_bytes,
        &page.title,
    )
    .await;
    tracing::info!("Migrated legacy page {user_id}/{slug} to version v1");
}

async fn enforce_visibility(
    state: &AppState,
    headers: &HeaderMap,
    page: &PageWithUsername,
) -> Result<(), StatusCode> {
    let visibility = page
        .visibility_enum()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    match visibility {
        PageVisibility::Public => Ok(()),
        PageVisibility::Relay => {
            resolve_viewer(state, headers).await?;
            Ok(())
        }
        PageVisibility::Private => {
            // Return NOT_FOUND for any auth failure so the existence of a
            // private page is not revealed to anonymous or foreign viewers.
            let viewer = resolve_viewer(state, headers)
                .await
                .map_err(|_| StatusCode::NOT_FOUND)?;
            if viewer.user_id != page.user_id {
                return Err(StatusCode::NOT_FOUND);
            }
            Ok(())
        }
    }
}

async fn resolve_viewer(state: &AppState, headers: &HeaderMap) -> Result<AuthUser, StatusCode> {
    // Only the Authorization header is accepted for viewer auth. Query-string
    // tokens are deliberately unsupported: published pages serve arbitrary
    // same-origin user JS, which could read tokens from `location.search`.
    let token = extract_bearer_token(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    validate_token(state, &token).await
}

fn process_check_page_files(
    asset_store: Arc<dyn WebAssetStore>,
    asset_key: &str,
    files: Vec<FileManifestEntry>,
) -> CheckPageFilesResponse {
    let mut needed = Vec::new();
    let mut existing_count = 0usize;
    let total_count = files.len();
    for entry in files {
        if crate::validated_asset_relative_path(&entry.path).is_err()
            || !crate::is_valid_content_hash(&entry.hash)
        {
            needed.push(entry.path);
            continue;
        }
        if asset_store.has_content(&entry.hash) {
            existing_count += 1;
            let _ = asset_store.map_to_room(asset_key, &entry.path, &entry.hash);
        } else {
            needed.push(entry.path);
        }
    }
    CheckPageFilesResponse {
        needed,
        existing_count,
        total_count,
    }
}

fn process_upload_page_files(
    asset_store: Arc<dyn WebAssetStore>,
    asset_key: &str,
    files: HashMap<String, UploadFileEntry>,
) -> Result<usize, StatusCode> {
    // Decode and verify the whole batch before writing anything, then enforce
    // the cumulative draft quota up front. Checking only after storing would
    // let repeated rejected batches bloat the content-addressed store.
    let mut decoded_files: Vec<(String, String, Vec<u8>)> = Vec::with_capacity(files.len());
    let mut batch_bytes: u64 = 0;
    for (rel_path, entry) in files {
        crate::validated_asset_relative_path(&rel_path).map_err(|_| StatusCode::BAD_REQUEST)?;
        if !crate::is_valid_content_hash(&entry.hash) {
            return Err(StatusCode::BAD_REQUEST);
        }
        let decoded = B64
            .decode(&entry.content)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        if decoded.len() as u64 > MAX_FILE_BYTES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        let actual_hash = hex_sha256(&decoded);
        if actual_hash != entry.hash {
            return Err(StatusCode::BAD_REQUEST);
        }
        batch_bytes = batch_bytes.saturating_add(decoded.len() as u64);
        decoded_files.push((rel_path, actual_hash, decoded));
    }
    if asset_store
        .room_total_bytes(asset_key)
        .saturating_add(batch_bytes)
        > MAX_PAGE_BYTES
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let mut stored = 0usize;
    for (rel_path, actual_hash, decoded) in decoded_files {
        if !asset_store.has_content(&actual_hash) {
            asset_store
                .store_content(&actual_hash, decoded)
                .map_err(crate::asset_store_error_status)?;
            stored += 1;
        }
        asset_store
            .map_to_room(asset_key, &rel_path, &actual_hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok(stored)
}

fn hex_sha256(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn mime_from_path(p: &str) -> &'static str {
    match p.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connect, AuthToken, DeviceRow, UserRow};
    use crate::relay::RoomManager;
    use crate::MemoryAssetStore;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn setup_app() -> (axum::Router, String, String) {
        let pool = connect(":memory:").await.unwrap();
        let pool = Arc::new(pool);
        UserRow::create(&pool, "u1", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        UserRow::create(&pool, "u2", "bob", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&pool, "d1", "u1", "Laptop", None)
            .await
            .unwrap();
        DeviceRow::upsert(&pool, "d2", "u2", "Phone", None)
            .await
            .unwrap();
        let tok_alice = AuthToken::create(&pool, "u1", "d1").await.unwrap();
        let tok_bob = AuthToken::create(&pool, "u2", "d2").await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let page_data_dir = tmp.path().join("page-data");
        std::mem::forget(tmp);

        let app = crate::build_relay_router_with_page_data(
            RoomManager::new(),
            Arc::new(MemoryAssetStore::new()),
            std::time::Instant::now(),
            Some(pool),
            "test",
            Some(page_data_dir),
        );
        (app, tok_alice.token, tok_bob.token)
    }

    async fn save_and_deploy(app: &axum::Router, token: &str, slug: &str, html: &str) -> String {
        save_and_deploy_with_visibility(app, token, slug, html, "public").await
    }

    async fn save_and_deploy_with_visibility(
        app: &axum::Router,
        token: &str,
        slug: &str,
        html: &str,
        visibility: &str,
    ) -> String {
        let hash = hex_sha256(html.as_bytes());
        let b64 = B64.encode(html.as_bytes());
        let upload = serde_json::json!({
            "slug": slug,
            "title": slug,
            "visibility": visibility,
            "finalize": true,
            "files": { "index.html": { "content": b64, "hash": hash } }
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/upload-files")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(upload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let freeze = serde_json::json!({ "title": slug, "note": "test" });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/pages/{slug}/versions"))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(freeze.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let version_id = v["version_id"].as_str().unwrap().to_string();

        let deploy = serde_json::json!({ "version_id": version_id });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/pages/{slug}/deploy"))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(deploy.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        version_id
    }

    #[tokio::test]
    async fn save_does_not_publish_until_deploy() {
        let (app, alice, _) = setup_app().await;
        let hash = hex_sha256(b"<html>draft</html>");
        let b64 = B64.encode(b"<html>draft</html>");
        let upload = serde_json::json!({
            "slug": "staged",
            "title": "staged",
            "visibility": "public",
            "files": { "index.html": { "content": b64, "hash": hash } }
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/upload-files")
                    .header("Authorization", format!("Bearer {alice}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(upload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/p/alice/staged")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let version_id = save_and_deploy(&app, &alice, "staged2", "<html>live</html>").await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/p/alice/staged2")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/p/alice/staged2/@v/{version_id}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn worker_can_use_kv() {
        let (app, alice, _) = setup_app().await;
        let worker = r#"
            function fetch(request, env) {
              if (request.path === "/api/hello") {
                env.KV.put("msg", "hi");
                return { status: 200, headers: { "content-type": "text/plain" }, body: env.KV.get("msg") };
              }
              return env.ASSETS.fetch("index.html");
            }
        "#;
        let files = [
            ("index.html", b"<html>static</html>".as_slice()),
            ("server/worker.js", worker.as_bytes()),
        ];
        let mut map = serde_json::Map::new();
        for (path, content) in files {
            let hash = hex_sha256(content);
            map.insert(
                path.to_string(),
                serde_json::json!({ "content": B64.encode(content), "hash": hash }),
            );
        }
        let upload = serde_json::json!({
            "slug": "fn",
            "title": "fn",
            "visibility": "public",
            "files": map
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/upload-files")
                    .header("Authorization", format!("Bearer {alice}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(upload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let freeze = serde_json::json!({});
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/fn/versions")
                    .header("Authorization", format!("Bearer {alice}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(freeze.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let version_id = v["version_id"].as_str().unwrap();
        assert_eq!(v["has_worker"], true);

        let deploy = serde_json::json!({ "version_id": version_id });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/fn/deploy")
                    .header("Authorization", format!("Bearer {alice}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(deploy.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/p/alice/fn/api/hello"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&body), "hi");
    }

    async fn get_page(app: &axum::Router, uri: &str, token: Option<&str>) -> StatusCode {
        let mut builder = Request::builder().uri(uri);
        if let Some(token) = token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        let resp = app
            .clone()
            .oneshot(builder.body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        resp.status()
    }

    #[tokio::test]
    async fn visibility_matrix_private_relay_public() {
        let (app, alice, bob) = setup_app().await;
        save_and_deploy_with_visibility(&app, &alice, "priv", "<html>p</html>", "private").await;
        save_and_deploy_with_visibility(&app, &alice, "rel", "<html>r</html>", "relay").await;
        save_and_deploy(&app, &alice, "pub", "<html>u</html>").await;

        // Private: hidden from anonymous and foreign users, visible to owner.
        assert_eq!(
            get_page(&app, "/p/alice/priv", None).await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_page(&app, "/p/alice/priv", Some(&bob)).await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_page(&app, "/p/alice/priv", Some(&alice)).await,
            StatusCode::OK
        );
        // Query-string tokens must be ignored (same-origin JS could steal them).
        assert_eq!(
            get_page(&app, &format!("/p/alice/priv?access_token={alice}"), None).await,
            StatusCode::NOT_FOUND
        );

        // Relay: any authenticated relay user may view, anonymous may not.
        assert_eq!(
            get_page(&app, "/p/alice/rel", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get_page(&app, "/p/alice/rel", Some(&bob)).await,
            StatusCode::OK
        );
        assert_eq!(
            get_page(&app, "/p/alice/rel", Some(&alice)).await,
            StatusCode::OK
        );

        // Public: no credentials needed.
        assert_eq!(get_page(&app, "/p/alice/pub", None).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn worker_does_not_receive_credential_headers() {
        let (app, alice, _) = setup_app().await;
        let worker = r#"
            function fetch(request, env) {
              const leaked = request.headers["authorization"] || request.headers["cookie"] || "none";
              return { status: 200, headers: { "content-type": "text/plain" }, body: leaked };
            }
        "#;
        let files = [("server/worker.js", worker.as_bytes())];
        let mut map = serde_json::Map::new();
        for (path, content) in files {
            let hash = hex_sha256(content);
            map.insert(
                path.to_string(),
                serde_json::json!({ "content": B64.encode(content), "hash": hash }),
            );
        }
        let upload = serde_json::json!({
            "slug": "hdr",
            "title": "hdr",
            "visibility": "relay",
            "files": map
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/upload-files")
                    .header("Authorization", format!("Bearer {alice}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(upload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/hdr/versions")
                    .header("Authorization", format!("Bearer {alice}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let version_id = v["version_id"].as_str().unwrap();
        let deploy = serde_json::json!({ "version_id": version_id });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/pages/hdr/deploy")
                    .header("Authorization", format!("Bearer {alice}"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(deploy.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Viewer presents a valid bearer token to pass the relay gate; the
        // worker must not see it.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/p/alice/hdr/api/echo")
                    .header("Authorization", format!("Bearer {alice}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&body), "none");
    }

    #[tokio::test]
    async fn freeze_records_real_stats() {
        let (app, alice, _) = setup_app().await;
        let html = "<html>real bytes</html>";
        save_and_deploy(&app, &alice, "stats", html).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/pages")
                    .header("Authorization", format!("Bearer {alice}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let pages: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let page = &pages[0];
        assert_eq!(page["file_count"].as_i64().unwrap(), 1);
        assert_eq!(page["total_bytes"].as_i64().unwrap(), html.len() as i64);
    }
}
