//! Per-page mutable runtime data (KV / SQLite / Blobs), keyed by (user_id, slug).
//! Survives version deploy/rollback; separate from immutable version assets.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bitfun_page_function_runtime::{PageHost, PageMeta};
use chrono::Utc;
use tokio::runtime::Handle;

use crate::db::{page_kv, DbPool};

/// Root directory for page-data (`{base}/{user_id}/{slug}/...`).
#[derive(Clone)]
pub struct PageDataStore {
    base_dir: PathBuf,
}

impl PageDataStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        let base_dir = base_dir.into();
        let _ = std::fs::create_dir_all(&base_dir);
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn page_dir(&self, user_id: &str, slug: &str) -> PathBuf {
        self.base_dir.join(user_id).join(slug)
    }

    pub fn db_path(&self, user_id: &str, slug: &str) -> PathBuf {
        self.page_dir(user_id, slug).join("db.sqlite")
    }

    pub fn blobs_dir(&self, user_id: &str, slug: &str) -> PathBuf {
        self.page_dir(user_id, slug).join("blobs")
    }

    pub fn cleanup_page(&self, user_id: &str, slug: &str) {
        let dir = self.page_dir(user_id, slug);
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    fn ensure_page_dir(&self, user_id: &str, slug: &str) -> Result<PathBuf> {
        let dir = self.page_dir(user_id, slug);
        std::fs::create_dir_all(&dir).map_err(|e| anyhow!("create page-data dir: {e}"))?;
        Ok(dir)
    }

    pub fn blob_put(
        &self,
        user_id: &str,
        slug: &str,
        blob_id: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<()> {
        if blob_id.contains("..") || blob_id.contains('/') || blob_id.contains('\\') {
            return Err(anyhow!("invalid blob id"));
        }
        let blobs = self.blobs_dir(user_id, slug);
        std::fs::create_dir_all(&blobs).map_err(|e| anyhow!("create blobs dir: {e}"))?;
        let path = blobs.join(blob_id);
        std::fs::write(&path, data).map_err(|e| anyhow!("write blob: {e}"))?;
        let meta = blobs.join(format!("{blob_id}.meta"));
        std::fs::write(&meta, content_type).map_err(|e| anyhow!("write blob meta: {e}"))?;
        let _ = content_type;
        let _ = Utc::now();
        Ok(())
    }

    pub fn blob_get(
        &self,
        user_id: &str,
        slug: &str,
        blob_id: &str,
    ) -> Result<Option<(String, Vec<u8>)>> {
        if blob_id.contains("..") || blob_id.contains('/') || blob_id.contains('\\') {
            return Err(anyhow!("invalid blob id"));
        }
        let path = self.blobs_dir(user_id, slug).join(blob_id);
        if !path.is_file() {
            return Ok(None);
        }
        let data = std::fs::read(&path).map_err(|e| anyhow!("read blob: {e}"))?;
        let meta = self
            .blobs_dir(user_id, slug)
            .join(format!("{blob_id}.meta"));
        let content_type = std::fs::read_to_string(&meta)
            .unwrap_or_else(|_| "application/octet-stream".to_string());
        Ok(Some((content_type, data)))
    }

    pub fn blob_delete(&self, user_id: &str, slug: &str, blob_id: &str) -> Result<bool> {
        if blob_id.contains("..") || blob_id.contains('/') || blob_id.contains('\\') {
            return Err(anyhow!("invalid blob id"));
        }
        let path = self.blobs_dir(user_id, slug).join(blob_id);
        let meta = self
            .blobs_dir(user_id, slug)
            .join(format!("{blob_id}.meta"));
        let existed = path.exists();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&meta);
        Ok(existed)
    }

    pub fn db_execute(
        &self,
        user_id: &str,
        slug: &str,
        sql: &str,
        params_json: &str,
    ) -> Result<String> {
        self.ensure_page_dir(user_id, slug)?;
        let path = self.db_path(user_id, slug);
        let conn = rusqlite::Connection::open(&path).map_err(|e| anyhow!("open page db: {e}"))?;
        let params: Vec<serde_json::Value> =
            serde_json::from_str(params_json).unwrap_or_else(|_| Vec::new());
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow!("prepare: {e}"))?;
        let changes = stmt
            .execute(rusqlite::params_from_iter(params.iter().map(json_to_sql)))
            .map_err(|e| anyhow!("execute: {e}"))?;
        Ok(serde_json::json!({ "ok": true, "changes": changes }).to_string())
    }

    pub fn db_query(
        &self,
        user_id: &str,
        slug: &str,
        sql: &str,
        params_json: &str,
    ) -> Result<String> {
        self.ensure_page_dir(user_id, slug)?;
        let path = self.db_path(user_id, slug);
        let conn = rusqlite::Connection::open(&path).map_err(|e| anyhow!("open page db: {e}"))?;
        let params: Vec<serde_json::Value> =
            serde_json::from_str(params_json).unwrap_or_else(|_| Vec::new());
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow!("prepare: {e}"))?;
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();
        let mut rows_out = Vec::new();
        let mut rows = stmt
            .query(rusqlite::params_from_iter(params.iter().map(json_to_sql)))
            .map_err(|e| anyhow!("query: {e}"))?;
        while let Some(row) = rows.next().map_err(|e| anyhow!("row: {e}"))? {
            let mut obj = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val = match row.get_ref(i).map_err(|e| anyhow!("get: {e}"))? {
                    rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => serde_json::json!(n),
                    rusqlite::types::ValueRef::Real(n) => serde_json::json!(n),
                    rusqlite::types::ValueRef::Text(t) => {
                        serde_json::Value::String(String::from_utf8_lossy(t).into_owned())
                    }
                    rusqlite::types::ValueRef::Blob(b) => serde_json::Value::String(B64.encode(b)),
                };
                obj.insert(name.clone(), val);
            }
            rows_out.push(serde_json::Value::Object(obj));
        }
        Ok(serde_json::json!({ "ok": true, "rows": rows_out }).to_string())
    }
}

fn json_to_sql(v: &serde_json::Value) -> rusqlite::types::Value {
    match v {
        serde_json::Value::Null => rusqlite::types::Value::Null,
        serde_json::Value::Bool(b) => rusqlite::types::Value::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                rusqlite::types::Value::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        other => rusqlite::types::Value::Text(other.to_string()),
    }
}

/// Sync PageHost bridging account DB KV + page-data store + version assets.
pub struct RelayPageHost {
    pub db: Arc<DbPool>,
    pub page_data: PageDataStore,
    pub user_id: String,
    pub slug: String,
    pub meta: PageMeta,
    pub asset_store: Arc<dyn crate::WebAssetStore>,
    pub asset_key: String,
}

impl RelayPageHost {
    fn block_on_kv<T>(
        &self,
        fut: impl std::future::Future<Output = Result<T>>,
    ) -> Result<T, String> {
        match Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| handle.block_on(fut)).map_err(|e| e.to_string())
            }
            Err(_) => {
                // Outside tokio (unit tests): create a tiny runtime.
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;
                rt.block_on(fut).map_err(|e| e.to_string())
            }
        }
    }
}

impl PageHost for RelayPageHost {
    fn kv_get(&self, key: &str) -> Result<Option<String>, String> {
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        let key = key.to_string();
        self.block_on_kv(async move { page_kv::get(&db, &user_id, &slug, &key).await })
    }

    fn kv_put(&self, key: &str, value: &str) -> Result<(), String> {
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        let key = key.to_string();
        let value = value.to_string();
        self.block_on_kv(async move { page_kv::put(&db, &user_id, &slug, &key, &value).await })
    }

    fn kv_delete(&self, key: &str) -> Result<bool, String> {
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        let key = key.to_string();
        self.block_on_kv(async move { page_kv::delete(&db, &user_id, &slug, &key).await })
    }

    fn kv_list(&self) -> Result<Vec<String>, String> {
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        self.block_on_kv(async move { page_kv::list_keys(&db, &user_id, &slug).await })
    }

    fn db_execute(&self, sql: &str, params_json: &str) -> Result<String, String> {
        self.page_data
            .db_execute(&self.user_id, &self.slug, sql, params_json)
            .map_err(|e| e.to_string())
    }

    fn db_query(&self, sql: &str, params_json: &str) -> Result<String, String> {
        self.page_data
            .db_query(&self.user_id, &self.slug, sql, params_json)
            .map_err(|e| e.to_string())
    }

    fn blob_put(&self, blob_id: &str, content_type: &str, data_b64: &str) -> Result<(), String> {
        let data = B64.decode(data_b64).map_err(|e| e.to_string())?;
        self.page_data
            .blob_put(&self.user_id, &self.slug, blob_id, content_type, &data)
            .map_err(|e| e.to_string())
    }

    fn blob_get(&self, blob_id: &str) -> Result<Option<(String, String)>, String> {
        match self
            .page_data
            .blob_get(&self.user_id, &self.slug, blob_id)
            .map_err(|e| e.to_string())?
        {
            Some((ct, data)) => Ok(Some((ct, B64.encode(data)))),
            None => Ok(None),
        }
    }

    fn blob_delete(&self, blob_id: &str) -> Result<bool, String> {
        self.page_data
            .blob_delete(&self.user_id, &self.slug, blob_id)
            .map_err(|e| e.to_string())
    }

    fn assets_get(&self, path: &str) -> Result<Option<(String, Vec<u8>)>, String> {
        let path = path.trim_start_matches('/');
        if path.contains("..") {
            return Err("invalid path".into());
        }
        let bytes = self.asset_store.get_file_exact(&self.asset_key, path);
        Ok(bytes.map(|b| {
            let ct = mime_from_path(path).to_string();
            (ct, b)
        }))
    }

    fn page_meta(&self) -> PageMeta {
        self.meta.clone()
    }
}

fn mime_from_path(p: &str) -> &'static str {
    match p.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

pub fn default_page_data_dir(room_web_dir: &Path) -> PathBuf {
    room_web_dir.join("page-data")
}
