//! MiniApp draft DTOs and pure response helpers.

use crate::miniapp::types::MiniApp;
use serde::{Deserialize, Serialize};

pub const MINIAPP_DRAFT_STATUS_DRAFT: &str = "draft";
pub const MINIAPP_DRAFT_STATUS_APPLIED: &str = "applied";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppDraftManifest {
    pub app_id: String,
    pub draft_id: String,
    pub source_version: u32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl MiniAppDraftManifest {
    pub fn mark_applied(&mut self, applied_at: i64) {
        self.status = MINIAPP_DRAFT_STATUS_APPLIED.to_string();
        self.updated_at = applied_at;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppDraft {
    pub app_id: String,
    pub draft_id: String,
    pub source_version: u32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub draft_root: String,
    pub app: MiniApp,
}

pub fn build_draft_manifest(
    app_id: impl Into<String>,
    draft_id: impl Into<String>,
    source_version: u32,
    now: i64,
) -> MiniAppDraftManifest {
    MiniAppDraftManifest {
        app_id: app_id.into(),
        draft_id: draft_id.into(),
        source_version,
        status: MINIAPP_DRAFT_STATUS_DRAFT.to_string(),
        created_at: now,
        updated_at: now,
    }
}

pub fn build_draft_response(
    draft_root: impl Into<String>,
    app: MiniApp,
    manifest: MiniAppDraftManifest,
) -> MiniAppDraft {
    MiniAppDraft {
        app_id: manifest.app_id,
        draft_id: manifest.draft_id,
        source_version: manifest.source_version,
        status: manifest.status,
        created_at: manifest.created_at,
        updated_at: manifest.updated_at,
        draft_root: draft_root.into(),
        app,
    }
}
