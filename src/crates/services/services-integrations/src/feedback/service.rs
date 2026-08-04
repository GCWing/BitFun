use super::identity::FeedbackIdentityStore;
use super::message_cache::{MessageCache, MessageCacheData};
use super::state_cache::{FeedbackStateCache, FeedbackStateCacheData};
use super::vault::{FeedbackCredentialStore, FileFeedbackCredentialStore};
use bitfun_product_domains::feedback::{
    validate_content, validate_inbox_page_size, validate_message_page_size,
    AcknowledgeFeedbackRequest, AcknowledgeFeedbackResponse, FeedbackAccessState,
    FeedbackConversationPage, FeedbackError, FeedbackInboxPage, FeedbackMessage,
    FeedbackRecordSummary, FeedbackSender, FeedbackStatus, ListFeedbackRecordsRequest,
    OpenFeedbackConversationRequest, ReplyFeedbackRequest, ReplyFeedbackResponse,
    SubmitFeedbackRequest, SubmitFeedbackResponse,
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use reqwest::{Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const ACCESS_TOKEN_REFRESH_MARGIN_SECONDS: i64 = 600;

const DEBUG_FEEDBACK_API_BASE_URL: &str = "http://api-test.infra-bitfun.com";
const RELEASE_FEEDBACK_API_BASE_URL: &str = "https://api.infra-bitfun.com";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StoredCredentials {
    enroll_key: String,
    #[serde(default, skip_serializing)]
    enroll_idempotency_key: Option<String>,
    #[serde(default, skip_serializing)]
    refresh_idempotency_key: Option<String>,
    #[serde(default, skip_serializing)]
    anonymous_id: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    capabilities: HashMap<String, String>,
    #[serde(default, skip_serializing)]
    pending_create_fingerprint: Option<String>,
    #[serde(default, skip_serializing)]
    pending_create_idempotency_key: Option<String>,
    #[serde(default, skip_serializing)]
    inbox_items: Vec<FeedbackRecordSummary>,
    #[serde(default, skip_serializing)]
    inbox_next_cursor: Option<String>,
    #[serde(default, skip_serializing)]
    inbox_has_more: bool,
    #[serde(default, skip_serializing)]
    read_cursors: HashMap<String, String>,
    #[serde(default, skip_serializing)]
    pending_reply_fingerprints: HashMap<String, String>,
    #[serde(default, skip_serializing)]
    pending_reply_idempotency_keys: HashMap<String, String>,
}

impl StoredCredentials {
    fn cached_state(&self) -> FeedbackStateCacheData {
        FeedbackStateCacheData {
            enroll_idempotency_key: self.enroll_idempotency_key.clone(),
            refresh_idempotency_key: self.refresh_idempotency_key.clone(),
            anonymous_id: self.anonymous_id.clone(),
            pending_create_fingerprint: self.pending_create_fingerprint.clone(),
            pending_create_idempotency_key: self.pending_create_idempotency_key.clone(),
            inbox_items: self.inbox_items.clone(),
            inbox_next_cursor: self.inbox_next_cursor.clone(),
            inbox_has_more: self.inbox_has_more,
            read_cursors: self.read_cursors.clone(),
            pending_reply_fingerprints: self.pending_reply_fingerprints.clone(),
            pending_reply_idempotency_keys: self.pending_reply_idempotency_keys.clone(),
            ..FeedbackStateCacheData::default()
        }
        .with_current_version()
    }

    fn apply_cached_state(&mut self, cached: FeedbackStateCacheData) {
        self.enroll_idempotency_key = cached.enroll_idempotency_key;
        self.refresh_idempotency_key = cached.refresh_idempotency_key;
        self.anonymous_id = cached.anonymous_id;
        self.pending_create_fingerprint = cached.pending_create_fingerprint;
        self.pending_create_idempotency_key = cached.pending_create_idempotency_key;
        self.inbox_items = cached.inbox_items;
        self.inbox_next_cursor = cached.inbox_next_cursor;
        self.inbox_has_more = cached.inbox_has_more;
        self.read_cursors = cached.read_cursors;
        self.pending_reply_fingerprints = cached.pending_reply_fingerprints;
        self.pending_reply_idempotency_keys = cached.pending_reply_idempotency_keys;
    }

    fn has_legacy_cached_state(&self) -> bool {
        self.enroll_idempotency_key.is_some()
            || self.refresh_idempotency_key.is_some()
            || self.anonymous_id.is_some()
            || self.pending_create_fingerprint.is_some()
            || self.pending_create_idempotency_key.is_some()
            || !self.inbox_items.is_empty()
            || self.inbox_next_cursor.is_some()
            || self.inbox_has_more
            || !self.read_cursors.is_empty()
            || !self.pending_reply_fingerprints.is_empty()
            || !self.pending_reply_idempotency_keys.is_empty()
    }
}

#[derive(Debug, Clone)]
struct AccessToken {
    value: String,
    expires_at: DateTime<Utc>,
    scopes: Vec<String>,
}

#[derive(Debug, Default)]
struct RuntimeState {
    loaded: bool,
    stored: StoredCredentials,
    access_token: Option<AccessToken>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    anonymous_id: String,
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct CreateResponse {
    feedback_id: String,
    capability_token: String,
    status: FeedbackStatus,
    inbox_cursor: String,
}

#[derive(Debug, Serialize)]
struct CreateRequestBody<'a> {
    category: bitfun_product_domains::feedback::FeedbackCategory,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id_hash: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_version: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct InboxResponse {
    items: Vec<InboxItem>,
    cursor: String,
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct InboxItem {
    feedback_id: String,
    category: bitfun_product_domains::feedback::FeedbackCategory,
    status: FeedbackStatus,
    has_new_reply: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    feedback_id: String,
    messages: Vec<MessageItem>,
    cursor: String,
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct MessageItem {
    message_id: String,
    sender_type: FeedbackSender,
    content: String,
    content_deleted: bool,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct AcknowledgeRequestBody<'a> {
    read_cursor: &'a str,
}

#[derive(Debug, Deserialize)]
struct AcknowledgeResponseBody {
    feedback_id: String,
    read_cursor: String,
    feedback_status: FeedbackStatus,
}

#[derive(Debug, Serialize)]
struct ReplyRequestBody<'a> {
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ReplyResponseBody {
    message_id: String,
    sender_type: FeedbackSender,
    created_at: String,
    feedback_status: FeedbackStatus,
}

#[derive(Debug, Default, Deserialize)]
struct ServerErrorBody {
    error_code: Option<String>,
    request_id: Option<String>,
}

pub struct FeedbackService {
    client: reqwest::Client,
    base_url: Option<String>,
    client_version: String,
    credential_store: Arc<dyn FeedbackCredentialStore>,
    cache_dir: Option<PathBuf>,
    identity_store: Option<FeedbackIdentityStore>,
    state: Mutex<RuntimeState>,
}

impl FeedbackService {
    pub fn from_environment(data_dir: PathBuf, client_version: impl Into<String>) -> Self {
        let cache_dir = data_dir.join("feedback-cache");
        Self::from_environment_with_credential_store(
            client_version,
            Arc::new(FileFeedbackCredentialStore::new(data_dir)),
        )
        .with_cache_dir(cache_dir)
    }

    pub fn from_environment_with_credential_store(
        client_version: impl Into<String>,
        credential_store: Arc<dyn FeedbackCredentialStore>,
    ) -> Self {
        Self::new(
            configured_base_url(),
            client_version.into(),
            credential_store,
            REQUEST_TIMEOUT,
        )
    }

    fn new(
        base_url: Option<String>,
        client_version: String,
        credential_store: Arc<dyn FeedbackCredentialStore>,
        timeout: Duration,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("feedback HTTP client must initialize"),
            base_url,
            client_version,
            credential_store,
            cache_dir: None,
            identity_store: None,
            state: Mutex::new(RuntimeState::default()),
        }
    }

    pub fn with_message_cache_dir(mut self, message_cache_dir: PathBuf) -> Self {
        self.cache_dir = Some(message_cache_dir);
        self
    }

    pub fn with_cache_dir(mut self, cache_dir: PathBuf) -> Self {
        self.cache_dir = Some(cache_dir);
        self
    }

    pub fn with_identity_path(mut self, identity_path: PathBuf) -> Self {
        self.identity_store = Some(FeedbackIdentityStore::new(identity_path));
        self
    }

    pub async fn submit_feedback(
        &self,
        request: SubmitFeedbackRequest,
    ) -> Result<SubmitFeedbackResponse, FeedbackError> {
        let request = normalize_request(request);
        validate_content(&request.content)?;
        let idempotency_key = self.create_idempotency_key(&request).await?;
        let body = CreateRequestBody {
            category: request.category,
            content: request.content.trim(),
            trace_id: request.trace_id.as_deref(),
            session_id_hash: request.session_id_hash.as_deref(),
            client_version: valid_optional_value(&self.client_version, 20),
        };
        let response = self
            .send_authenticated("feedback:write", |token| {
                self.client
                    .post(self.url("/support/v1/feedback")?)
                    .bearer_auth(token)
                    .header("X-Request-ID", Uuid::new_v4().to_string())
                    .header("Idempotency-Key", &idempotency_key)
                    .json(&body)
                    .build()
                    .map_err(network_error)
            })
            .await?;
        let created: CreateResponse = decode_success(response, StatusCode::CREATED).await?;
        if created.status != FeedbackStatus::Submitted {
            return Err(FeedbackError::new(
                "RESPONSE_INVALID",
                "Feedback service returned an invalid initial status",
                true,
            ));
        }

        let mut state = self.state.lock().await;
        self.ensure_loaded(&mut state).await?;
        let mut next = state.stored.clone();
        next.capabilities
            .insert(created.feedback_id.clone(), created.capability_token);
        next.pending_create_fingerprint = None;
        next.pending_create_idempotency_key = None;
        if let Err(error) = self.persist_credentials(&next).await {
            return Err(FeedbackError {
                code: "CAPABILITY_SAVE_FAILED".to_string(),
                message: "Feedback access could not be saved securely".to_string(),
                retryable: true,
                request_id: error.request_id,
                retry_after_seconds: None,
            });
        }
        state.stored = next;
        self.persist_cached_state_best_effort(&state.stored).await;
        Ok(SubmitFeedbackResponse {
            feedback_id: created.feedback_id,
            status: created.status,
            inbox_cursor: created.inbox_cursor,
        })
    }

    pub async fn access_state(&self) -> Result<FeedbackAccessState, FeedbackError> {
        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        let can_reuse_access = state.stored.refresh_token.is_some();
        let has_history =
            !state.stored.capabilities.is_empty() || !state.stored.inbox_items.is_empty();
        Ok(FeedbackAccessState {
            has_history,
            can_reuse_access,
            cached_inbox: cached_inbox(&state.stored, can_reuse_access),
        })
    }

    pub async fn list_feedback(
        &self,
        request: ListFeedbackRecordsRequest,
    ) -> Result<FeedbackInboxPage, FeedbackError> {
        validate_inbox_page_size(request.page_size)?;
        let cursor = request.cursor.clone();
        let page_size = request.page_size.to_string();
        let response = self
            .send_authenticated_existing("feedback:read", |token| {
                let mut request = self
                    .client
                    .get(self.url("/support/v1/feedback/inbox")?)
                    .bearer_auth(token)
                    .header("X-Request-ID", Uuid::new_v4().to_string())
                    .query(&[("limit", &page_size)]);
                if let Some(cursor) = cursor.as_deref() {
                    request = request.query(&[("cursor", cursor)]);
                }
                request.build().map_err(network_error)
            })
            .await?;
        let received: InboxResponse = decode_success(response, StatusCode::OK).await?;

        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        let can_reuse_access = state.stored.refresh_token.is_some();
        let items = received
            .items
            .into_iter()
            .map(|item| FeedbackRecordSummary {
                can_open: can_reuse_access
                    && state.stored.capabilities.contains_key(&item.feedback_id),
                feedback_id: item.feedback_id,
                category: item.category,
                status: item.status,
                has_new_reply: item.has_new_reply,
                created_at: item.created_at,
                updated_at: item.updated_at,
            })
            .collect::<Vec<_>>();
        let next_cursor = (!received.cursor.is_empty()).then_some(received.cursor);

        let mut next = state.stored.clone();
        if request.cursor.is_none() {
            next.inbox_items = items.clone();
        } else {
            for item in &items {
                if let Some(existing) = next
                    .inbox_items
                    .iter_mut()
                    .find(|existing| existing.feedback_id == item.feedback_id)
                {
                    *existing = item.clone();
                } else {
                    next.inbox_items.push(item.clone());
                }
            }
        }
        next.inbox_next_cursor = next_cursor.clone();
        next.inbox_has_more = received.has_more;
        state.stored = next;
        self.persist_cached_state_best_effort(&state.stored).await;

        Ok(FeedbackInboxPage {
            items,
            next_cursor,
            has_more: received.has_more,
        })
    }

    pub async fn open_conversation(
        &self,
        request: OpenFeedbackConversationRequest,
    ) -> Result<FeedbackConversationPage, FeedbackError> {
        validate_message_page_size(request.page_size)?;
        validate_feedback_id(&request.feedback_id)?;
        let cache = self.message_cache().await?;
        let loaded = cache.load(&request.feedback_id).await;
        let mut data = match loaded {
            Ok(Some(data)) => data,
            Ok(None) => MessageCacheData::empty(&request.feedback_id),
            Err(_) => {
                cache.remove(&request.feedback_id).await.map_err(|_| {
                    credential_error(
                        "CACHE_RESET_FAILED",
                        "Feedback message cache could not be reset",
                    )
                })?;
                MessageCacheData::empty(&request.feedback_id)
            }
        };

        let mut sync_error = None;
        if request.cursor.is_none() {
            if let Err(error) = self
                .synchronize_messages(&request.feedback_id, &cache, &mut data)
                .await
            {
                if is_terminal_capability_error(&error.code) {
                    self.invalidate_conversation_access(&request.feedback_id, &cache)
                        .await;
                    return Err(error);
                }
                sync_error = Some(error);
            }
        }

        if !data.sync_complete {
            return Ok(FeedbackConversationPage {
                messages: Vec::new(),
                next_cursor: None,
                has_more: false,
                sync_error,
            });
        }
        if request.cursor.is_none() && sync_error.is_none() {
            self.reconcile_user_message_unread(&request.feedback_id, &data, true)
                .await;
        }
        page_from_cache(
            &data,
            request.cursor.as_deref(),
            request.page_size,
            sync_error,
        )
    }

    pub async fn acknowledge_feedback(
        &self,
        request: AcknowledgeFeedbackRequest,
    ) -> Result<AcknowledgeFeedbackResponse, FeedbackError> {
        validate_feedback_id(&request.feedback_id)?;
        validate_timestamp(&request.last_visible_at)?;
        let capability = self.capability_for(&request.feedback_id).await?;
        let feedback_id = request.feedback_id.clone();
        let body = AcknowledgeRequestBody {
            read_cursor: &request.last_visible_at,
        };
        let response = match self
            .send_authenticated_existing("feedback:read", |token| {
                self.client
                    .post(self.url(&format!("/support/v1/feedback/{feedback_id}/ack"))?)
                    .bearer_auth(token)
                    .header("X-Feedback-Capability", &capability)
                    .header("X-Request-ID", Uuid::new_v4().to_string())
                    .json(&body)
                    .build()
                    .map_err(network_error)
            })
            .await
        {
            Ok(response) => response,
            Err(error) => {
                if is_terminal_capability_error(&error.code) {
                    let cache = self.message_cache().await?;
                    self.invalidate_conversation_access(&feedback_id, &cache)
                        .await;
                }
                return Err(error);
            }
        };
        let acknowledged: AcknowledgeResponseBody =
            match decode_success(response, StatusCode::OK).await {
                Ok(value) => value,
                Err(error) => {
                    if is_terminal_capability_error(&error.code) {
                        let cache = self.message_cache().await?;
                        self.invalidate_conversation_access(&feedback_id, &cache)
                            .await;
                    }
                    return Err(error);
                }
            };
        if acknowledged.feedback_id != feedback_id {
            return Err(FeedbackError::new(
                "RESPONSE_INVALID",
                "Feedback service returned a mismatched conversation",
                true,
            ));
        }
        validate_timestamp(&acknowledged.read_cursor).map_err(|_| {
            FeedbackError::new(
                "RESPONSE_INVALID",
                "Feedback service returned an invalid read cursor",
                true,
            )
        })?;

        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        let mut next = state.stored.clone();
        next.read_cursors
            .insert(feedback_id.clone(), acknowledged.read_cursor.clone());
        if let Some(record) = next
            .inbox_items
            .iter_mut()
            .find(|record| record.feedback_id == feedback_id)
        {
            record.status = acknowledged.feedback_status;
            record.has_new_reply = false;
        }
        state.stored = next;
        self.persist_cached_state_best_effort(&state.stored).await;

        Ok(AcknowledgeFeedbackResponse {
            read_through: acknowledged.read_cursor,
            feedback_status: acknowledged.feedback_status,
        })
    }

    pub async fn reply_feedback(
        &self,
        request: ReplyFeedbackRequest,
    ) -> Result<ReplyFeedbackResponse, FeedbackError> {
        validate_feedback_id(&request.feedback_id)?;
        validate_content(&request.content)?;
        let feedback_id = request.feedback_id.clone();
        self.ensure_reply_allowed(&feedback_id).await?;
        let content = request.content.trim().to_string();
        let idempotency_key = self.reply_idempotency_key(&feedback_id, &content).await?;
        let capability = self.capability_for(&feedback_id).await?;
        let body = ReplyRequestBody { content: &content };
        let response = match self
            .send_authenticated_existing("feedback:write", |token| {
                self.client
                    .post(self.url(&format!("/support/v1/feedback/{feedback_id}/messages"))?)
                    .bearer_auth(token)
                    .header("X-Feedback-Capability", &capability)
                    .header("X-Request-ID", Uuid::new_v4().to_string())
                    .header("Idempotency-Key", &idempotency_key)
                    .json(&body)
                    .build()
                    .map_err(network_error)
            })
            .await
        {
            Ok(response) => response,
            Err(error) => {
                if is_terminal_capability_error(&error.code) {
                    let cache = self.message_cache().await?;
                    self.invalidate_conversation_access(&feedback_id, &cache)
                        .await;
                }
                return Err(error);
            }
        };
        let replied: ReplyResponseBody = match decode_success(response, StatusCode::CREATED).await {
            Ok(value) => value,
            Err(error) => {
                if is_terminal_capability_error(&error.code) {
                    let cache = self.message_cache().await?;
                    self.invalidate_conversation_access(&feedback_id, &cache)
                        .await;
                }
                return Err(error);
            }
        };
        // The message fields prove that the write committed. Treat the returned
        // status as server-authoritative so a lagging status transition cannot
        // turn a successful idempotent write into a retryable client failure.
        if replied.sender_type != FeedbackSender::User
            || Uuid::parse_str(&replied.message_id).is_err()
            || validate_timestamp(&replied.created_at).is_err()
        {
            return Err(FeedbackError::new(
                "RESPONSE_INVALID",
                "Feedback service returned an invalid reply",
                true,
            ));
        }

        let message = FeedbackMessage {
            message_id: replied.message_id,
            sender: replied.sender_type,
            content,
            content_deleted: false,
            created_at: replied.created_at,
        };
        let mut cached_after_reply = None;
        if let Ok(cache) = self.message_cache().await {
            let cached = match cache.load(&feedback_id).await {
                Ok(cached) => cached,
                Err(_) => {
                    let _ = cache.remove(&feedback_id).await;
                    None
                }
            };
            if let Some(mut cached) = cached {
                if !cached
                    .messages
                    .iter()
                    .any(|existing| existing.message_id == message.message_id)
                {
                    cached.messages.push(message.clone());
                    cached.messages.sort_by(|left, right| {
                        left.created_at
                            .cmp(&right.created_at)
                            .then_with(|| left.message_id.cmp(&right.message_id))
                    });
                    if cache.store(&cached).await.is_err() {
                        log::warn!(
                            "Failed to save feedback message cache after a successful reply"
                        );
                    }
                }
                cached_after_reply = Some(cached);
            }
        }

        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        let mut next = state.stored.clone();
        next.pending_reply_fingerprints.remove(&feedback_id);
        next.pending_reply_idempotency_keys.remove(&feedback_id);
        if let Some(record) = next
            .inbox_items
            .iter_mut()
            .find(|record| record.feedback_id == feedback_id)
        {
            record.status = replied.feedback_status;
            record.updated_at = message.created_at.clone();
            record.has_new_reply = false;
        }
        state.stored = next;
        self.persist_cached_state_best_effort(&state.stored).await;
        drop(state);

        let feedback_status = match cached_after_reply.as_ref() {
            Some(cached) => self
                .reconcile_user_message_unread(&feedback_id, cached, false)
                .await
                .unwrap_or(replied.feedback_status),
            None => replied.feedback_status,
        };

        Ok(ReplyFeedbackResponse {
            message,
            feedback_status,
        })
    }

    async fn reconcile_user_message_unread(
        &self,
        feedback_id: &str,
        data: &MessageCacheData,
        require_reported_unread: bool,
    ) -> Option<FeedbackStatus> {
        let mut state = self.state.lock().await;
        if self.ensure_existing_loaded(&mut state).await.is_err() {
            return None;
        }
        if require_reported_unread
            && !state
                .stored
                .inbox_items
                .iter()
                .any(|record| record.feedback_id == feedback_id && record.has_new_reply)
        {
            return None;
        }
        let cursor =
            user_message_reconciliation_cursor(data, state.stored.read_cursors.get(feedback_id))?;
        drop(state);

        match tokio::time::timeout(
            Duration::from_secs(3),
            self.acknowledge_feedback(AcknowledgeFeedbackRequest {
                feedback_id: feedback_id.to_string(),
                last_visible_at: cursor,
            }),
        )
        .await
        {
            Ok(Ok(acknowledged)) => Some(acknowledged.feedback_status),
            Ok(Err(error)) => {
                log::warn!(
                    "Failed to reconcile feedback unread state: code={}, request_id={}",
                    error.code,
                    error.request_id.as_deref().unwrap_or("unavailable")
                );
                None
            }
            Err(_) => {
                log::warn!("Timed out while reconciling feedback unread state");
                None
            }
        }
    }

    async fn synchronize_messages(
        &self,
        feedback_id: &str,
        cache: &MessageCache,
        data: &mut MessageCacheData,
    ) -> Result<(), FeedbackError> {
        let capability = self.capability_for(feedback_id).await?;
        let mut reset_after_invalid_cursor = data.sync_cursor.is_some();
        loop {
            let cursor = data.sync_cursor.clone();
            let response = self
                .send_authenticated_existing("feedback:read", |token| {
                    let mut request = self
                        .client
                        .get(self.url(&format!("/support/v1/feedback/{feedback_id}/messages"))?)
                        .bearer_auth(token)
                        .header("X-Feedback-Capability", &capability)
                        .header("X-Request-ID", Uuid::new_v4().to_string())
                        .query(&[("limit", "50")]);
                    if let Some(cursor) = cursor.as_deref() {
                        request = request.query(&[("cursor", cursor)]);
                    }
                    request.build().map_err(network_error)
                })
                .await?;
            let received: MessagesResponse = match decode_success(response, StatusCode::OK).await {
                Ok(value) => value,
                Err(error) if error.code == "CURSOR_INVALID" && reset_after_invalid_cursor => {
                    *data = MessageCacheData::empty(feedback_id);
                    cache.store(data).await.map_err(|_| {
                        credential_error(
                            "CACHE_SAVE_FAILED",
                            "Feedback message cache could not be saved",
                        )
                    })?;
                    reset_after_invalid_cursor = false;
                    continue;
                }
                Err(error) => return Err(error),
            };
            if received.feedback_id != feedback_id {
                return Err(FeedbackError::new(
                    "RESPONSE_INVALID",
                    "Feedback service returned a mismatched conversation",
                    true,
                ));
            }
            let previous_cursor = data.sync_cursor.clone();
            merge_messages(data, received.messages)?;
            data.sync_cursor = (!received.cursor.is_empty()).then_some(received.cursor);
            if received.has_more && data.sync_cursor == previous_cursor {
                return Err(FeedbackError::new(
                    "RESPONSE_INVALID",
                    "Feedback service did not advance the message cursor",
                    true,
                ));
            }
            if !received.has_more {
                data.sync_complete = true;
            }
            cache.store(data).await.map_err(|_| {
                credential_error(
                    "CACHE_SAVE_FAILED",
                    "Feedback message cache could not be saved",
                )
            })?;
            if !received.has_more {
                return Ok(());
            }
        }
    }

    async fn message_cache(&self) -> Result<MessageCache, FeedbackError> {
        let directory = self.cache_dir.clone().ok_or_else(|| {
            FeedbackError::new(
                "CACHE_UNAVAILABLE",
                "Feedback message cache is unavailable",
                false,
            )
        })?;
        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        let key = cache_encryption_key(&state.stored.enroll_key).ok_or_else(|| {
            FeedbackError::new(
                "CACHE_UNAVAILABLE",
                "Feedback message cache key is unavailable",
                false,
            )
        })?;
        Ok(MessageCache::new(directory, key))
    }

    async fn capability_for(&self, feedback_id: &str) -> Result<String, FeedbackError> {
        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        state
            .stored
            .capabilities
            .get(feedback_id)
            .cloned()
            .ok_or_else(|| {
                FeedbackError::new(
                    "CAPABILITY_UNAVAILABLE",
                    "Feedback conversation access is unavailable",
                    false,
                )
            })
    }

    async fn ensure_reply_allowed(&self, feedback_id: &str) -> Result<(), FeedbackError> {
        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        if state.stored.inbox_items.iter().any(|record| {
            record.feedback_id == feedback_id && record.status == FeedbackStatus::Resolved
        }) {
            return Err(FeedbackError::new(
                "FEEDBACK_ALREADY_RESOLVED",
                "Resolved feedback cannot receive replies",
                false,
            ));
        }
        Ok(())
    }

    async fn invalidate_conversation_access(&self, feedback_id: &str, cache: &MessageCache) {
        let _ = cache.remove(feedback_id).await;
        let mut state = self.state.lock().await;
        if self.ensure_existing_loaded(&mut state).await.is_err() {
            return;
        }
        let mut next = state.stored.clone();
        next.capabilities.remove(feedback_id);
        next.read_cursors.remove(feedback_id);
        next.pending_reply_fingerprints.remove(feedback_id);
        next.pending_reply_idempotency_keys.remove(feedback_id);
        if let Some(record) = next
            .inbox_items
            .iter_mut()
            .find(|record| record.feedback_id == feedback_id)
        {
            record.can_open = false;
        }
        if self.persist_credentials(&next).await.is_ok() {
            state.stored = next;
            self.persist_cached_state_best_effort(&state.stored).await;
        }
    }

    fn url(&self, path: &str) -> Result<String, FeedbackError> {
        self.base_url
            .as_ref()
            .map(|base| format!("{base}{path}"))
            .ok_or_else(|| {
                FeedbackError::new(
                    "FEEDBACK_NOT_CONFIGURED",
                    "Feedback API base URL is not configured",
                    false,
                )
            })
    }

    async fn create_idempotency_key(
        &self,
        request: &SubmitFeedbackRequest,
    ) -> Result<String, FeedbackError> {
        let fingerprint = request_fingerprint(request)?;
        let mut state = self.state.lock().await;
        self.ensure_loaded(&mut state).await?;
        if state.stored.pending_create_fingerprint.as_deref() == Some(&fingerprint) {
            if let Some(key) = state.stored.pending_create_idempotency_key.as_ref() {
                return Ok(key.clone());
            }
        }

        let key = Uuid::new_v4().to_string();
        let mut next = state.stored.clone();
        next.pending_create_fingerprint = Some(fingerprint);
        next.pending_create_idempotency_key = Some(key.clone());
        self.persist_cached_state(&next).await?;
        state.stored = next;
        Ok(key)
    }

    async fn reply_idempotency_key(
        &self,
        feedback_id: &str,
        content: &str,
    ) -> Result<String, FeedbackError> {
        let fingerprint = format!("{:x}", Sha256::digest(content.as_bytes()));
        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        if state
            .stored
            .pending_reply_fingerprints
            .get(feedback_id)
            .map(String::as_str)
            == Some(fingerprint.as_str())
        {
            if let Some(key) = state.stored.pending_reply_idempotency_keys.get(feedback_id) {
                return Ok(key.clone());
            }
        }

        let key = Uuid::new_v4().to_string();
        let mut next = state.stored.clone();
        next.pending_reply_fingerprints
            .insert(feedback_id.to_string(), fingerprint);
        next.pending_reply_idempotency_keys
            .insert(feedback_id.to_string(), key.clone());
        self.persist_cached_state(&next).await?;
        state.stored = next;
        Ok(key)
    }

    async fn send_authenticated<F>(
        &self,
        scope: &str,
        build_request: F,
    ) -> Result<Response, FeedbackError>
    where
        F: Fn(&str) -> Result<reqwest::Request, FeedbackError>,
    {
        let token = self.access_token(scope, false).await?;
        let response = self
            .client
            .execute(build_request(&token)?)
            .await
            .map_err(network_error)?;
        if response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }

        let token = self.access_token(scope, true).await?;
        self.client
            .execute(build_request(&token)?)
            .await
            .map_err(network_error)
    }

    async fn send_authenticated_existing<F>(
        &self,
        scope: &str,
        build_request: F,
    ) -> Result<Response, FeedbackError>
    where
        F: Fn(&str) -> Result<reqwest::Request, FeedbackError>,
    {
        let token = self.existing_access_token(scope, false).await?;
        let response = self
            .client
            .execute(build_request(&token)?)
            .await
            .map_err(network_error)?;
        if response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }

        let token = self.existing_access_token(scope, true).await?;
        self.client
            .execute(build_request(&token)?)
            .await
            .map_err(network_error)
    }

    async fn existing_access_token(
        &self,
        scope: &str,
        force_refresh: bool,
    ) -> Result<String, FeedbackError> {
        let mut state = self.state.lock().await;
        self.ensure_existing_loaded(&mut state).await?;
        if !force_refresh {
            if let Some(token) = state.access_token.as_ref() {
                if token.expires_at
                    > Utc::now() + ChronoDuration::seconds(ACCESS_TOKEN_REFRESH_MARGIN_SECONDS)
                    && token.scopes.iter().any(|item| item == scope)
                {
                    return Ok(token.value.clone());
                }
            }
        }
        if state.stored.refresh_token.is_none() {
            return Err(FeedbackError::new(
                "FEEDBACK_ACCESS_UNAVAILABLE",
                "Saved feedback access is unavailable",
                false,
            ));
        }
        let token = match self.refresh(&mut state).await {
            Ok(token) => token,
            Err(error) if refresh_requires_enroll(&error.code) => {
                let mut next = state.stored.clone();
                next.anonymous_id = None;
                next.refresh_token = None;
                next.refresh_idempotency_key = None;
                self.persist_credentials(&next).await?;
                state.stored = next;
                self.persist_cached_state_best_effort(&state.stored).await;
                state.access_token = None;
                return Err(FeedbackError::new(
                    "FEEDBACK_ACCESS_EXPIRED",
                    "Saved feedback access has expired",
                    false,
                ));
            }
            Err(error) => return Err(error),
        };
        if !token.scopes.iter().any(|item| item == scope) {
            return Err(FeedbackError::new(
                "SCOPE_INSUFFICIENT",
                "The feedback token does not include the required scope",
                false,
            ));
        }
        let value = token.value.clone();
        state.access_token = Some(token);
        Ok(value)
    }

    async fn access_token(
        &self,
        scope: &str,
        force_refresh: bool,
    ) -> Result<String, FeedbackError> {
        let mut state = self.state.lock().await;
        self.ensure_loaded(&mut state).await?;
        if !force_refresh {
            if let Some(token) = state.access_token.as_ref() {
                if token.expires_at
                    > Utc::now() + ChronoDuration::seconds(ACCESS_TOKEN_REFRESH_MARGIN_SECONDS)
                    && token.scopes.iter().any(|item| item == scope)
                {
                    return Ok(token.value.clone());
                }
            }
        }

        let token = if state.stored.refresh_token.is_some() {
            match self.refresh(&mut state).await {
                Ok(token) => token,
                Err(error) if refresh_requires_enroll(&error.code) => {
                    let mut next = state.stored.clone();
                    next.anonymous_id = None;
                    next.refresh_token = None;
                    next.refresh_idempotency_key = None;
                    next.capabilities.clear();
                    self.persist_credentials(&next).await?;
                    state.stored = next;
                    self.persist_cached_state_best_effort(&state.stored).await;
                    state.access_token = None;
                    self.enroll(&mut state).await?
                }
                Err(error) => return Err(error),
            }
        } else {
            self.enroll(&mut state).await?
        };
        if !token.scopes.iter().any(|item| item == scope) {
            return Err(FeedbackError::new(
                "SCOPE_INSUFFICIENT",
                "The feedback token does not include the required scope",
                false,
            ));
        }
        let value = token.value.clone();
        state.access_token = Some(token);
        Ok(value)
    }

    async fn enroll(&self, state: &mut RuntimeState) -> Result<AccessToken, FeedbackError> {
        let idempotency_key = match state.stored.enroll_idempotency_key.as_ref() {
            Some(key) => key.clone(),
            None => {
                let mut next = state.stored.clone();
                let key = Uuid::new_v4().to_string();
                next.enroll_idempotency_key = Some(key.clone());
                self.persist_cached_state(&next).await?;
                state.stored = next;
                key
            }
        };
        let response = self
            .client
            .post(self.url("/auth/v1/anonymous/enroll")?)
            .header("X-Request-ID", Uuid::new_v4().to_string())
            .header("Idempotency-Key", idempotency_key)
            .json(&serde_json::json!({ "key": state.stored.enroll_key }))
            .send()
            .await
            .map_err(network_error)?;
        let token: TokenResponse = decode_success(response, StatusCode::CREATED).await?;
        self.commit_token(state, token, true).await
    }

    async fn refresh(&self, state: &mut RuntimeState) -> Result<AccessToken, FeedbackError> {
        let refresh_token = state.stored.refresh_token.clone().ok_or_else(|| {
            FeedbackError::new(
                "REFRESH_TOKEN_MISSING",
                "Feedback refresh token is unavailable",
                false,
            )
        })?;
        let idempotency_key = match state.stored.refresh_idempotency_key.as_ref() {
            Some(key) => key.clone(),
            None => {
                let mut next = state.stored.clone();
                let key = Uuid::new_v4().to_string();
                next.refresh_idempotency_key = Some(key.clone());
                self.persist_cached_state(&next).await?;
                state.stored = next;
                key
            }
        };
        let response = self
            .client
            .post(self.url("/auth/v1/anonymous/token")?)
            .header("X-Request-ID", Uuid::new_v4().to_string())
            .header("Idempotency-Key", idempotency_key)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(network_error)?;
        let token: TokenResponse = decode_success(response, StatusCode::OK).await?;
        self.commit_token(state, token, false).await
    }

    async fn commit_token(
        &self,
        state: &mut RuntimeState,
        response: TokenResponse,
        enrolled: bool,
    ) -> Result<AccessToken, FeedbackError> {
        let token = AccessToken {
            value: response.access_token,
            expires_at: Utc::now() + ChronoDuration::seconds(response.expires_in.max(0)),
            scopes: parse_scopes(&response.scope),
        };
        let anonymous_id = response.anonymous_id;
        let mut next = state.stored.clone();
        next.anonymous_id = Some(anonymous_id.clone());
        next.refresh_token = Some(response.refresh_token);
        next.refresh_idempotency_key = None;
        if enrolled {
            next.enroll_idempotency_key = None;
        }
        self.persist_credentials(&next).await?;
        if let Some(identity_store) = &self.identity_store {
            if identity_store.store(&anonymous_id).await.is_err() {
                log::warn!("Failed to save the local feedback identity copy");
            }
        }
        state.stored = next;
        self.persist_cached_state_best_effort(&state.stored).await;
        Ok(token)
    }

    async fn ensure_loaded(&self, state: &mut RuntimeState) -> Result<(), FeedbackError> {
        self.ensure_existing_loaded(state).await?;
        if state.stored.enroll_key.is_empty() {
            state.stored.enroll_key = Uuid::new_v4().to_string();
            self.persist_credentials(&state.stored).await?;
        }
        Ok(())
    }

    async fn ensure_existing_loaded(&self, state: &mut RuntimeState) -> Result<(), FeedbackError> {
        if state.loaded {
            return Ok(());
        }
        let stored = self.credential_store.load().await.map_err(|_| {
            credential_error(
                "CREDENTIAL_LOAD_FAILED",
                "Feedback access could not be loaded",
            )
        })?;
        state.stored = match stored {
            Some(value) => serde_json::from_str(&value).map_err(|_| {
                credential_error(
                    "CREDENTIALS_INVALID",
                    "Saved feedback access data is invalid",
                )
            })?,
            None => StoredCredentials::default(),
        };
        let had_legacy_cached_state = state.stored.has_legacy_cached_state();
        if let Some(cache) = self.state_cache(&state.stored) {
            match cache.load().await {
                Ok(Some(cached)) => state.stored.apply_cached_state(cached),
                Ok(None) => {
                    if had_legacy_cached_state {
                        self.store_cached_state_best_effort(&cache, &state.stored)
                            .await;
                    }
                }
                Err(_) => {
                    let _ = cache.remove().await;
                    log::warn!("Feedback state cache was invalid and has been reset");
                    if had_legacy_cached_state {
                        self.store_cached_state_best_effort(&cache, &state.stored)
                            .await;
                    }
                }
            }
        }
        if had_legacy_cached_state {
            if let Err(error) = self.persist_credentials(&state.stored).await {
                log::warn!(
                    "Failed to migrate feedback credentials away from legacy cache fields: code={}",
                    error.code
                );
            }
        }
        if let (Some(identity_store), Some(anonymous_id)) =
            (&self.identity_store, state.stored.anonymous_id.as_deref())
        {
            if identity_store.store(anonymous_id).await.is_err() {
                log::warn!("Failed to save the local feedback identity copy");
            }
        }
        state.loaded = true;
        Ok(())
    }

    async fn persist_credentials(&self, stored: &StoredCredentials) -> Result<(), FeedbackError> {
        let value = serde_json::to_string(stored).map_err(|_| {
            credential_error(
                "CREDENTIAL_SAVE_FAILED",
                "Feedback access could not be encoded",
            )
        })?;
        self.credential_store.store(&value).await.map_err(|_| {
            credential_error(
                "CREDENTIAL_SAVE_FAILED",
                "Feedback access could not be saved securely",
            )
        })
    }

    async fn persist_cached_state(&self, stored: &StoredCredentials) -> Result<(), FeedbackError> {
        let cache = self.state_cache(stored).ok_or_else(|| {
            credential_error("CACHE_UNAVAILABLE", "Feedback state cache is unavailable")
        })?;
        cache.store(&stored.cached_state()).await.map_err(|_| {
            credential_error(
                "CACHE_SAVE_FAILED",
                "Feedback state cache could not be saved",
            )
        })
    }

    async fn persist_cached_state_best_effort(&self, stored: &StoredCredentials) {
        let Some(cache) = self.state_cache(stored) else {
            return;
        };
        self.store_cached_state_best_effort(&cache, stored).await;
    }

    async fn store_cached_state_best_effort(
        &self,
        cache: &FeedbackStateCache,
        stored: &StoredCredentials,
    ) {
        if cache.store(&stored.cached_state()).await.is_err() {
            log::warn!("Failed to save feedback state cache; server data remains authoritative");
        }
    }

    fn state_cache(&self, stored: &StoredCredentials) -> Option<FeedbackStateCache> {
        let directory = self.cache_dir.clone()?;
        let key = cache_encryption_key(&stored.enroll_key)?;
        Some(FeedbackStateCache::new(directory, key))
    }
}

fn cached_inbox(stored: &StoredCredentials, can_reuse_access: bool) -> FeedbackInboxPage {
    FeedbackInboxPage {
        items: stored
            .inbox_items
            .iter()
            .cloned()
            .map(|mut item| {
                item.can_open =
                    can_reuse_access && stored.capabilities.contains_key(&item.feedback_id);
                item
            })
            .collect(),
        next_cursor: stored.inbox_next_cursor.clone(),
        has_more: stored.inbox_has_more,
    }
}

fn merge_messages(
    data: &mut MessageCacheData,
    received: Vec<MessageItem>,
) -> Result<(), FeedbackError> {
    let mut known = data
        .messages
        .iter()
        .map(|message| message.message_id.clone())
        .collect::<HashSet<_>>();
    for message in received {
        validate_content(&message.content).map_err(|_| {
            FeedbackError::new(
                "RESPONSE_INVALID",
                "Feedback service returned invalid message content",
                true,
            )
        })?;
        validate_timestamp(&message.created_at).map_err(|_| {
            FeedbackError::new(
                "RESPONSE_INVALID",
                "Feedback service returned an invalid message timestamp",
                true,
            )
        })?;
        if known.insert(message.message_id.clone()) {
            data.messages.push(FeedbackMessage {
                message_id: message.message_id,
                sender: message.sender_type,
                content: message.content,
                content_deleted: message.content_deleted,
                created_at: message.created_at,
            });
        }
    }
    data.messages.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.message_id.cmp(&right.message_id))
    });
    Ok(())
}

fn page_from_cache(
    data: &MessageCacheData,
    cursor: Option<&str>,
    page_size: u16,
    sync_error: Option<FeedbackError>,
) -> Result<FeedbackConversationPage, FeedbackError> {
    let end = match cursor {
        Some(cursor) => cursor
            .strip_prefix("cache:")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value <= data.messages.len())
            .ok_or_else(|| {
                FeedbackError::validation(
                    "CACHE_CURSOR_INVALID",
                    "Feedback message cache cursor is invalid",
                )
            })?,
        None => data.messages.len(),
    };
    let start = end.saturating_sub(usize::from(page_size));
    Ok(FeedbackConversationPage {
        messages: data.messages[start..end].to_vec(),
        next_cursor: (start > 0).then(|| format!("cache:{start}")),
        has_more: start > 0,
        sync_error,
    })
}

fn cached_unread_admin_reply(
    data: &MessageCacheData,
    read_cursor: Option<&String>,
) -> Option<bool> {
    if !data.sync_complete {
        return None;
    }
    let latest_admin = data
        .messages
        .iter()
        .filter(|message| message.sender == FeedbackSender::Admin)
        .filter_map(|message| {
            DateTime::parse_from_rfc3339(&message.created_at)
                .ok()
                .map(|timestamp| (timestamp, message.created_at.as_str()))
        })
        .max_by_key(|(timestamp, _)| *timestamp);
    let Some((latest_admin, _)) = latest_admin else {
        return Some(false);
    };
    let Some(read_cursor) = read_cursor else {
        return Some(true);
    };
    let Ok(read_cursor) = DateTime::parse_from_rfc3339(read_cursor) else {
        return None;
    };
    Some(latest_admin > read_cursor)
}

fn user_message_reconciliation_cursor(
    data: &MessageCacheData,
    read_cursor: Option<&String>,
) -> Option<String> {
    if cached_unread_admin_reply(data, read_cursor)? {
        return None;
    }
    data.messages
        .iter()
        .filter_map(|message| {
            DateTime::parse_from_rfc3339(&message.created_at)
                .ok()
                .map(|timestamp| (timestamp, message))
        })
        .max_by_key(|(timestamp, _)| *timestamp)
        .filter(|(_, message)| message.sender == FeedbackSender::User)
        .map(|(_, message)| message.created_at.clone())
}

fn cache_encryption_key(enroll_key: &str) -> Option<[u8; 32]> {
    if enroll_key.is_empty() {
        return None;
    }
    let mut digest = Sha256::new();
    digest.update(b"bitfun-feedback-cache-v2\0");
    digest.update(enroll_key.as_bytes());
    Some(digest.finalize().into())
}

fn validate_feedback_id(feedback_id: &str) -> Result<(), FeedbackError> {
    Uuid::parse_str(feedback_id)
        .map(|_| ())
        .map_err(|_| FeedbackError::validation("FEEDBACK_ID_INVALID", "Feedback ID is invalid"))
}

fn validate_timestamp(timestamp: &str) -> Result<(), FeedbackError> {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|_| ())
        .map_err(|_| FeedbackError::validation("TIMESTAMP_INVALID", "Timestamp is invalid"))
}

fn is_terminal_capability_error(code: &str) -> bool {
    matches!(
        code,
        "CAPABILITY_INVALID" | "CAPABILITY_EXPIRED" | "CAPABILITY_REVOKED"
    )
}

fn configured_base_url() -> Option<String> {
    Some(feedback_api_base_url(cfg!(debug_assertions)).to_string())
}

fn feedback_api_base_url(debug: bool) -> &'static str {
    if debug {
        DEBUG_FEEDBACK_API_BASE_URL
    } else {
        RELEASE_FEEDBACK_API_BASE_URL
    }
}

fn request_fingerprint(request: &SubmitFeedbackRequest) -> Result<String, FeedbackError> {
    let encoded = serde_json::to_vec(request).map_err(|_| {
        FeedbackError::new(
            "REQUEST_ENCODING_FAILED",
            "Feedback request could not be encoded",
            false,
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

fn normalize_request(mut request: SubmitFeedbackRequest) -> SubmitFeedbackRequest {
    request.trace_id = valid_optional_value_owned(request.trace_id, 64);
    request.session_id_hash = valid_optional_value_owned(request.session_id_hash, 64);
    request
}

fn valid_optional_value(value: &str, max_chars: usize) -> Option<&str> {
    (!value.is_empty() && value.chars().count() <= max_chars).then_some(value)
}

fn valid_optional_value_owned(value: Option<String>, max_chars: usize) -> Option<String> {
    value.filter(|item| !item.is_empty() && item.chars().count() <= max_chars)
}

fn parse_scopes(value: &str) -> Vec<String> {
    value
        .split(|character: char| character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn refresh_requires_enroll(code: &str) -> bool {
    matches!(
        code,
        "REFRESH_TOKEN_INVALID" | "REFRESH_TOKEN_REUSED" | "TOKEN_FAMILY_REVOKED"
    )
}

async fn decode_success<T: DeserializeOwned>(
    response: Response,
    expected: StatusCode,
) -> Result<T, FeedbackError> {
    if response.status() != expected {
        return Err(decode_error(response).await);
    }
    response.json::<T>().await.map_err(|_| {
        FeedbackError::new(
            "RESPONSE_INVALID",
            "Feedback service returned an invalid response",
            true,
        )
    })
}

async fn decode_error(response: Response) -> FeedbackError {
    let status = response.status();
    let request_id_header = response
        .headers()
        .get("X-Request-ID")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let retry_after_seconds = response
        .headers()
        .get("Retry-After")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let body = response.json::<ServerErrorBody>().await.unwrap_or_default();
    let code = body.error_code.unwrap_or_else(|| match status {
        StatusCode::UNAUTHORIZED => "ACCESS_TOKEN_INVALID".to_string(),
        StatusCode::FORBIDDEN => "ACCESS_FORBIDDEN".to_string(),
        StatusCode::TOO_MANY_REQUESTS => "RATE_LIMITED".to_string(),
        status if status.is_server_error() => "SERVICE_UNAVAILABLE".to_string(),
        _ => "REQUEST_REJECTED".to_string(),
    });
    let retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
    FeedbackError {
        message: safe_error_message(&code).to_string(),
        code,
        retryable,
        request_id: body.request_id.or(request_id_header),
        retry_after_seconds,
    }
}

fn safe_error_message(code: &str) -> &'static str {
    match code {
        "CONTENT_EMPTY" | "CONTENT_TOO_LONG" | "CONTENT_UNSAFE" | "CATEGORY_INVALID" => {
            "Feedback content was rejected"
        }
        "SCOPE_INSUFFICIENT" | "INSTANCE_BANNED" | "ACCESS_FORBIDDEN" => {
            "Feedback access is not permitted"
        }
        "RATE_LIMITED" | "FEEDBACK_QUOTA_EXCEEDED" | "QUOTA_EXCEEDED" => {
            "Feedback requests are temporarily limited"
        }
        _ => "Feedback request could not be completed",
    }
}

fn network_error(error: reqwest::Error) -> FeedbackError {
    if error.is_timeout() {
        FeedbackError::new("REQUEST_TIMEOUT", "Feedback request timed out", true)
    } else {
        FeedbackError::new("NETWORK_ERROR", "Feedback service is unavailable", true)
    }
}

fn credential_error(code: &str, message: &str) -> FeedbackError {
    FeedbackError::new(code, message, true)
}

#[cfg(test)]
mod tests {
    use super::super::message_cache::{MessageCache, MessageCacheData};
    use super::{
        cache_encryption_key, cached_unread_admin_reply, configured_base_url,
        feedback_api_base_url, is_terminal_capability_error, normalize_request, parse_scopes,
        user_message_reconciliation_cursor, FeedbackService, StoredCredentials,
        DEBUG_FEEDBACK_API_BASE_URL, RELEASE_FEEDBACK_API_BASE_URL,
    };
    use crate::feedback::FeedbackCredentialStore;
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use bitfun_product_domains::feedback::{
        AcknowledgeFeedbackRequest, FeedbackCategory, FeedbackMessage, FeedbackRecordSummary,
        FeedbackSender, FeedbackStatus, ListFeedbackRecordsRequest,
        OpenFeedbackConversationRequest, ReplyFeedbackRequest, SubmitFeedbackRequest,
    };
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;

    #[derive(Default)]
    struct MemoryStore {
        value: StdMutex<Option<String>>,
        stores: AtomicUsize,
        fail_at: AtomicUsize,
    }

    #[async_trait]
    impl FeedbackCredentialStore for MemoryStore {
        async fn load(&self) -> Result<Option<String>> {
            Ok(self.value.lock().unwrap().clone())
        }

        async fn store(&self, value: &str) -> Result<()> {
            let call = self.stores.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_at.load(Ordering::SeqCst) == call {
                return Err(anyhow!("injected store failure"));
            }
            *self.value.lock().unwrap() = Some(value.to_string());
            Ok(())
        }
    }

    #[test]
    fn accepts_backend_scope_delimiters() {
        assert_eq!(
            parse_scopes("config:read,feedback:write feedback:read"),
            vec!["config:read", "feedback:write", "feedback:read"]
        );
    }

    #[test]
    fn selects_a_fixed_feedback_endpoint_for_each_build_profile() {
        assert_eq!(
            DEBUG_FEEDBACK_API_BASE_URL,
            "http://api-test.infra-bitfun.com"
        );
        assert_eq!(
            RELEASE_FEEDBACK_API_BASE_URL,
            "https://api.infra-bitfun.com"
        );
        assert_eq!(feedback_api_base_url(true), DEBUG_FEEDBACK_API_BASE_URL);
        assert_eq!(feedback_api_base_url(false), RELEASE_FEEDBACK_API_BASE_URL);
        assert_eq!(
            configured_base_url().as_deref(),
            Some(feedback_api_base_url(cfg!(debug_assertions)))
        );
    }

    #[test]
    fn only_explicit_terminal_capability_errors_invalidate_conversation_access() {
        for code in [
            "CAPABILITY_INVALID",
            "CAPABILITY_EXPIRED",
            "CAPABILITY_REVOKED",
        ] {
            assert!(is_terminal_capability_error(code), "{code}");
        }
        for code in [
            "CAPABILITY_UNAVAILABLE",
            "CAPABILITY_REQUIRED",
            "FEEDBACK_ACCESS_DENIED",
            "FEEDBACK_NOT_FOUND",
            "FEEDBACK_ACCESS_UNAVAILABLE",
            "FEEDBACK_ACCESS_EXPIRED",
            "SCOPE_INSUFFICIENT",
            "INSTANCE_BANNED",
            "ACCESS_TOKEN_INVALID",
        ] {
            assert!(!is_terminal_capability_error(code), "{code}");
        }
    }

    #[test]
    fn omits_invalid_optional_correlation_values() {
        let normalized = normalize_request(SubmitFeedbackRequest {
            category: FeedbackCategory::Other,
            content: "feedback".to_string(),
            trace_id: Some(String::new()),
            session_id_hash: Some("x".repeat(65)),
        });
        assert_eq!(normalized.trace_id, None);
        assert_eq!(normalized.session_id_hash, None);
    }

    #[test]
    fn distinguishes_user_messages_from_unread_admin_replies_in_complete_cache() {
        let mut cached = MessageCacheData::empty("feedback-1");
        cached.sync_complete = true;
        cached.messages.push(FeedbackMessage {
            message_id: "message-user".to_string(),
            sender: FeedbackSender::User,
            content: "own reply".to_string(),
            content_deleted: false,
            created_at: "2026-07-28T02:00:00Z".to_string(),
        });
        assert_eq!(cached_unread_admin_reply(&cached, None), Some(false));

        cached.messages.push(FeedbackMessage {
            message_id: "message-admin".to_string(),
            sender: FeedbackSender::Admin,
            content: "support reply".to_string(),
            content_deleted: false,
            created_at: "2026-07-28T03:00:00Z".to_string(),
        });
        assert_eq!(cached_unread_admin_reply(&cached, None), Some(true));
        assert_eq!(user_message_reconciliation_cursor(&cached, None), None);
        assert_eq!(
            cached_unread_admin_reply(&cached, Some(&"2026-07-28T03:00:00Z".to_string()),),
            Some(false)
        );

        cached.messages.push(FeedbackMessage {
            message_id: "message-user-latest".to_string(),
            sender: FeedbackSender::User,
            content: "follow-up".to_string(),
            content_deleted: false,
            created_at: "2026-07-28T04:00:00Z".to_string(),
        });
        assert_eq!(
            user_message_reconciliation_cursor(&cached, Some(&"2026-07-28T03:00:00Z".to_string()),)
                .as_deref(),
            Some("2026-07-28T04:00:00Z")
        );

        cached.sync_complete = false;
        assert_eq!(cached_unread_admin_reply(&cached, None), None);
    }

    #[test]
    fn secure_credentials_exclude_rebuildable_feedback_state() {
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            refresh_token: Some("refresh".to_string()),
            capabilities: HashMap::from([("feedback-1".to_string(), "capability".to_string())]),
            anonymous_id: Some("anonymous".to_string()),
            inbox_items: vec![FeedbackRecordSummary {
                feedback_id: "feedback-1".to_string(),
                category: FeedbackCategory::Other,
                status: FeedbackStatus::Submitted,
                has_new_reply: false,
                created_at: "2026-07-30T01:00:00Z".to_string(),
                updated_at: "2026-07-30T01:00:00Z".to_string(),
                can_open: true,
            }],
            pending_create_idempotency_key: Some("idempotency".to_string()),
            ..StoredCredentials::default()
        };

        let serialized = serde_json::to_value(&stored).unwrap();

        assert_eq!(serialized["enroll_key"], "enroll");
        assert_eq!(serialized["refresh_token"], "refresh");
        assert_eq!(serialized["capabilities"]["feedback-1"], "capability");
        assert!(serialized.get("anonymous_id").is_none());
        assert!(serialized.get("inbox_items").is_none());
        assert!(serialized.get("pending_create_idempotency_key").is_none());
    }

    #[tokio::test]
    async fn access_state_does_not_create_an_identity() {
        let store = Arc::new(MemoryStore::default());
        let identity_dir = test_cache_dir("identity-not-created");
        let identity_path = identity_dir.join("identity.json");
        let service = FeedbackService::new(
            None,
            "1.0.0".to_string(),
            store.clone(),
            Duration::from_secs(2),
        )
        .with_identity_path(identity_path.clone());

        let state = service.access_state().await.unwrap();

        assert!(!state.has_history);
        assert!(!state.can_reuse_access);
        assert_eq!(store.stores.load(Ordering::SeqCst), 0);
        assert!(!identity_path.exists());
        let _ = tokio::fs::remove_dir_all(identity_dir).await;
    }

    #[tokio::test]
    async fn successful_enrollment_saves_the_plain_anonymous_identity_copy() {
        let responses = vec![json_response(
            201,
            r#"{"anonymous_id":"11111111-1111-4111-8111-111111111111","access_token":"access","refresh_token":"refresh","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
        )];
        let (base_url, _) = spawn_server(responses).await;
        let cache_dir = test_cache_dir("identity-copy");
        let identity_path = cache_dir.join("config").join("identity.json");
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            Arc::new(MemoryStore::default()),
            Duration::from_secs(2),
        )
        .with_cache_dir(cache_dir.join("cache"))
        .with_identity_path(identity_path.clone());

        service.access_token("feedback:write", false).await.unwrap();

        let value: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(identity_path).await.unwrap()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["anonymousId"], "11111111-1111-4111-8111-111111111111");
        assert_eq!(value.as_object().unwrap().len(), 2);
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn existing_cached_identity_is_copied_without_enrolling() {
        let cache_dir = test_cache_dir("existing-identity-copy");
        let identity_path = cache_dir.join("config").join("identity.json");
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            anonymous_id: Some("22222222-2222-4222-8222-222222222222".to_string()),
            refresh_token: Some("refresh".to_string()),
            ..StoredCredentials::default()
        };
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(
                serde_json::json!({
                    "enroll_key": stored.enroll_key,
                    "anonymous_id": stored.anonymous_id,
                    "refresh_token": stored.refresh_token,
                })
                .to_string(),
            )),
            ..MemoryStore::default()
        });
        let service = FeedbackService::new(
            None,
            "1.0.0".to_string(),
            store.clone(),
            Duration::from_secs(2),
        )
        .with_identity_path(identity_path.clone());

        let state = service.access_state().await.unwrap();

        assert!(state.can_reuse_access);
        let value: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(identity_path).await.unwrap()).unwrap();
        assert_eq!(value["anonymousId"], "22222222-2222-4222-8222-222222222222");
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn lists_inbox_with_backend_cursor_mapping_and_cached_accessibility() {
        let responses = vec![
            json_response(
                200,
                r#"{"anonymous_id":"anon","access_token":"fresh","refresh_token":"refresh-2","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                200,
                r#"{"items":[{"feedback_id":"feedback-1","category":"other","status":"waiting_user","has_new_reply":true,"created_at":"2026-07-28T01:00:00Z","updated_at":"2026-07-28T02:00:00Z"}],"cursor":"cursor-2","has_more":true}"#,
            ),
        ];
        let (base_url, requests) = spawn_server(responses).await;
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            anonymous_id: Some("anon".to_string()),
            refresh_token: Some("refresh-1".to_string()),
            capabilities: HashMap::from([("feedback-1".to_string(), "capability".to_string())]),
            ..StoredCredentials::default()
        };
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(serde_json::to_string(&stored).unwrap())),
            ..MemoryStore::default()
        });
        let cache_dir = test_cache_dir("inbox");
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            store.clone(),
            Duration::from_secs(2),
        )
        .with_cache_dir(cache_dir.clone());

        let page = service
            .list_feedback(ListFeedbackRecordsRequest {
                cursor: Some("cursor-1".to_string()),
                page_size: 20,
            })
            .await
            .unwrap();

        assert_eq!(page.next_cursor.as_deref(), Some("cursor-2"));
        assert!(page.has_more);
        assert!(page.items[0].can_open);
        let captured = requests.lock().unwrap();
        assert!(captured[1].starts_with("GET /support/v1/feedback/inbox?limit=20&cursor=cursor-1 "));
        drop(captured);
        let restored = service.access_state().await.unwrap();
        assert!(restored.cached_inbox.items[0].has_new_reply);
        let secure_value = store.value.lock().unwrap().clone().unwrap();
        assert!(!secure_value.contains("inbox_items"));

        let restarted =
            FeedbackService::new(None, "1.0.0".to_string(), store, Duration::from_secs(2))
                .with_cache_dir(cache_dir.clone());
        let restored_after_restart = restarted.access_state().await.unwrap();
        assert_eq!(restored_after_restart.cached_inbox.items.len(), 1);
        assert!(restored_after_restart.cached_inbox.items[0].has_new_reply);
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn returns_refreshed_inbox_when_local_cache_write_fails() {
        let responses = vec![
            json_response(
                200,
                r#"{"anonymous_id":"anon","access_token":"fresh","refresh_token":"refresh-2","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                200,
                r#"{"items":[{"feedback_id":"feedback-1","category":"other","status":"submitted","has_new_reply":false,"created_at":"2026-07-30T01:00:00Z","updated_at":"2026-07-30T01:00:00Z"}],"cursor":"","has_more":false}"#,
            ),
            json_response(
                200,
                r#"{"items":[{"feedback_id":"feedback-1","category":"other","status":"submitted","has_new_reply":false,"created_at":"2026-07-30T01:00:00Z","updated_at":"2026-07-30T01:00:00Z"},{"feedback_id":"feedback-2","category":"runtime_error","status":"submitted","has_new_reply":false,"created_at":"2026-07-30T01:05:00Z","updated_at":"2026-07-30T01:05:00Z"}],"cursor":"","has_more":false}"#,
            ),
        ];
        let (base_url, _) = spawn_server(responses).await;
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            refresh_token: Some("refresh-1".to_string()),
            ..StoredCredentials::default()
        };
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(serde_json::to_string(&stored).unwrap())),
            ..MemoryStore::default()
        });
        let cache_dir = test_cache_dir("unwritable-cache");
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            store,
            Duration::from_secs(2),
        )
        .with_cache_dir(cache_dir.clone());

        let first = service
            .list_feedback(ListFeedbackRecordsRequest {
                cursor: None,
                page_size: 20,
            })
            .await
            .unwrap();
        assert_eq!(first.items.len(), 1);

        tokio::fs::remove_dir_all(&cache_dir).await.unwrap();
        tokio::fs::write(&cache_dir, b"not a directory")
            .await
            .unwrap();
        let second = service
            .list_feedback(ListFeedbackRecordsRequest {
                cursor: None,
                page_size: 20,
            })
            .await
            .unwrap();

        assert_eq!(second.items.len(), 2);
        let _ = tokio::fs::remove_file(cache_dir).await;
    }

    #[tokio::test]
    async fn rebuilds_message_cache_then_pages_latest_to_earliest_and_acks_server_state() {
        let feedback_id = "11111111-1111-4111-8111-111111111111";
        let responses = vec![
            json_response(
                200,
                r#"{"anonymous_id":"anon","access_token":"fresh","refresh_token":"refresh-2","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                200,
                r#"{"feedback_id":"11111111-1111-4111-8111-111111111111","messages":[{"message_id":"message-1","sender_type":"user","content":"first","content_deleted":false,"created_at":"2026-07-28T01:00:00Z"}],"cursor":"cursor-1","has_more":true}"#,
            ),
            json_response(
                200,
                r#"{"feedback_id":"11111111-1111-4111-8111-111111111111","messages":[{"message_id":"message-2","sender_type":"admin","content":"second","content_deleted":true,"created_at":"2026-07-28T02:00:00Z"}],"cursor":"cursor-2","has_more":false}"#,
            ),
            json_response(
                200,
                r#"{"feedback_id":"11111111-1111-4111-8111-111111111111","read_cursor":"2026-07-28T02:00:00Z","feedback_status":"in_progress"}"#,
            ),
        ];
        let (base_url, requests) = spawn_server(responses).await;
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            anonymous_id: Some("anon".to_string()),
            refresh_token: Some("refresh-1".to_string()),
            capabilities: HashMap::from([(
                feedback_id.to_string(),
                "capability-secret".to_string(),
            )]),
            ..StoredCredentials::default()
        };
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(serde_json::to_string(&stored).unwrap())),
            ..MemoryStore::default()
        });
        let cache_dir = std::env::temp_dir().join(format!(
            "bitfun-feedback-service-cache-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            store,
            Duration::from_secs(2),
        )
        .with_message_cache_dir(cache_dir.clone());

        let latest = service
            .open_conversation(OpenFeedbackConversationRequest {
                feedback_id: feedback_id.to_string(),
                cursor: None,
                page_size: 1,
            })
            .await
            .unwrap();
        assert_eq!(latest.messages[0].content, "second");
        assert!(latest.messages[0].content_deleted);
        assert_eq!(latest.next_cursor.as_deref(), Some("cache:1"));
        assert!(latest.has_more);

        let earlier = service
            .open_conversation(OpenFeedbackConversationRequest {
                feedback_id: feedback_id.to_string(),
                cursor: latest.next_cursor,
                page_size: 1,
            })
            .await
            .unwrap();
        assert_eq!(earlier.messages[0].content, "first");
        assert!(!earlier.messages[0].content_deleted);
        assert!(!earlier.has_more);

        let acknowledged = service
            .acknowledge_feedback(AcknowledgeFeedbackRequest {
                feedback_id: feedback_id.to_string(),
                last_visible_at: "2026-07-28T02:00:00Z".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(acknowledged.read_through, "2026-07-28T02:00:00Z");
        assert_eq!(acknowledged.feedback_status, FeedbackStatus::InProgress);

        let captured = requests.lock().unwrap();
        assert!(captured[1].starts_with(
            "GET /support/v1/feedback/11111111-1111-4111-8111-111111111111/messages?limit=50 "
        ));
        assert!(captured[2].contains("cursor=cursor-1"));
        assert_eq!(
            header(&captured[1], "x-feedback-capability").as_deref(),
            Some("capability-secret")
        );
        assert!(captured[3]
            .starts_with("POST /support/v1/feedback/11111111-1111-4111-8111-111111111111/ack "));
        drop(captured);
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn clears_false_unread_marker_when_synced_history_contains_only_user_messages() {
        let feedback_id = "11111111-1111-4111-8111-111111111111";
        let responses = vec![
            json_response(
                200,
                r#"{"anonymous_id":"anon","access_token":"fresh","refresh_token":"refresh-2","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                200,
                r#"{"items":[{"feedback_id":"11111111-1111-4111-8111-111111111111","category":"other","status":"submitted","has_new_reply":true,"created_at":"2026-07-28T01:00:00Z","updated_at":"2026-07-28T02:00:00Z"}],"cursor":"","has_more":false}"#,
            ),
            json_response(
                200,
                r#"{"feedback_id":"11111111-1111-4111-8111-111111111111","messages":[{"message_id":"22222222-2222-4222-8222-222222222222","sender_type":"user","content":"initial report","content_deleted":false,"created_at":"2026-07-28T01:00:00Z"},{"message_id":"33333333-3333-4333-8333-333333333333","sender_type":"user","content":"own follow-up","content_deleted":false,"created_at":"2026-07-28T02:00:00Z"}],"cursor":"cursor-2","has_more":false}"#,
            ),
            json_response(
                200,
                r#"{"feedback_id":"11111111-1111-4111-8111-111111111111","read_cursor":"2026-07-28T02:00:00Z","feedback_status":"submitted"}"#,
            ),
        ];
        let (base_url, requests) = spawn_server(responses).await;
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            anonymous_id: Some("anon".to_string()),
            refresh_token: Some("refresh-1".to_string()),
            capabilities: HashMap::from([(
                feedback_id.to_string(),
                "capability-secret".to_string(),
            )]),
            ..StoredCredentials::default()
        };
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(serde_json::to_string(&stored).unwrap())),
            ..MemoryStore::default()
        });
        let cache_dir = test_cache_dir("own-reply-unread");
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            store,
            Duration::from_secs(2),
        )
        .with_message_cache_dir(cache_dir.clone());

        let inbox = service
            .list_feedback(ListFeedbackRecordsRequest {
                cursor: None,
                page_size: 20,
            })
            .await
            .unwrap();
        assert!(inbox.items[0].has_new_reply);

        service
            .open_conversation(OpenFeedbackConversationRequest {
                feedback_id: feedback_id.to_string(),
                cursor: None,
                page_size: 20,
            })
            .await
            .unwrap();

        let restored = service.access_state().await.unwrap();
        assert!(!restored.cached_inbox.items[0].has_new_reply);
        let captured = requests.lock().unwrap();
        assert!(captured[3]
            .starts_with("POST /support/v1/feedback/11111111-1111-4111-8111-111111111111/ack "));
        drop(captured);
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn preserves_capability_when_conversation_sync_is_denied_by_scope() {
        let feedback_id = "11111111-1111-4111-8111-111111111111";
        let responses = vec![
            json_response(
                200,
                r#"{"anonymous_id":"anon","access_token":"fresh","refresh_token":"refresh-2","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                403,
                r#"{"error_code":"SCOPE_INSUFFICIENT","request_id":"request-scope"}"#,
            ),
        ];
        let (base_url, _) = spawn_server(responses).await;
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            anonymous_id: Some("anon".to_string()),
            refresh_token: Some("refresh-1".to_string()),
            capabilities: HashMap::from([(
                feedback_id.to_string(),
                "capability-secret".to_string(),
            )]),
            ..StoredCredentials::default()
        };
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(serde_json::to_string(&stored).unwrap())),
            ..MemoryStore::default()
        });
        let cache_dir = test_cache_dir("scope-preserves-capability");
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            store.clone(),
            Duration::from_secs(2),
        )
        .with_message_cache_dir(cache_dir.clone());

        let page = service
            .open_conversation(OpenFeedbackConversationRequest {
                feedback_id: feedback_id.to_string(),
                cursor: None,
                page_size: 20,
            })
            .await
            .unwrap();

        assert_eq!(page.sync_error.as_ref().unwrap().code, "SCOPE_INSUFFICIENT");
        let secure_value = store.value.lock().unwrap().clone().unwrap();
        let persisted: StoredCredentials = serde_json::from_str(&secure_value).unwrap();
        assert_eq!(
            persisted.capabilities.get(feedback_id).map(String::as_str),
            Some("capability-secret")
        );
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn reuses_reply_idempotency_key_and_accepts_committed_server_status() {
        let feedback_id = "11111111-1111-4111-8111-111111111111";
        let message_id = "22222222-2222-4222-8222-222222222222";
        let responses = vec![
            json_response(
                200,
                r#"{"anonymous_id":"anon","access_token":"fresh","refresh_token":"refresh-2","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                503,
                r#"{"error_code":"INTERNAL_ERROR","request_id":"request-failed"}"#,
            ),
            json_response(
                201,
                &format!(
                    r#"{{"message_id":"{message_id}","sender_type":"user","created_at":"2026-07-28T03:00:00Z","feedback_status":"submitted"}}"#,
                ),
            ),
            json_response(
                200,
                r#"{"feedback_id":"11111111-1111-4111-8111-111111111111","read_cursor":"2026-07-28T03:00:00Z","feedback_status":"submitted"}"#,
            ),
        ];
        let (base_url, requests) = spawn_server(responses).await;
        let stored = StoredCredentials {
            enroll_key: "enroll".to_string(),
            anonymous_id: Some("anon".to_string()),
            refresh_token: Some("refresh-1".to_string()),
            capabilities: HashMap::from([(
                feedback_id.to_string(),
                "capability-secret".to_string(),
            )]),
            ..StoredCredentials::default()
        };
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(serde_json::to_string(&stored).unwrap())),
            ..MemoryStore::default()
        });
        let cache_dir = std::env::temp_dir().join(format!(
            "bitfun-feedback-reply-cache-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let cache = MessageCache::new(cache_dir.clone(), cache_encryption_key("enroll").unwrap());
        let mut cached = MessageCacheData::empty(feedback_id);
        cached.sync_complete = true;
        cache.store(&cached).await.unwrap();
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            store.clone(),
            Duration::from_secs(2),
        )
        .with_message_cache_dir(cache_dir.clone());
        let request = ReplyFeedbackRequest {
            feedback_id: feedback_id.to_string(),
            content: "  retry safely  ".to_string(),
        };

        let first = service.reply_feedback(request.clone()).await.unwrap_err();
        assert_eq!(first.code, "INTERNAL_ERROR");
        let replied = service.reply_feedback(request).await.unwrap();
        assert_eq!(replied.message.message_id, message_id);
        assert_eq!(replied.message.content, "retry safely");
        assert_eq!(replied.feedback_status, FeedbackStatus::Submitted);

        let captured = requests.lock().unwrap();
        let replies = captured
            .iter()
            .filter(|request| {
                request.starts_with(&format!(
                    "POST /support/v1/feedback/{feedback_id}/messages "
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(replies.len(), 2);
        assert_eq!(
            header(replies[0], "idempotency-key"),
            header(replies[1], "idempotency-key")
        );
        assert_eq!(
            header(replies[1], "x-feedback-capability").as_deref(),
            Some("capability-secret")
        );
        assert!(captured[3]
            .starts_with("POST /support/v1/feedback/11111111-1111-4111-8111-111111111111/ack "));
        drop(captured);
        let persisted: StoredCredentials =
            serde_json::from_str(store.value.lock().unwrap().as_deref().unwrap()).unwrap();
        assert!(persisted.pending_reply_fingerprints.is_empty());
        assert!(persisted.pending_reply_idempotency_keys.is_empty());
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn rejects_replies_to_locally_known_resolved_feedback() {
        let feedback_id = "11111111-1111-4111-8111-111111111111";
        let store = Arc::new(MemoryStore {
            value: StdMutex::new(Some(
                serde_json::json!({
                    "enroll_key": "enroll",
                    "inbox_items": [{
                        "feedbackId": feedback_id,
                        "category": "other",
                        "status": "resolved",
                        "hasNewReply": false,
                        "createdAt": "2026-07-28T01:00:00Z",
                        "updatedAt": "2026-07-28T02:00:00Z",
                        "canOpen": true
                    }]
                })
                .to_string(),
            )),
            ..MemoryStore::default()
        });
        let service =
            FeedbackService::new(None, "1.0.0".to_string(), store, Duration::from_secs(2));

        let error = service
            .reply_feedback(ReplyFeedbackRequest {
                feedback_id: feedback_id.to_string(),
                content: "should not send".to_string(),
            })
            .await
            .unwrap_err();

        assert_eq!(error.code, "FEEDBACK_ALREADY_RESOLVED");
    }

    #[tokio::test]
    async fn capability_persistence_is_part_of_submission_success() {
        let responses = vec![
            json_response(
                201,
                r#"{"anonymous_id":"anon","access_token":"access","refresh_token":"refresh","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                201,
                r#"{"feedback_id":"feedback-1","capability_token":"capability","status":"submitted","inbox_cursor":"cursor-1","schema_version":"1"}"#,
            ),
            json_response(
                201,
                r#"{"feedback_id":"feedback-1","capability_token":"capability","status":"submitted","inbox_cursor":"cursor-1","schema_version":"1","idempotency_replayed":true}"#,
            ),
        ];
        let (base_url, requests) = spawn_server(responses).await;
        let store = Arc::new(MemoryStore::default());
        store.fail_at.store(3, Ordering::SeqCst);
        let cache_dir = test_cache_dir("capability-save");
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            store,
            Duration::from_secs(2),
        )
        .with_cache_dir(cache_dir.clone());
        let request = SubmitFeedbackRequest {
            category: FeedbackCategory::Other,
            content: "privacy-safe feedback".to_string(),
            trace_id: None,
            session_id_hash: None,
        };

        let first = service.submit_feedback(request.clone()).await.unwrap_err();
        assert_eq!(first.code, "CAPABILITY_SAVE_FAILED");
        let second = service.submit_feedback(request).await.unwrap();
        assert_eq!(second.feedback_id, "feedback-1");

        let captured = requests.lock().unwrap();
        let create_keys: Vec<_> = captured
            .iter()
            .filter(|request| request.starts_with("POST /support/v1/feedback "))
            .filter_map(|request| header(request, "idempotency-key"))
            .collect();
        assert_eq!(create_keys.len(), 2);
        assert_eq!(create_keys[0], create_keys[1]);
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    #[tokio::test]
    async fn recovers_from_one_unauthorized_response_and_replays_once() {
        let responses = vec![
            json_response(
                201,
                r#"{"anonymous_id":"anon","access_token":"expired","refresh_token":"refresh-1","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                401,
                r#"{"error_code":"ACCESS_TOKEN_INVALID","error_message":"expired","request_id":"request-401"}"#,
            ),
            json_response(
                200,
                r#"{"anonymous_id":"anon","access_token":"fresh","refresh_token":"refresh-2","expires_in":3600,"refresh_expires_in":2592000,"scope":"feedback:write,feedback:read","schema_version":"1"}"#,
            ),
            json_response(
                201,
                r#"{"feedback_id":"feedback-1","capability_token":"capability","status":"submitted","inbox_cursor":"cursor-1","schema_version":"1"}"#,
            ),
        ];
        let (base_url, requests) = spawn_server(responses).await;
        let cache_dir = test_cache_dir("unauthorized-recovery");
        let service = FeedbackService::new(
            Some(base_url),
            "1.0.0".to_string(),
            Arc::new(MemoryStore::default()),
            Duration::from_secs(2),
        )
        .with_cache_dir(cache_dir.clone());
        service
            .submit_feedback(SubmitFeedbackRequest {
                category: FeedbackCategory::Other,
                content: "recover once".to_string(),
                trace_id: None,
                session_id_hash: None,
            })
            .await
            .unwrap();

        let captured = requests.lock().unwrap();
        assert!(captured[0].starts_with("POST /auth/v1/anonymous/enroll "));
        assert!(captured[1].starts_with("POST /support/v1/feedback "));
        assert!(captured[2].starts_with("POST /auth/v1/anonymous/token "));
        assert!(captured[3].starts_with("POST /support/v1/feedback "));
        assert_eq!(
            header(&captured[1], "idempotency-key"),
            header(&captured[3], "idempotency-key")
        );
        drop(captured);
        let _ = tokio::fs::remove_dir_all(cache_dir).await;
    }

    fn test_cache_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "bitfun-feedback-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }

    fn json_response(status: u16, body: &str) -> String {
        format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    async fn spawn_server(responses: Vec<String>) -> (String, Arc<StdMutex<Vec<String>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let captured = requests.clone();
        tokio::spawn(async move {
            for response in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).await.unwrap();
                    if read == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..read]);
                    if request_is_complete(&bytes) {
                        break;
                    }
                }
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&bytes).into_owned());
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (format!("http://{address}"), requests)
    }

    fn request_is_complete(bytes: &[u8]) -> bool {
        let request = String::from_utf8_lossy(bytes);
        let Some(header_end) = request.find("\r\n\r\n") else {
            return false;
        };
        let content_length = request[..header_end]
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        bytes.len() >= header_end + 4 + content_length
    }

    fn header(request: &str, name: &str) -> Option<String> {
        request.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
    }
}
