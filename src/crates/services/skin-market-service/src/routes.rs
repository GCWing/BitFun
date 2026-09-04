use crate::artifacts::{ArtifactStore, MarketImageVariant};
use crate::auth::{AuthenticatedIdentity, IdentityVerifier};
use crate::config::SkinMarketConfig;
use crate::db::Database;
use crate::error::{SkinMarketError, SkinMarketResult};
use crate::package::validate_appearance_package;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post, put};
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::Utc;
use hmac::{Hmac, Mac};
use openbitfun_product_domains::appearance_market::{
    compute_appearance_review_bundle_hash, validate_appearance_market_slug,
    AppearanceAdminSubmissionDetail, AppearanceCursorPage, AppearanceMarketListingDetail,
    AppearanceMarketListingSummary, AppearanceMarketPackageMeta, AppearanceMarketPublicationStatus,
    AppearanceMarketRelease, AppearanceMarketSort, AppearanceMarketSubmission,
    AppearanceMarketSubmissionDraftRequest, AppearanceMarketSubmissionStatus,
    AppearanceMarketUserSummary, AppearanceReviewDecision, AppearanceReviewDecisionRequest,
    APPEARANCE_MARKET_API_VERSION, APPEARANCE_MARKET_DEFAULT_PAGE_SIZE,
    APPEARANCE_MARKET_MAX_PACKAGE_BYTES, APPEARANCE_MARKET_MAX_PAGE_SIZE,
    APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE,
};
use openbitfun_product_domains::product_release::OPENBITFUN_INITIAL_RELEASE_VERSION;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use sqlx::{Executor, QueryBuilder, Row, Sqlite};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct SkinMarketState {
    pub config: SkinMarketConfig,
    pub database: Database,
    pub artifacts: ArtifactStore,
    pub identity: IdentityVerifier,
    pub upload_permits: Arc<Semaphore>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListingQuery {
    q: Option<String>,
    mode: Option<String>,
    sort: Option<AppearanceMarketSort>,
    cursor: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageVariantQuery {
    variant: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubmissionQuery {
    status: Option<AppearanceMarketSubmissionStatus>,
    cursor: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ModerationReason {
    reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigResponse {
    api_version: &'static str,
    public_browse: bool,
    web_submissions_enabled: bool,
    package_content_type: &'static str,
    max_package_bytes: u64,
}

pub(crate) fn api_router(state: Arc<SkinMarketState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/config", get(config))
        .route("/listings", get(list_listings))
        .route("/listings/{slug}", get(get_listing))
        .route(
            "/listings/{slug}/releases/{release_number}/download",
            get(download_release),
        )
        .route("/artifacts/previews/{sha256}", get(get_preview))
        .route(
            "/submissions",
            post(create_submission).get(list_my_submissions),
        )
        .route(
            "/submissions/{submission_id}",
            get(get_my_submission).delete(withdraw_submission),
        )
        .route(
            "/submissions/{submission_id}/package",
            put(upload_submission_package),
        )
        .route(
            "/submissions/{submission_id}/submit",
            post(submit_submission),
        )
        .route("/admin/submissions", get(list_admin_submissions))
        .route(
            "/admin/submissions/{submission_id}",
            get(get_admin_submission),
        )
        .route(
            "/admin/submissions/{submission_id}/decision",
            post(review_submission),
        )
        .route("/admin/releases/{release_id}/yank", post(yank_release))
        .route(
            "/admin/listings/{listing_id}/unpublish",
            post(unpublish_listing),
        )
        .route("/{*rest}", any(api_not_found))
        .layer(axum::middleware::from_fn(normalize_api_rejection))
        // Uploads use a raw Request and enforce the 96 MiB package limit in
        // the authenticated handler. Keep every structured API extractor
        // small so malformed or unauthenticated JSON cannot reserve 96 MiB.
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

async fn normalize_api_rejection(request: Request, next: axum::middleware::Next) -> Response {
    let response = next.run(request).await;
    if !response.status().is_client_error()
        || response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json"))
    {
        return response;
    }
    match response.status() {
        StatusCode::PAYLOAD_TOO_LARGE => SkinMarketError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            "The Skin market API request body exceeds 64 KiB.",
        )
        .into_response(),
        StatusCode::METHOD_NOT_ALLOWED => SkinMarketError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "The HTTP method is not allowed for this Skin market API route.",
        )
        .into_response(),
        status
            if matches!(
                status,
                StatusCode::BAD_REQUEST
                    | StatusCode::UNPROCESSABLE_ENTITY
                    | StatusCode::UNSUPPORTED_MEDIA_TYPE
            ) =>
        {
            SkinMarketError::bad_request(
                "invalid_request",
                "The Skin market API request is malformed or has an unsupported content type.",
            )
            .into_response()
        }
        _ => response,
    }
}

async fn api_not_found() -> SkinMarketError {
    SkinMarketError::not_found("Unknown appearance marketplace API route.")
}

async fn health(State(state): State<Arc<SkinMarketState>>) -> impl IntoResponse {
    let database_ready = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(state.database.pool())
        .await
        .is_ok();
    (
        if database_ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(serde_json::json!({
            "status": if database_ready { "ok" } else { "degraded" },
            "database": database_ready,
            "identityVerifierConfigured": true
        })),
    )
}

async fn config(State(state): State<Arc<SkinMarketState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        api_version: APPEARANCE_MARKET_API_VERSION,
        public_browse: state.config.public_browse,
        web_submissions_enabled: false,
        package_content_type: APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE,
        max_package_bytes: APPEARANCE_MARKET_MAX_PACKAGE_BYTES,
    })
}

async fn list_listings(
    State(state): State<Arc<SkinMarketState>>,
    Query(query): Query<ListingQuery>,
) -> SkinMarketResult<Json<AppearanceCursorPage<AppearanceMarketListingSummary>>> {
    ensure_public_browse(&state)?;
    let limit = query
        .limit
        .unwrap_or(APPEARANCE_MARKET_DEFAULT_PAGE_SIZE)
        .clamp(1, APPEARANCE_MARKET_MAX_PAGE_SIZE);
    let offset = decode_cursor(query.cursor.as_deref())?;
    let mode = match query.mode.as_deref().map(str::trim) {
        None | Some("") | Some("all") => None,
        Some("light") => Some("light"),
        Some("dark") => Some("dark"),
        Some(_) => {
            return Err(SkinMarketError::bad_request(
                "invalid_mode",
                "Appearance mode must be light, dark, or all.",
            ))
        }
    };
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT l.id AS listing_id, l.slug, r.release_number, r.draft_json,
                r.package_meta_json, r.preview_sha256, r.published_at,
                u.github_id, u.login, u.avatar_url,
                l.download_count
                  + (SELECT COUNT(*) FROM download_days d WHERE d.listing_id = l.id)
                    AS download_count
         FROM listings l
         JOIN releases r ON r.id = l.latest_release_id
         JOIN users u ON u.id = l.owner_user_id
         WHERE l.is_published = 1 AND r.yanked_at IS NULL",
    );
    if let Some(search) = query.q.filter(|value| !value.trim().is_empty()) {
        let search = format!("%{}%", search.trim().to_ascii_lowercase());
        builder.push(" AND (lower(l.slug) LIKE ");
        builder.push_bind(search.clone());
        builder.push(" OR lower(json_extract(r.package_meta_json, '$.name')) LIKE ");
        builder.push_bind(search.clone());
        builder.push(" OR lower(json_extract(r.package_meta_json, '$.description')) LIKE ");
        builder.push_bind(search);
        builder.push(")");
    }
    if let Some(mode) = mode {
        builder.push(" AND json_extract(r.package_meta_json, '$.mode') = ");
        builder.push_bind(mode);
    }
    match query.sort.unwrap_or_default() {
        AppearanceMarketSort::Newest => {
            builder.push(" ORDER BY r.published_at DESC, l.id DESC");
        }
        AppearanceMarketSort::Downloads => {
            builder.push(" ORDER BY download_count DESC, r.published_at DESC, l.id DESC");
        }
    }
    builder.push(" LIMIT ");
    builder.push_bind(i64::from(limit + 1));
    builder.push(" OFFSET ");
    builder.push_bind(i64::from(offset));
    let rows = builder
        .build()
        .fetch_all(state.database.pool())
        .await
        .map_err(SkinMarketError::internal)?;
    let has_more = rows.len() > limit as usize;
    let mut items = Vec::with_capacity(rows.len().min(limit as usize));
    for row in rows.into_iter().take(limit as usize) {
        items.push(summary_from_row(&state, &row)?);
    }
    Ok(Json(AppearanceCursorPage {
        items,
        next_cursor: has_more.then(|| encode_cursor(offset + limit)),
    }))
}

async fn get_listing(
    State(state): State<Arc<SkinMarketState>>,
    Path(slug): Path<String>,
) -> SkinMarketResult<Json<AppearanceMarketListingDetail>> {
    ensure_public_browse(&state)?;
    Ok(Json(listing_detail_by_slug(&state, &slug).await?))
}

async fn download_release(
    State(state): State<Arc<SkinMarketState>>,
    method: Method,
    headers: HeaderMap,
    Path((slug, release_number)): Path<(String, u32)>,
) -> SkinMarketResult<Response> {
    ensure_public_browse(&state)?;
    let row = sqlx::query(
        "SELECT l.id AS listing_id, r.package_sha256, r.package_size, r.package_meta_json
         FROM listings l JOIN releases r ON r.listing_id = l.id
         WHERE l.slug = ? AND l.is_published = 1
           AND r.release_number = ? AND r.yanked_at IS NULL",
    )
    .bind(&slug)
    .bind(i64::from(release_number))
    .fetch_optional(state.database.pool())
    .await
    .map_err(SkinMarketError::internal)?
    .ok_or_else(|| {
        SkinMarketError::not_found("The requested appearance release is unavailable.")
    })?;
    let listing_id: String = row.get("listing_id");
    let package_sha256: String = row.get("package_sha256");
    let expected_size = row.get::<i64, _>("package_size") as u64;
    let meta: AppearanceMarketPackageMeta = parse_json(
        row.get::<String, _>("package_meta_json"),
        "package metadata",
    )?;
    let (file, content_length) = state.artifacts.open_package(&package_sha256).await?;
    if content_length != expected_size {
        return Err(SkinMarketError::internal(
            "appearance package artifact size does not match its release record",
        ));
    }
    if method != Method::HEAD {
        record_download(&state, &headers, &listing_id).await?;
    }
    let mut response = Response::new(if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(ReaderStream::new(file))
    });
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}-{}.openbitfun-appearance\"",
            slug, meta.package_version
        ))
        .map_err(SkinMarketError::internal)?,
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{package_sha256}\""))
            .map_err(SkinMarketError::internal)?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string()).map_err(SkinMarketError::internal)?,
    );
    Ok(response)
}

async fn get_preview(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
    Query(query): Query<ImageVariantQuery>,
) -> SkinMarketResult<Response> {
    validate_sha256(&sha256)?;
    let public_references: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM releases r
         JOIN listings l ON l.id = r.listing_id
         WHERE r.preview_sha256 = ? AND r.yanked_at IS NULL AND l.is_published = 1",
    )
    .bind(&sha256)
    .fetch_one(state.database.pool())
    .await
    .map_err(SkinMarketError::internal)?;
    let is_public = public_references > 0 && state.config.public_browse;
    if !is_public {
        let identity = state.identity.require(&headers, &state.database).await?;
        let private_references: i64 = if identity.is_admin {
            sqlx::query_scalar("SELECT COUNT(*) FROM submissions WHERE preview_sha256 = ?")
                .bind(&sha256)
                .fetch_one(state.database.pool())
                .await
                .map_err(SkinMarketError::internal)?
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM submissions WHERE preview_sha256 = ? AND owner_user_id = ?",
            )
            .bind(&sha256)
            .bind(identity.user.internal_id)
            .fetch_one(state.database.pool())
            .await
            .map_err(SkinMarketError::internal)?
        };
        if private_references == 0 {
            return Err(SkinMarketError::not_found(
                "The requested appearance preview is unavailable.",
            ));
        }
    }
    let variant = match query.variant.as_deref() {
        None => None,
        Some("compact-v1") => Some(MarketImageVariant::CompactV1),
        Some("large-v1") => Some(MarketImageVariant::LargeV1),
        Some(_) => {
            return Err(SkinMarketError::bad_request(
                "invalid_image_variant",
                "Image variant must be compact-v1 or large-v1.",
            ))
        }
    };
    let bytes = match variant {
        Some(variant) => {
            state
                .artifacts
                .read_preview_variant(&sha256, variant)
                .await?
        }
        None => state.artifacts.read_preview(&sha256).await?,
    };
    let content_length = bytes.len();
    let etag = match variant {
        Some(variant) => format!("\"{}-{}\"", sha256, variant.cache_key()),
        None => format!("\"{sha256}\""),
    };
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/webp"));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        if is_public {
            HeaderValue::from_static("public, max-age=31536000, immutable")
        } else {
            HeaderValue::from_static("private, no-store")
        },
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&etag).map_err(SkinMarketError::internal)?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string()).map_err(SkinMarketError::internal)?,
    );
    Ok(response)
}

async fn create_submission(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Json(request): Json<AppearanceMarketSubmissionDraftRequest>,
) -> SkinMarketResult<(StatusCode, Json<AppearanceMarketSubmission>)> {
    let identity = state
        .identity
        .require_write(&headers, &state.database)
        .await?;
    validate_draft(&request)?;
    validate_draft_target(&state, &identity, &request).await?;
    let now = Utc::now().timestamp();
    let submission_id = Uuid::new_v4().to_string();
    let draft_json = serde_json::to_string(&request).map_err(SkinMarketError::internal)?;
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    sqlx::query(
        "INSERT INTO submissions(
           id, listing_id, owner_user_id, slug, release_number, draft_json,
           status, created_at, updated_at
         ) VALUES(?, ?, ?, ?, ?, ?, 'draft', ?, ?)",
    )
    .bind(&submission_id)
    .bind(&request.listing_id)
    .bind(identity.user.internal_id)
    .bind(&request.slug)
    .bind(i64::from(request.release_number))
    .bind(draft_json)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            SkinMarketError::conflict(
                "submission_conflict",
                "An active submission already reserves this Skin release target.",
            )
        } else {
            SkinMarketError::internal(error)
        }
    })?;
    insert_audit(
        &mut *transaction,
        identity.user.internal_id,
        "submission_created",
        "submission",
        &submission_id,
        serde_json::json!({"slug": request.slug, "releaseNumber": request.release_number}),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)?;
    Ok((
        StatusCode::CREATED,
        Json(submission_by_id(&state, &submission_id, Some(identity.user.internal_id)).await?),
    ))
}

async fn upload_submission_package(
    State(state): State<Arc<SkinMarketState>>,
    Path(submission_id): Path<String>,
    request: Request,
) -> SkinMarketResult<Json<AppearanceMarketSubmission>> {
    let headers = request.headers().clone();
    ensure_package_content_type(&headers)?;
    let identity = state
        .identity
        .require_write(&headers, &state.database)
        .await?;
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > APPEARANCE_MARKET_MAX_PACKAGE_BYTES)
    {
        return Err(SkinMarketError::payload_too_large());
    }
    let row = sqlx::query("SELECT status FROM submissions WHERE id = ? AND owner_user_id = ?")
        .bind(&submission_id)
        .bind(identity.user.internal_id)
        .fetch_optional(state.database.pool())
        .await
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| SkinMarketError::not_found("Appearance submission was not found."))?;
    if row.get::<String, _>("status") != "draft" {
        return Err(SkinMarketError::conflict(
            "submission_not_editable",
            "Only draft appearance submissions can replace their package.",
        ));
    }
    let _upload_permit = state
        .upload_permits
        .clone()
        .acquire_owned()
        .await
        .map_err(SkinMarketError::internal)?;
    let body = axum::body::to_bytes(
        request.into_body(),
        APPEARANCE_MARKET_MAX_PACKAGE_BYTES as usize,
    )
    .await
    .map_err(|_| SkinMarketError::payload_too_large())?;
    let package_bytes = body;
    let validated = tokio::task::spawn_blocking(move || {
        validate_appearance_package(&package_bytes).map(|package| (package, package_bytes))
    })
    .await
    .map_err(SkinMarketError::internal)??;
    let (validated, package_bytes) = validated;
    let artifact_guard = state.artifacts.lock_mutations().await;
    state
        .artifacts
        .put_package(&artifact_guard, &validated.sha256, &package_bytes)
        .await?;
    state
        .artifacts
        .put_preview(
            &artifact_guard,
            &validated.preview.sha256,
            &validated.preview.bytes,
        )
        .await?;
    let package_meta_json =
        serde_json::to_string(&validated.meta).map_err(SkinMarketError::internal)?;
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    let updated = sqlx::query(
        "UPDATE submissions SET package_meta_json = ?, manifest_json = ?,
           package_sha256 = ?, package_size = ?, preview_sha256 = ?, updated_at = ?
         WHERE id = ? AND owner_user_id = ? AND status = 'draft'",
    )
    .bind(package_meta_json)
    .bind(validated.canonical_manifest_json)
    .bind(&validated.sha256)
    .bind(validated.size as i64)
    .bind(&validated.preview.sha256)
    .bind(Utc::now().timestamp())
    .bind(&submission_id)
    .bind(identity.user.internal_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    if updated.rows_affected() != 1 {
        return Err(SkinMarketError::conflict(
            "submission_changed",
            "The appearance submission changed while its package was being validated.",
        ));
    }
    insert_audit(
        &mut *transaction,
        identity.user.internal_id,
        "submission_package_uploaded",
        "submission",
        &submission_id,
        serde_json::json!({
            "packageSha256": validated.sha256,
            "previewSha256": validated.preview.sha256,
            "previewWidth": validated.preview.width,
            "previewHeight": validated.preview.height
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)?;
    Ok(Json(
        submission_by_id(&state, &submission_id, Some(identity.user.internal_id)).await?,
    ))
}

async fn submit_submission(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(submission_id): Path<String>,
) -> SkinMarketResult<Json<AppearanceMarketSubmission>> {
    let identity = state
        .identity
        .require_write(&headers, &state.database)
        .await?;
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    let row = sqlx::query(
        "SELECT listing_id, slug, release_number, draft_json, package_meta_json,
                package_sha256, preview_sha256, status
         FROM submissions WHERE id = ? AND owner_user_id = ?",
    )
    .bind(&submission_id)
    .bind(identity.user.internal_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?
    .ok_or_else(|| SkinMarketError::not_found("Appearance submission was not found."))?;
    if row.get::<String, _>("status") != "draft" {
        return Err(SkinMarketError::conflict(
            "submission_not_submittable",
            "Only draft appearance submissions can be submitted.",
        ));
    }
    let draft: AppearanceMarketSubmissionDraftRequest =
        parse_json(row.get("draft_json"), "submission draft")?;
    let package_meta: AppearanceMarketPackageMeta = parse_optional_json(
        row.try_get::<Option<String>, _>("package_meta_json")
            .map_err(SkinMarketError::internal)?,
        "package metadata",
    )?
    .ok_or_else(|| {
        SkinMarketError::conflict(
            "package_required",
            "An appearance package must be uploaded before submission.",
        )
    })?;
    let package_present = row
        .try_get::<Option<String>, _>("package_sha256")
        .map_err(SkinMarketError::internal)?
        .is_some();
    let preview_present = row
        .try_get::<Option<String>, _>("preview_sha256")
        .map_err(SkinMarketError::internal)?
        .is_some();
    if !package_present || !preview_present {
        return Err(SkinMarketError::conflict(
            "package_required",
            "The appearance package and normalized preview are required.",
        ));
    }
    validate_release_target(
        &mut transaction,
        identity.user.internal_id,
        &draft,
        &package_meta,
    )
    .await?;
    let now = Utc::now().timestamp();
    let updated = sqlx::query(
        "UPDATE submissions SET status = 'submitted', submitted_at = ?, updated_at = ?
         WHERE id = ? AND owner_user_id = ? AND status = 'draft'",
    )
    .bind(now)
    .bind(now)
    .bind(&submission_id)
    .bind(identity.user.internal_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    if updated.rows_affected() != 1 {
        return Err(SkinMarketError::conflict(
            "submission_changed",
            "The appearance submission changed while it was being submitted.",
        ));
    }
    insert_audit(
        &mut *transaction,
        identity.user.internal_id,
        "submission_submitted",
        "submission",
        &submission_id,
        serde_json::json!({"slug": draft.slug, "packageId": package_meta.package_id}),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)?;
    Ok(Json(
        submission_by_id(&state, &submission_id, Some(identity.user.internal_id)).await?,
    ))
}

async fn list_my_submissions(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Query(query): Query<SubmissionQuery>,
) -> SkinMarketResult<Json<AppearanceCursorPage<AppearanceMarketSubmission>>> {
    let identity = state.identity.require(&headers, &state.database).await?;
    Ok(Json(
        list_submissions(
            &state,
            Some(identity.user.internal_id),
            query.status,
            query.cursor.as_deref(),
            query.limit,
        )
        .await?,
    ))
}

async fn get_my_submission(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(submission_id): Path<String>,
) -> SkinMarketResult<Json<AppearanceMarketSubmission>> {
    let identity = state.identity.require(&headers, &state.database).await?;
    Ok(Json(
        submission_by_id(&state, &submission_id, Some(identity.user.internal_id)).await?,
    ))
}

async fn withdraw_submission(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(submission_id): Path<String>,
) -> SkinMarketResult<Json<AppearanceMarketSubmission>> {
    let identity = state
        .identity
        .require_write(&headers, &state.database)
        .await?;
    let now = Utc::now().timestamp();
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    let updated = sqlx::query(
        "UPDATE submissions SET status = 'withdrawn', updated_at = ?
         WHERE id = ? AND owner_user_id = ? AND status IN ('draft', 'submitted')",
    )
    .bind(now)
    .bind(&submission_id)
    .bind(identity.user.internal_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    if updated.rows_affected() != 1 {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM submissions WHERE id = ? AND owner_user_id = ?",
        )
        .bind(&submission_id)
        .bind(identity.user.internal_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(SkinMarketError::internal)?;
        return Err(if exists == 0 {
            SkinMarketError::not_found("Appearance submission was not found.")
        } else {
            SkinMarketError::conflict(
                "submission_not_withdrawable",
                "Only draft or submitted appearance entries may be withdrawn.",
            )
        });
    }
    insert_audit(
        &mut *transaction,
        identity.user.internal_id,
        "submission_withdrawn",
        "submission",
        &submission_id,
        serde_json::json!({}),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)?;
    Ok(Json(
        submission_by_id(&state, &submission_id, Some(identity.user.internal_id)).await?,
    ))
}

async fn list_admin_submissions(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Query(query): Query<SubmissionQuery>,
) -> SkinMarketResult<Json<AppearanceCursorPage<AppearanceMarketSubmission>>> {
    state
        .identity
        .require_admin(&headers, &state.database)
        .await?;
    Ok(Json(
        list_submissions(
            &state,
            None,
            Some(
                query
                    .status
                    .unwrap_or(AppearanceMarketSubmissionStatus::Submitted),
            ),
            query.cursor.as_deref(),
            query.limit,
        )
        .await?,
    ))
}

async fn get_admin_submission(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(submission_id): Path<String>,
) -> SkinMarketResult<Json<AppearanceAdminSubmissionDetail>> {
    state
        .identity
        .require_admin(&headers, &state.database)
        .await?;
    Ok(Json(admin_submission_detail(&state, &submission_id).await?))
}

async fn review_submission(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(submission_id): Path<String>,
    Json(request): Json<AppearanceReviewDecisionRequest>,
) -> SkinMarketResult<Json<AppearanceAdminSubmissionDetail>> {
    let admin = state
        .identity
        .require_admin_write(&headers, &state.database)
        .await?;
    match request.decision {
        AppearanceReviewDecision::Approve => {
            approve_submission(&state, &admin, &submission_id).await?;
        }
        AppearanceReviewDecision::Reject => {
            let reason = validate_reason(&request.reason)?;
            reject_submission(&state, &admin, &submission_id, reason).await?;
        }
    }
    Ok(Json(admin_submission_detail(&state, &submission_id).await?))
}

async fn reject_submission(
    state: &SkinMarketState,
    admin: &AuthenticatedIdentity,
    submission_id: &str,
    reason: &str,
) -> SkinMarketResult<()> {
    let now = Utc::now().timestamp();
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    let updated = sqlx::query(
        "UPDATE submissions SET status = 'rejected', rejection_reason = ?,
           reviewed_at = ?, reviewer_user_id = ?, updated_at = ?
         WHERE id = ? AND status = 'submitted'",
    )
    .bind(reason)
    .bind(now)
    .bind(admin.user.internal_id)
    .bind(now)
    .bind(submission_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    if updated.rows_affected() != 1 {
        return Err(SkinMarketError::conflict(
            "submission_not_reviewable",
            "Only submitted appearance entries may be rejected.",
        ));
    }
    insert_audit(
        &mut *transaction,
        admin.user.internal_id,
        "submission_rejected",
        "submission",
        submission_id,
        serde_json::json!({"reason": reason}),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)
}

async fn approve_submission(
    state: &SkinMarketState,
    admin: &AuthenticatedIdentity,
    submission_id: &str,
) -> SkinMarketResult<()> {
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    let row = sqlx::query(
        "SELECT listing_id, owner_user_id, slug, release_number, draft_json,
                package_meta_json, manifest_json, package_sha256, package_size,
                preview_sha256, status
         FROM submissions WHERE id = ?",
    )
    .bind(submission_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?
    .ok_or_else(|| SkinMarketError::not_found("Appearance submission was not found."))?;
    if row.get::<String, _>("status") != "submitted" {
        return Err(SkinMarketError::conflict(
            "submission_not_reviewable",
            "Only submitted appearance entries may be approved.",
        ));
    }
    let owner_user_id: i64 = row.get("owner_user_id");
    let slug: String = row.get("slug");
    let release_number = row.get::<i64, _>("release_number") as u32;
    let draft_json: String = row.get("draft_json");
    let package_meta_json: String = row
        .try_get::<Option<String>, _>("package_meta_json")
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| {
            SkinMarketError::conflict("package_required", "Package metadata is missing.")
        })?;
    let manifest_json: String = row
        .try_get::<Option<String>, _>("manifest_json")
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| {
            SkinMarketError::conflict("package_required", "Package manifest is missing.")
        })?;
    let package_sha256: String = row
        .try_get::<Option<String>, _>("package_sha256")
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| {
            SkinMarketError::conflict("package_required", "Package artifact is missing.")
        })?;
    let package_size: i64 = row
        .try_get::<Option<i64>, _>("package_size")
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| SkinMarketError::conflict("package_required", "Package size is missing."))?;
    let preview_sha256: String = row
        .try_get::<Option<String>, _>("preview_sha256")
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| {
            SkinMarketError::conflict("preview_required", "Preview artifact is missing.")
        })?;
    let draft: AppearanceMarketSubmissionDraftRequest =
        parse_json(draft_json.clone(), "submission draft")?;
    let package_meta: AppearanceMarketPackageMeta =
        parse_json(package_meta_json.clone(), "package metadata")?;
    validate_release_target(&mut transaction, owner_user_id, &draft, &package_meta).await?;

    let now = Utc::now().timestamp();
    let listing_id = if let Some(listing_id) = row
        .try_get::<Option<String>, _>("listing_id")
        .map_err(SkinMarketError::internal)?
    {
        listing_id
    } else {
        let listing_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO listings(
               id, slug, package_id, owner_user_id, is_published, created_at, updated_at
             ) VALUES(?, ?, ?, ?, 0, ?, ?)",
        )
        .bind(&listing_id)
        .bind(&slug)
        .bind(&package_meta.package_id)
        .bind(owner_user_id)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            if is_unique_violation(&error) {
                listing_identity_conflict(&error)
            } else {
                SkinMarketError::internal(error)
            }
        })?;
        listing_id
    };
    let canonical_metadata =
        canonical_review_metadata(&draft_json, &package_meta_json, &manifest_json)?;
    let review_bundle_hash = compute_appearance_review_bundle_hash(
        &package_sha256,
        &canonical_metadata,
        &preview_sha256,
    );
    let release_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO releases(
           id, listing_id, submission_id, release_number, draft_json,
           package_meta_json, manifest_json, package_sha256, package_size,
           preview_sha256, review_bundle_hash, published_at
         ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&release_id)
    .bind(&listing_id)
    .bind(submission_id)
    .bind(i64::from(release_number))
    .bind(&draft_json)
    .bind(&package_meta_json)
    .bind(&manifest_json)
    .bind(&package_sha256)
    .bind(package_size)
    .bind(&preview_sha256)
    .bind(&review_bundle_hash)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            SkinMarketError::conflict(
                "release_exists",
                "This appearance release number has already been published.",
            )
        } else {
            SkinMarketError::internal(error)
        }
    })?;
    sqlx::query(
        "UPDATE listings SET latest_release_id = ?, is_published = 1, updated_at = ?
         WHERE id = ?",
    )
    .bind(&release_id)
    .bind(now)
    .bind(&listing_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    sqlx::query(
        "UPDATE submissions SET listing_id = ?, status = 'approved', reviewer_user_id = ?,
           reviewed_at = ?, updated_at = ? WHERE id = ? AND status = 'submitted'",
    )
    .bind(&listing_id)
    .bind(admin.user.internal_id)
    .bind(now)
    .bind(now)
    .bind(submission_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    insert_audit(
        &mut *transaction,
        admin.user.internal_id,
        "submission_approved",
        "submission",
        submission_id,
        serde_json::json!({
            "listingId": listing_id,
            "releaseId": release_id,
            "reviewBundleHash": review_bundle_hash
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)
}

async fn yank_release(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(release_id): Path<String>,
    Json(request): Json<ModerationReason>,
) -> SkinMarketResult<StatusCode> {
    let admin = state
        .identity
        .require_admin_write(&headers, &state.database)
        .await?;
    let reason = validate_reason(&request.reason)?;
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    let row = sqlx::query("SELECT listing_id, submission_id, yanked_at FROM releases WHERE id = ?")
        .bind(&release_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| SkinMarketError::not_found("Appearance release was not found."))?;
    if row
        .try_get::<Option<i64>, _>("yanked_at")
        .map_err(SkinMarketError::internal)?
        .is_some()
    {
        return Err(SkinMarketError::conflict(
            "release_already_yanked",
            "The appearance release has already been yanked.",
        ));
    }
    let listing_id: String = row.get("listing_id");
    let submission_id: String = row.get("submission_id");
    let now = Utc::now().timestamp();
    sqlx::query("UPDATE releases SET yanked_at = ?, yank_reason = ? WHERE id = ?")
        .bind(now)
        .bind(reason)
        .bind(&release_id)
        .execute(&mut *transaction)
        .await
        .map_err(SkinMarketError::internal)?;
    let replacement: Option<String> = sqlx::query_scalar(
        "SELECT id FROM releases
         WHERE listing_id = ? AND yanked_at IS NULL
         ORDER BY release_number DESC LIMIT 1",
    )
    .bind(&listing_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    sqlx::query(
        "UPDATE listings SET latest_release_id = ?, is_published = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&replacement)
    .bind(i64::from(replacement.is_some()))
    .bind(now)
    .bind(&listing_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    sqlx::query("UPDATE submissions SET updated_at = ? WHERE id = ? AND status = 'approved'")
        .bind(now)
        .bind(&submission_id)
        .execute(&mut *transaction)
        .await
        .map_err(SkinMarketError::internal)?;
    insert_audit(
        &mut *transaction,
        admin.user.internal_id,
        "release_yanked",
        "release",
        &release_id,
        serde_json::json!({"reason": reason, "listingId": listing_id}),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn unpublish_listing(
    State(state): State<Arc<SkinMarketState>>,
    headers: HeaderMap,
    Path(listing_id): Path<String>,
    Json(request): Json<ModerationReason>,
) -> SkinMarketResult<StatusCode> {
    let admin = state
        .identity
        .require_admin_write(&headers, &state.database)
        .await?;
    let reason = validate_reason(&request.reason)?;
    let mut transaction = state
        .database
        .pool()
        .begin()
        .await
        .map_err(SkinMarketError::internal)?;
    let now = Utc::now().timestamp();
    let updated = sqlx::query(
        "UPDATE listings SET is_published = 0, updated_at = ?
         WHERE id = ? AND is_published = 1",
    )
    .bind(now)
    .bind(&listing_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    if updated.rows_affected() != 1 {
        return Err(SkinMarketError::not_found(
            "Published appearance listing was not found.",
        ));
    }
    sqlx::query(
        "UPDATE submissions SET updated_at = ?
         WHERE listing_id = ? AND status = 'approved'",
    )
    .bind(now)
    .bind(&listing_id)
    .execute(&mut *transaction)
    .await
    .map_err(SkinMarketError::internal)?;
    insert_audit(
        &mut *transaction,
        admin.user.internal_id,
        "listing_unpublished",
        "listing",
        &listing_id,
        serde_json::json!({"reason": reason}),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(SkinMarketError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn listing_detail_by_slug(
    state: &SkinMarketState,
    slug: &str,
) -> SkinMarketResult<AppearanceMarketListingDetail> {
    let row = sqlx::query(
        "SELECT l.id AS listing_id, l.slug, r.release_number, r.draft_json,
                r.package_meta_json, r.preview_sha256, r.published_at,
                u.github_id, u.login, u.avatar_url,
                l.download_count
                  + (SELECT COUNT(*) FROM download_days d WHERE d.listing_id = l.id)
                    AS download_count
         FROM listings l
         JOIN releases r ON r.id = l.latest_release_id
         JOIN users u ON u.id = l.owner_user_id
         WHERE l.slug = ? AND l.is_published = 1 AND r.yanked_at IS NULL",
    )
    .bind(slug)
    .fetch_optional(state.database.pool())
    .await
    .map_err(SkinMarketError::internal)?
    .ok_or_else(|| SkinMarketError::not_found("Appearance listing was not found."))?;
    let summary = summary_from_row(state, &row)?;
    let draft: AppearanceMarketSubmissionDraftRequest =
        parse_json(row.get::<String, _>("draft_json"), "release draft")?;
    let rows = sqlx::query(
        "SELECT id, listing_id, release_number, draft_json, package_meta_json,
                package_sha256, package_size, review_bundle_hash, published_at, yanked_at
         FROM releases WHERE listing_id = ? ORDER BY release_number DESC",
    )
    .bind(&summary.listing_id)
    .fetch_all(state.database.pool())
    .await
    .map_err(SkinMarketError::internal)?;
    let releases = rows
        .iter()
        .map(release_from_row)
        .collect::<SkinMarketResult<Vec<_>>>()?;
    Ok(AppearanceMarketListingDetail {
        summary,
        changelog: draft.changelog,
        license: draft.license,
        repository_url: draft.repository_url,
        releases,
    })
}

fn summary_from_row(
    state: &SkinMarketState,
    row: &sqlx::sqlite::SqliteRow,
) -> SkinMarketResult<AppearanceMarketListingSummary> {
    let draft: AppearanceMarketSubmissionDraftRequest =
        parse_json(row.get::<String, _>("draft_json"), "release draft")?;
    let meta: AppearanceMarketPackageMeta = parse_json(
        row.get::<String, _>("package_meta_json"),
        "package metadata",
    )?;
    let preview_sha256: String = row.get("preview_sha256");
    Ok(AppearanceMarketListingSummary {
        listing_id: row.get("listing_id"),
        slug: row.get("slug"),
        package_id: meta.package_id,
        name: meta.name,
        description: meta.description,
        author: meta.author,
        mode: meta.mode,
        package_version: meta.package_version,
        latest_release: row.get::<i64, _>("release_number") as u32,
        min_openbitfun_version: draft.min_openbitfun_version,
        required_capabilities: meta.required_capabilities,
        owner: AppearanceMarketUserSummary {
            github_id: row.get("github_id"),
            login: row.get("login"),
            avatar_url: row.get("avatar_url"),
        },
        preview_url: format!(
            "{}/api/v1/artifacts/previews/{preview_sha256}",
            state.config.public_base_url
        ),
        download_count: row.get::<i64, _>("download_count").max(0) as u64,
        published_at: row.get("published_at"),
    })
}

fn release_from_row(row: &sqlx::sqlite::SqliteRow) -> SkinMarketResult<AppearanceMarketRelease> {
    let draft: AppearanceMarketSubmissionDraftRequest =
        parse_json(row.get::<String, _>("draft_json"), "release draft")?;
    let meta: AppearanceMarketPackageMeta = parse_json(
        row.get::<String, _>("package_meta_json"),
        "package metadata",
    )?;
    Ok(AppearanceMarketRelease {
        release_id: row.get("id"),
        listing_id: row.get("listing_id"),
        release_number: row.get::<i64, _>("release_number") as u32,
        package_version: meta.package_version,
        min_openbitfun_version: draft.min_openbitfun_version,
        package_sha256: row.get("package_sha256"),
        package_size: row.get::<i64, _>("package_size") as u64,
        review_bundle_hash: row.get("review_bundle_hash"),
        published_at: row.get("published_at"),
        yanked: row
            .try_get::<Option<i64>, _>("yanked_at")
            .map_err(SkinMarketError::internal)?
            .is_some(),
    })
}

async fn list_submissions(
    state: &SkinMarketState,
    owner_user_id: Option<i64>,
    status: Option<AppearanceMarketSubmissionStatus>,
    cursor: Option<&str>,
    limit: Option<u32>,
) -> SkinMarketResult<AppearanceCursorPage<AppearanceMarketSubmission>> {
    let limit = limit
        .unwrap_or(APPEARANCE_MARKET_DEFAULT_PAGE_SIZE)
        .clamp(1, APPEARANCE_MARKET_MAX_PAGE_SIZE);
    let cursor = decode_submission_cursor(cursor)?;
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT s.id, s.listing_id, s.owner_user_id, s.slug, s.release_number, s.draft_json,
                s.package_meta_json, s.status, s.package_sha256, s.package_size,
                s.preview_sha256, s.rejection_reason, s.created_at, s.updated_at,
                l.is_published AS listing_is_published,
                r.yanked_at AS release_yanked_at
         FROM submissions s
         LEFT JOIN listings l ON l.id = s.listing_id
         LEFT JOIN releases r ON r.submission_id = s.id
         WHERE 1 = 1",
    );
    if let Some(owner_user_id) = owner_user_id {
        builder.push(" AND s.owner_user_id = ");
        builder.push_bind(owner_user_id);
    }
    if let Some(status) = status {
        builder.push(" AND s.status = ");
        builder.push_bind(status_string(status));
    }
    if let Some((updated_at, submission_id)) = cursor {
        builder.push(" AND (s.updated_at < ");
        builder.push_bind(updated_at);
        builder.push(" OR (s.updated_at = ");
        builder.push_bind(updated_at);
        builder.push(" AND s.id < ");
        builder.push_bind(submission_id);
        builder.push("))");
    }
    builder.push(" ORDER BY s.updated_at DESC, s.id DESC LIMIT ");
    builder.push_bind(i64::from(limit + 1));
    let rows = builder
        .build()
        .fetch_all(state.database.pool())
        .await
        .map_err(SkinMarketError::internal)?;
    let has_more = rows.len() > limit as usize;
    let items = rows
        .iter()
        .take(limit as usize)
        .map(|row| submission_from_row(state, row))
        .collect::<SkinMarketResult<Vec<_>>>()?;
    let next_cursor = has_more.then(|| {
        let last = items
            .last()
            .expect("a paginated submission page with more rows is non-empty");
        encode_submission_cursor(last.updated_at, &last.submission_id)
    });
    Ok(AppearanceCursorPage { items, next_cursor })
}

async fn submission_by_id(
    state: &SkinMarketState,
    submission_id: &str,
    owner_user_id: Option<i64>,
) -> SkinMarketResult<AppearanceMarketSubmission> {
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT s.id, s.listing_id, s.owner_user_id, s.slug, s.release_number, s.draft_json,
                s.package_meta_json, s.status, s.package_sha256, s.package_size,
                s.preview_sha256, s.rejection_reason, s.created_at, s.updated_at,
                l.is_published AS listing_is_published,
                r.yanked_at AS release_yanked_at
         FROM submissions s
         LEFT JOIN listings l ON l.id = s.listing_id
         LEFT JOIN releases r ON r.submission_id = s.id
         WHERE s.id = ",
    );
    builder.push_bind(submission_id);
    if let Some(owner_user_id) = owner_user_id {
        builder.push(" AND s.owner_user_id = ");
        builder.push_bind(owner_user_id);
    }
    let row = builder
        .build()
        .fetch_optional(state.database.pool())
        .await
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| SkinMarketError::not_found("Appearance submission was not found."))?;
    submission_from_row(state, &row)
}

fn submission_from_row(
    state: &SkinMarketState,
    row: &sqlx::sqlite::SqliteRow,
) -> SkinMarketResult<AppearanceMarketSubmission> {
    let draft: AppearanceMarketSubmissionDraftRequest =
        parse_json(row.get::<String, _>("draft_json"), "submission draft")?;
    let meta: Option<AppearanceMarketPackageMeta> = parse_optional_json(
        row.try_get::<Option<String>, _>("package_meta_json")
            .map_err(SkinMarketError::internal)?,
        "package metadata",
    )?;
    let preview_sha256 = row
        .try_get::<Option<String>, _>("preview_sha256")
        .map_err(SkinMarketError::internal)?;
    let status = parse_submission_status(&row.get::<String, _>("status"))?;
    let publication_status = if status == AppearanceMarketSubmissionStatus::Approved {
        let release_yanked_at = row
            .try_get::<Option<i64>, _>("release_yanked_at")
            .map_err(SkinMarketError::internal)?;
        let listing_is_published = row
            .try_get::<Option<i64>, _>("listing_is_published")
            .map_err(SkinMarketError::internal)?;
        Some(if release_yanked_at.is_some() {
            AppearanceMarketPublicationStatus::Yanked
        } else if listing_is_published == Some(1) {
            AppearanceMarketPublicationStatus::Published
        } else {
            AppearanceMarketPublicationStatus::Unpublished
        })
    } else {
        None
    };
    Ok(AppearanceMarketSubmission {
        submission_id: row.get("id"),
        listing_id: row
            .try_get("listing_id")
            .map_err(SkinMarketError::internal)?,
        slug: row.get("slug"),
        release_number: row.get::<i64, _>("release_number") as u32,
        package_id: meta.as_ref().map(|meta| meta.package_id.clone()),
        name: meta.as_ref().map(|meta| meta.name.clone()),
        description: meta.as_ref().map(|meta| meta.description.clone()),
        author: meta.as_ref().and_then(|meta| meta.author.clone()),
        mode: meta.as_ref().map(|meta| meta.mode),
        package_version: meta.as_ref().map(|meta| meta.package_version.clone()),
        min_openbitfun_version: draft.min_openbitfun_version,
        required_capabilities: meta
            .map(|meta| meta.required_capabilities)
            .unwrap_or_default(),
        changelog: draft.changelog,
        license: draft.license,
        repository_url: draft.repository_url,
        status,
        publication_status,
        package_sha256: row
            .try_get("package_sha256")
            .map_err(SkinMarketError::internal)?,
        package_size: row
            .try_get::<Option<i64>, _>("package_size")
            .map_err(SkinMarketError::internal)?
            .map(|value| value as u64),
        preview_url: preview_sha256.map(|sha256| {
            format!(
                "{}/api/v1/artifacts/previews/{sha256}",
                state.config.public_base_url
            )
        }),
        rejection_reason: row
            .try_get("rejection_reason")
            .map_err(SkinMarketError::internal)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn admin_submission_detail(
    state: &SkinMarketState,
    submission_id: &str,
) -> SkinMarketResult<AppearanceAdminSubmissionDetail> {
    let row = sqlx::query(
        "SELECT s.manifest_json, s.package_sha256, s.preview_sha256, s.draft_json,
                s.package_meta_json, u.github_id, u.login, u.avatar_url
         FROM submissions s
         LEFT JOIN users u ON u.id = s.owner_user_id
         WHERE s.id = ?",
    )
    .bind(submission_id)
    .fetch_optional(state.database.pool())
    .await
    .map_err(SkinMarketError::internal)?
    .ok_or_else(|| SkinMarketError::not_found("Appearance submission was not found."))?;
    let manifest_json = row
        .try_get::<Option<String>, _>("manifest_json")
        .map_err(SkinMarketError::internal)?;
    let package_sha256 = row
        .try_get::<Option<String>, _>("package_sha256")
        .map_err(SkinMarketError::internal)?;
    let preview_sha256 = row
        .try_get::<Option<String>, _>("preview_sha256")
        .map_err(SkinMarketError::internal)?;
    let review_bundle_hash = match (
        package_sha256.as_deref(),
        preview_sha256.as_deref(),
        manifest_json.as_deref(),
        row.try_get::<Option<String>, _>("package_meta_json")
            .map_err(SkinMarketError::internal)?
            .as_deref(),
    ) {
        (Some(package), Some(preview), Some(manifest), Some(package_meta)) => {
            Some(compute_appearance_review_bundle_hash(
                package,
                &canonical_review_metadata(
                    &row.get::<String, _>("draft_json"),
                    package_meta,
                    manifest,
                )?,
                preview,
            ))
        }
        _ => None,
    };
    let submitter = row
        .try_get::<Option<i64>, _>("github_id")
        .map_err(SkinMarketError::internal)?
        .map(|github_id| {
            Ok::<_, SkinMarketError>(AppearanceMarketUserSummary {
                github_id,
                login: row.try_get("login").map_err(SkinMarketError::internal)?,
                avatar_url: row
                    .try_get("avatar_url")
                    .map_err(SkinMarketError::internal)?,
            })
        })
        .transpose()?;
    Ok(AppearanceAdminSubmissionDetail {
        submission: submission_by_id(state, submission_id, None).await?,
        submitter,
        manifest: manifest_json
            .map(|value| parse_json(value, "appearance manifest"))
            .transpose()?,
        package_sha256,
        preview_sha256,
        review_bundle_hash,
    })
}

fn validate_draft(request: &AppearanceMarketSubmissionDraftRequest) -> SkinMarketResult<()> {
    if !validate_appearance_market_slug(&request.slug) {
        return Err(SkinMarketError::bad_request(
            "invalid_slug",
            "Appearance slugs must be lowercase, dashed identifiers of 3 to 63 characters.",
        ));
    }
    if request.release_number == 0 {
        return Err(SkinMarketError::bad_request(
            "invalid_release_number",
            "Appearance release numbers start at 1.",
        ));
    }
    validate_min_openbitfun_version(&request.min_openbitfun_version)?;
    if request.changelog.chars().count() > 2_000 {
        return Err(SkinMarketError::bad_request(
            "invalid_changelog",
            "Appearance changelogs may contain at most 2000 characters.",
        ));
    }
    validate_license(&request.license)?;
    if let Some(repository_url) = &request.repository_url {
        if !is_safe_external_url(repository_url) {
            return Err(SkinMarketError::bad_request(
                "invalid_repository_url",
                "Repository URLs must be credential-free HTTPS URLs.",
            ));
        }
    }
    Ok(())
}

fn validate_min_openbitfun_version(value: &str) -> SkinMarketResult<()> {
    let version = Version::parse(value).map_err(|_| {
        SkinMarketError::bad_request(
            "invalid_min_openbitfun_version",
            "minOpenBitFunVersion must use semantic version syntax, for example 1.0.0.",
        )
    })?;
    if version < OPENBITFUN_INITIAL_RELEASE_VERSION {
        return Err(SkinMarketError::bad_request(
            "invalid_min_openbitfun_version",
            "minOpenBitFunVersion must be 1.0.0 or newer.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod minimum_version_tests {
    use super::validate_min_openbitfun_version;

    #[test]
    fn minimum_openbitfun_version_starts_at_initial_release() {
        assert!(validate_min_openbitfun_version("1.0.0").is_ok());
        assert!(validate_min_openbitfun_version("1.2.0").is_ok());

        let pre_release_identity = [0, 9, 0]
            .into_iter()
            .map(|part| part.to_string())
            .collect::<Vec<_>>()
            .join(".");
        assert!(validate_min_openbitfun_version(&pre_release_identity).is_err());
        assert!(validate_min_openbitfun_version("1.0.0-rc.1").is_err());
    }
}

fn validate_license(
    license: &openbitfun_product_domains::appearance_market::AppearanceMarketLicense,
) -> SkinMarketResult<()> {
    let spdx = license
        .spdx_expression
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let custom_url = license
        .custom_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if spdx.is_none() == custom_url.is_none() {
        return Err(SkinMarketError::bad_request(
            "invalid_license",
            "Declare exactly one SPDX expression or custom license URL.",
        ));
    }
    if spdx.is_some_and(|value| {
        value.len() > 120
            || !value.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '-' | '.' | '+' | '(' | ')' | ' ' | ':')
            })
    }) {
        return Err(SkinMarketError::bad_request(
            "invalid_license",
            "The SPDX license expression has an invalid shape.",
        ));
    }
    if custom_url.is_some_and(|value| !is_safe_external_url(value)) {
        return Err(SkinMarketError::bad_request(
            "invalid_license",
            "Custom license URLs must be credential-free HTTPS URLs.",
        ));
    }
    Ok(())
}

fn is_safe_external_url(value: &str) -> bool {
    value.len() <= 2_048
        && url::Url::parse(value).is_ok_and(|parsed| {
            parsed.scheme() == "https"
                && parsed.host_str().is_some()
                && parsed.username().is_empty()
                && parsed.password().is_none()
        })
}

async fn validate_draft_target(
    state: &SkinMarketState,
    identity: &AuthenticatedIdentity,
    draft: &AppearanceMarketSubmissionDraftRequest,
) -> SkinMarketResult<()> {
    if let Some(listing_id) = &draft.listing_id {
        let row = sqlx::query(
            "SELECT slug, owner_user_id,
                    COALESCE((SELECT MAX(release_number) FROM releases WHERE listing_id = l.id), 0) AS latest
             FROM listings l WHERE id = ?",
        )
        .bind(listing_id)
        .fetch_optional(state.database.pool())
        .await
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| SkinMarketError::not_found("Appearance listing was not found."))?;
        if row.get::<i64, _>("owner_user_id") != identity.user.internal_id {
            return Err(SkinMarketError::forbidden(
                "Only the listing owner may submit a new appearance release.",
            ));
        }
        if row.get::<String, _>("slug") != draft.slug
            || draft.release_number != row.get::<i64, _>("latest") as u32 + 1
        {
            return Err(SkinMarketError::conflict(
                "invalid_release_target",
                "The listing slug or next release number does not match.",
            ));
        }
    } else {
        if draft.release_number != 1 {
            return Err(SkinMarketError::bad_request(
                "invalid_release_number",
                "A new appearance listing must start at release 1.",
            ));
        }
        let conflicts: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM listings WHERE slug = ?)
                  + (SELECT COUNT(*) FROM submissions
                     WHERE slug = ? AND status IN ('draft', 'submitted'))",
        )
        .bind(&draft.slug)
        .bind(&draft.slug)
        .fetch_one(state.database.pool())
        .await
        .map_err(SkinMarketError::internal)?;
        if conflicts > 0 {
            return Err(SkinMarketError::conflict(
                "slug_unavailable",
                "The appearance listing slug is already reserved.",
            ));
        }
    }
    Ok(())
}

async fn validate_release_target(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    owner_user_id: i64,
    draft: &AppearanceMarketSubmissionDraftRequest,
    package_meta: &AppearanceMarketPackageMeta,
) -> SkinMarketResult<()> {
    if let Some(listing_id) = &draft.listing_id {
        let row = sqlx::query(
            "SELECT l.slug, l.package_id, l.owner_user_id,
                    COALESCE((SELECT MAX(release_number) FROM releases WHERE listing_id = l.id), 0) AS latest,
                    r.package_meta_json AS latest_package_meta
             FROM listings l LEFT JOIN releases r ON r.id = (
               SELECT newest.id FROM releases newest
               WHERE newest.listing_id = l.id
               ORDER BY newest.release_number DESC LIMIT 1
             )
             WHERE l.id = ?",
        )
        .bind(listing_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(SkinMarketError::internal)?
        .ok_or_else(|| SkinMarketError::not_found("Appearance listing was not found."))?;
        if row.get::<i64, _>("owner_user_id") != owner_user_id {
            return Err(SkinMarketError::forbidden(
                "Only the listing owner may publish a new appearance release.",
            ));
        }
        if row.get::<String, _>("slug") != draft.slug
            || draft.release_number != row.get::<i64, _>("latest") as u32 + 1
        {
            return Err(SkinMarketError::conflict(
                "invalid_release_target",
                "The listing slug or next release number changed.",
            ));
        }
        if row.get::<String, _>("package_id") != package_meta.package_id {
            return Err(SkinMarketError::conflict(
                "package_id_changed",
                "New releases must preserve the appearance package id.",
            ));
        }
        if let Some(previous_json) = row
            .try_get::<Option<String>, _>("latest_package_meta")
            .map_err(SkinMarketError::internal)?
        {
            let previous: AppearanceMarketPackageMeta =
                parse_json(previous_json, "latest package metadata")?;
            if previous.package_id != package_meta.package_id {
                return Err(SkinMarketError::conflict(
                    "package_id_changed",
                    "New releases must preserve the appearance package id.",
                ));
            }
            let previous_version =
                Version::parse(&previous.package_version).map_err(SkinMarketError::internal)?;
            let next_version =
                Version::parse(&package_meta.package_version).map_err(SkinMarketError::internal)?;
            if next_version <= previous_version {
                return Err(SkinMarketError::conflict(
                    "package_version_not_newer",
                    "New releases must use a higher semantic package version.",
                ));
            }
        }
    } else {
        if draft.release_number != 1 {
            return Err(SkinMarketError::conflict(
                "invalid_release_number",
                "A new appearance listing must start at release 1.",
            ));
        }
        let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM listings WHERE slug = ?")
            .bind(&draft.slug)
            .fetch_one(&mut **transaction)
            .await
            .map_err(SkinMarketError::internal)?;
        if existing > 0 {
            return Err(SkinMarketError::conflict(
                "slug_unavailable",
                "The appearance listing slug is no longer available.",
            ));
        }
        let package_conflict: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM listings WHERE package_id = ?")
                .bind(&package_meta.package_id)
                .fetch_one(&mut **transaction)
                .await
                .map_err(SkinMarketError::internal)?;
        if package_conflict > 0 {
            return Err(SkinMarketError::conflict(
                "package_id_unavailable",
                "The appearance package id already belongs to another Skin listing.",
            ));
        }
    }
    Ok(())
}

async fn record_download(
    state: &SkinMarketState,
    headers: &HeaderMap,
    listing_id: &str,
) -> SkinMarketResult<()> {
    let day = Utc::now().format("%Y-%m-%d").to_string();
    let visitor = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .unwrap_or("unknown");
    let visitor = visitor
        .trim()
        .parse::<std::net::IpAddr>()
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let mut digest = Hmac::<Sha256>::new_from_slice(state.config.download_hash_secret.as_bytes())
        .map_err(SkinMarketError::internal)?;
    digest.update(day.as_bytes());
    digest.update(b"\0");
    digest.update(visitor.as_bytes());
    let visitor_hash = hex::encode(digest.finalize().into_bytes());
    sqlx::query(
        "INSERT OR IGNORE INTO download_days(listing_id, day, visitor_hash, created_at)
         VALUES(?, ?, ?, ?)",
    )
    .bind(listing_id)
    .bind(day)
    .bind(visitor_hash)
    .bind(Utc::now().timestamp())
    .execute(state.database.pool())
    .await
    .map_err(SkinMarketError::internal)?;
    Ok(())
}

async fn insert_audit<'executor, ExecutorType>(
    executor: ExecutorType,
    actor_user_id: i64,
    action: &str,
    target_kind: &str,
    target_id: &str,
    details: Value,
) -> SkinMarketResult<()>
where
    ExecutorType: Executor<'executor, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO audit_log(
           id, actor_user_id, action, target_kind, target_id, details_json, created_at
         ) VALUES(?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_user_id)
    .bind(action)
    .bind(target_kind)
    .bind(target_id)
    .bind(serde_json::to_string(&details).map_err(SkinMarketError::internal)?)
    .bind(Utc::now().timestamp())
    .execute(executor)
    .await
    .map_err(SkinMarketError::internal)?;
    Ok(())
}

fn canonical_review_metadata(
    draft_json: &str,
    package_meta_json: &str,
    manifest_json: &str,
) -> SkinMarketResult<String> {
    let value = serde_json::json!({
        "draft": serde_json::from_str::<Value>(draft_json).map_err(SkinMarketError::internal)?,
        "package": serde_json::from_str::<Value>(package_meta_json).map_err(SkinMarketError::internal)?,
        "manifest": serde_json::from_str::<Value>(manifest_json).map_err(SkinMarketError::internal)?
    });
    serde_json::to_string(&value).map_err(SkinMarketError::internal)
}

fn ensure_public_browse(state: &SkinMarketState) -> SkinMarketResult<()> {
    if state.config.public_browse {
        Ok(())
    } else {
        Err(SkinMarketError::forbidden(
            "Public appearance marketplace browsing is disabled.",
        ))
    }
}

fn ensure_package_content_type(headers: &HeaderMap) -> SkinMarketResult<()> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if matches!(
        content_type,
        Some(APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE | "application/zip")
    ) {
        Ok(())
    } else {
        Err(SkinMarketError::bad_request(
            "unsupported_package_content_type",
            format!("Appearance uploads must use {APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE}."),
        ))
    }
}

fn validate_reason(reason: &str) -> SkinMarketResult<&str> {
    let reason = reason.trim();
    if reason.is_empty() || reason.chars().count() > 500 {
        Err(SkinMarketError::bad_request(
            "invalid_moderation_reason",
            "Moderation reasons must contain between 1 and 500 characters.",
        ))
    } else {
        Ok(reason)
    }
}

fn validate_sha256(value: &str) -> SkinMarketResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(SkinMarketError::not_found(
            "The requested appearance artifact is unavailable.",
        ))
    }
}

fn parse_submission_status(value: &str) -> SkinMarketResult<AppearanceMarketSubmissionStatus> {
    match value {
        "draft" => Ok(AppearanceMarketSubmissionStatus::Draft),
        "submitted" => Ok(AppearanceMarketSubmissionStatus::Submitted),
        "approved" => Ok(AppearanceMarketSubmissionStatus::Approved),
        "rejected" => Ok(AppearanceMarketSubmissionStatus::Rejected),
        "withdrawn" => Ok(AppearanceMarketSubmissionStatus::Withdrawn),
        _ => Err(SkinMarketError::internal(format!(
            "unknown appearance submission status: {value}"
        ))),
    }
}

fn status_string(status: AppearanceMarketSubmissionStatus) -> &'static str {
    match status {
        AppearanceMarketSubmissionStatus::Draft => "draft",
        AppearanceMarketSubmissionStatus::Submitted => "submitted",
        AppearanceMarketSubmissionStatus::Approved => "approved",
        AppearanceMarketSubmissionStatus::Rejected => "rejected",
        AppearanceMarketSubmissionStatus::Withdrawn => "withdrawn",
    }
}

fn encode_cursor(offset: u32) -> String {
    URL_SAFE_NO_PAD.encode(offset.to_string())
}

fn decode_cursor(cursor: Option<&str>) -> SkinMarketResult<u32> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| {
        SkinMarketError::bad_request("invalid_cursor", "The appearance cursor is invalid.")
    })?;
    String::from_utf8(bytes)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|offset| *offset <= 100_000)
        .ok_or_else(|| {
            SkinMarketError::bad_request("invalid_cursor", "The appearance cursor is invalid.")
        })
}

fn encode_submission_cursor(updated_at: i64, submission_id: &str) -> String {
    URL_SAFE_NO_PAD.encode(format!("{updated_at}:{submission_id}"))
}

fn decode_submission_cursor(cursor: Option<&str>) -> SkinMarketResult<Option<(i64, String)>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let decoded = URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|value| {
            let (updated_at, submission_id) = value.split_once(':')?;
            let updated_at = updated_at.parse::<i64>().ok()?;
            (updated_at >= 0 && Uuid::parse_str(submission_id).is_ok())
                .then(|| (updated_at, submission_id.to_string()))
        })
        .ok_or_else(|| {
            SkinMarketError::bad_request(
                "invalid_cursor",
                "The appearance submission cursor is invalid.",
            )
        })?;
    Ok(Some(decoded))
}

fn parse_json<T: serde::de::DeserializeOwned>(value: String, label: &str) -> SkinMarketResult<T> {
    serde_json::from_str(&value)
        .map_err(|error| SkinMarketError::internal(format!("invalid stored {label}: {error}")))
}

fn parse_optional_json<T: serde::de::DeserializeOwned>(
    value: Option<String>,
    label: &str,
) -> SkinMarketResult<Option<T>> {
    value.map(|value| parse_json(value, label)).transpose()
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
}

fn listing_identity_conflict(error: &sqlx::Error) -> SkinMarketError {
    if error.to_string().contains("listings.package_id") {
        SkinMarketError::conflict(
            "package_id_unavailable",
            "The appearance package id already belongs to another Skin listing.",
        )
    } else {
        SkinMarketError::conflict(
            "slug_unavailable",
            "The appearance listing slug is no longer available.",
        )
    }
}
