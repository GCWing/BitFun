//! SQLite-backed account storage for the relay server.
//!
//! The relay remains zero-knowledge: it stores only password-derived hashes
//! and AES-GCM-wrapped master keys (encrypted client-side). It never holds a
//! plaintext master key and cannot decrypt synced session/settings blobs.

use anyhow::{anyhow, Result};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

pub type DbPool = Pool<Sqlite>;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
  user_id            TEXT PRIMARY KEY,
  username           TEXT UNIQUE NOT NULL,
  salt               TEXT NOT NULL,
  kdf_salt           TEXT NOT NULL,
  argon2_params      TEXT NOT NULL,
  password_hash      TEXT NOT NULL,
  wrapped_master_key TEXT NOT NULL,
  failed_attempts    INTEGER NOT NULL DEFAULT 0,
  locked_until       INTEGER NOT NULL DEFAULT 0,
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS devices (
  device_id    TEXT PRIMARY KEY,
  user_id      TEXT NOT NULL REFERENCES users(user_id),
  device_name  TEXT,
  public_key   TEXT,
  last_seen_at INTEGER,
  online       INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS auth_tokens (
  token       TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL REFERENCES users(user_id),
  device_id   TEXT NOT NULL REFERENCES devices(device_id),
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_auth_tokens_user ON auth_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_devices_user ON devices(user_id);
CREATE TABLE IF NOT EXISTS sync_sessions (
  user_id        TEXT NOT NULL REFERENCES users(user_id),
  session_id     TEXT NOT NULL,
  encrypted_data TEXT NOT NULL,
  nonce          TEXT NOT NULL,
  version        INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  deleted        INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, session_id)
);
CREATE INDEX IF NOT EXISTS idx_sync_sessions_user ON sync_sessions(user_id);
CREATE TABLE IF NOT EXISTS sync_settings (
  user_id        TEXT PRIMARY KEY REFERENCES users(user_id),
  encrypted_data TEXT NOT NULL,
  nonce          TEXT NOT NULL,
  version        INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);
"#;

/// Open (or create) the SQLite database and ensure the schema exists.
pub async fn connect(db_path: &str) -> Result<DbPool> {
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{db_path}"))?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    sqlx::query(SCHEMA).execute(&pool).await?;
    tracing::info!("Account database initialized at {db_path}");
    Ok(pool)
}

// ── Users ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub user_id: String,
    pub username: String,
    pub salt: String,
    pub kdf_salt: String,
    pub argon2_params: String,
    pub password_hash: String,
    pub wrapped_master_key: String,
    pub failed_attempts: i64,
    pub locked_until: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl UserRow {
    /// Insert a new user row. Not exposed via HTTP — accounts are provisioned
    /// out-of-band (e.g. an admin import tool) so the relay never sees a
    /// password. Kept as a DB primitive for that future tooling.
    #[allow(dead_code)]
    pub async fn create(
        pool: &DbPool,
        user_id: &str,
        username: &str,
        salt: &str,
        kdf_salt: &str,
        argon2_params: &str,
        password_hash: &str,
        wrapped_master_key: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO users \
             (user_id, username, salt, kdf_salt, argon2_params, password_hash, \
              wrapped_master_key, failed_attempts, locked_until, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, ?, ?)",
        )
        .bind(user_id)
        .bind(username)
        .bind(salt)
        .bind(kdf_salt)
        .bind(argon2_params)
        .bind(password_hash)
        .bind(wrapped_master_key)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("create user: {e}"))?;
        Ok(())
    }

    pub async fn find_by_username(pool: &DbPool, username: &str) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT user_id, username, salt, kdf_salt, argon2_params, password_hash, \
             wrapped_master_key, failed_attempts, locked_until, created_at, updated_at \
             FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("find user: {e}"))?;
        Ok(row)
    }

    /// Rename a user. Fails if the new username already exists.
    pub async fn rename(pool: &DbPool, user_id: &str, new_username: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query("UPDATE users SET username = ?, updated_at = ? WHERE user_id = ?")
            .bind(new_username)
            .bind(now)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("rename user: {e}"))?;
        Ok(())
    }

    /// List all usernames (admin tooling). Returns `(username, created_at)`.
    pub async fn list_all(pool: &DbPool) -> Result<Vec<(String, String, i64)>> {
        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT username, user_id, created_at FROM users ORDER BY created_at",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!("list users: {e}"))?;
        Ok(rows)
    }

    /// Update credentials for an existing user (admin password reset).
    /// Replaces salt, kdf_salt, password_hash, and wrapped_master_key.
    pub async fn update_credentials(
        pool: &DbPool,
        user_id: &str,
        salt: &str,
        kdf_salt: &str,
        argon2_params: &str,
        password_hash: &str,
        wrapped_master_key: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE users SET salt = ?, kdf_salt = ?, argon2_params = ?, \
             password_hash = ?, wrapped_master_key = ?, failed_attempts = 0, \
             locked_until = 0, updated_at = ? WHERE user_id = ?",
        )
        .bind(salt)
        .bind(kdf_salt)
        .bind(argon2_params)
        .bind(password_hash)
        .bind(wrapped_master_key)
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("update credentials: {e}"))?;
        Ok(())
    }

    /// Permanently delete a user and all associated data (devices, tokens,
    /// sync blobs).  Cascading deletes handle FK-linked rows.
    pub async fn delete(pool: &DbPool, user_id: &str) -> Result<()> {
        // Clean up sync tables first (no FK cascade configured on them).
        sqlx::query("DELETE FROM sync_sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete sync_sessions: {e}"))?;
        sqlx::query("DELETE FROM sync_settings WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete sync_settings: {e}"))?;
        // auth_tokens and devices have REFERENCES users(user_id) but SQLite
        // doesn't cascade by default, so clean them up explicitly.
        sqlx::query("DELETE FROM auth_tokens WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete auth_tokens: {e}"))?;
        sqlx::query("DELETE FROM devices WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete devices: {e}"))?;
        sqlx::query("DELETE FROM users WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete user: {e}"))?;
        Ok(())
    }

    /// Increment the failed-attempt counter and apply an exponential-backoff
    /// lockout once the threshold is reached. Returns the new `locked_until`
    /// timestamp (0 when not locked).
    pub async fn record_failed_attempt(pool: &DbPool, user_id: &str) -> Result<i64> {
        let now = Utc::now().timestamp();
        let row = sqlx::query("SELECT failed_attempts FROM users WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow!("fetch attempts: {e}"))?;
        let current: i64 = sqlx::Row::get(&row, "failed_attempts");
        let new_count = current + 1;
        let locked_until = lockout_until(new_count, now);
        sqlx::query(
            "UPDATE users SET failed_attempts = ?, locked_until = ?, updated_at = ? \
             WHERE user_id = ?",
        )
        .bind(new_count)
        .bind(locked_until)
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("update attempts: {e}"))?;
        Ok(locked_until)
    }

    pub async fn reset_failed_attempts(pool: &DbPool, user_id: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE users SET failed_attempts = 0, locked_until = 0, updated_at = ? \
             WHERE user_id = ?",
        )
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("reset attempts: {e}"))?;
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.locked_until > Utc::now().timestamp()
    }
}

/// Exponential backoff lockout schedule.
///
/// `attempts` is the count *after* the latest failure. Locking kicks in at 5
/// failures and grows: 1m → 5m → 15m → 60m (capped).
fn lockout_until(attempts: i64, now: i64) -> i64 {
    if attempts < 5 {
        return 0;
    }
    let level = (attempts - 4).min(4) as i64;
    let secs = match level {
        1 => 60,
        2 => 300,
        3 => 900,
        _ => 3600,
    };
    now + secs
}

// ── Devices ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeviceRow {
    pub device_id: String,
    pub user_id: String,
    pub device_name: Option<String>,
    pub public_key: Option<String>,
    pub last_seen_at: Option<i64>,
    pub online: i64,
}

impl DeviceRow {
    pub async fn upsert(
        pool: &DbPool,
        device_id: &str,
        user_id: &str,
        device_name: &str,
        public_key: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO devices (device_id, user_id, device_name, public_key, last_seen_at, online) \
             VALUES (?, ?, ?, ?, ?, 1) \
             ON CONFLICT(device_id) DO UPDATE SET \
               user_id = excluded.user_id, \
               device_name = excluded.device_name, \
               public_key = excluded.public_key, \
               last_seen_at = excluded.last_seen_at, \
               online = 1",
        )
        .bind(device_id)
        .bind(user_id)
        .bind(device_name)
        .bind(public_key)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("upsert device: {e}"))?;
        Ok(())
    }

    pub async fn set_online(pool: &DbPool, device_id: &str, online: bool) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query("UPDATE devices SET online = ?, last_seen_at = ? WHERE device_id = ?")
            .bind(online as i64)
            .bind(now)
            .bind(device_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("set device online: {e}"))?;
        Ok(())
    }

    pub async fn list_by_user(pool: &DbPool, user_id: &str) -> Result<Vec<DeviceRow>> {
        let rows = sqlx::query_as::<_, DeviceRow>(
            "SELECT device_id, user_id, device_name, public_key, last_seen_at, online \
             FROM devices WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!("list devices: {e}"))?;
        Ok(rows)
    }

    pub async fn delete(pool: &DbPool, device_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM devices WHERE device_id = ?")
            .bind(device_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete device: {e}"))?;
        Ok(())
    }
}

// ── Auth tokens ─────────────────────────────────────────────────────────

const TOKEN_TTL_SECS: i64 = 30 * 24 * 3600;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthToken {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
    pub created_at: i64,
    pub expires_at: i64,
}

impl AuthToken {
    pub async fn create(pool: &DbPool, user_id: &str, device_id: &str) -> Result<AuthToken> {
        let token = generate_token();
        let now = Utc::now().timestamp();
        let expires_at = now + TOKEN_TTL_SECS;
        sqlx::query(
            "INSERT INTO auth_tokens (token, user_id, device_id, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&token)
        .bind(user_id)
        .bind(device_id)
        .bind(now)
        .bind(expires_at)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("create token: {e}"))?;
        Ok(AuthToken {
            token,
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            created_at: now,
            expires_at,
        })
    }

    /// Look up a token; returns None if missing or expired (expired rows are
    /// deleted as a side effect).
    pub async fn find(pool: &DbPool, token: &str) -> Result<Option<AuthToken>> {
        let row = sqlx::query_as::<_, AuthToken>(
            "SELECT token, user_id, device_id, created_at, expires_at \
             FROM auth_tokens WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("find token: {e}"))?;

        let Some(auth_token) = row else {
            return Ok(None);
        };

        if auth_token.expires_at < Utc::now().timestamp() {
            let _ = sqlx::query("DELETE FROM auth_tokens WHERE token = ?")
                .bind(token)
                .execute(pool)
                .await;
            return Ok(None);
        }
        Ok(Some(auth_token))
    }

    /// Revoke (delete) all tokens belonging to a specific device.
    pub async fn revoke_by_device(pool: &DbPool, device_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM auth_tokens WHERE device_id = ?")
            .bind(device_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("revoke tokens by device: {e}"))?;
        Ok(())
    }
}

fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── Sync sessions (encrypted blobs, server never decrypts) ─────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SyncSessionRow {
    pub session_id: String,
    pub encrypted_data: String,
    pub nonce: String,
    pub version: i64,
    pub updated_at: i64,
    pub deleted: i64,
}

impl SyncSessionRow {
    /// Upsert an encrypted session blob. Last-writer-wins via version.
    pub async fn upsert(
        pool: &DbPool,
        user_id: &str,
        session_id: &str,
        encrypted_data: &str,
        nonce: &str,
        version: i64,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO sync_sessions (user_id, session_id, encrypted_data, nonce, version, updated_at, deleted) \
             VALUES (?, ?, ?, ?, ?, ?, 0) \
             ON CONFLICT(user_id, session_id) DO UPDATE SET \
               encrypted_data = excluded.encrypted_data, \
               nonce = excluded.nonce, \
               version = excluded.version, \
               updated_at = excluded.updated_at, \
               deleted = 0",
        )
        .bind(user_id)
        .bind(session_id)
        .bind(encrypted_data)
        .bind(nonce)
        .bind(version)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("upsert sync session: {e}"))?;
        Ok(())
    }

    /// Fetch all non-deleted sessions for a user updated after `since_version`.
    pub async fn list_since(
        pool: &DbPool,
        user_id: &str,
        since_version: i64,
    ) -> Result<Vec<SyncSessionRow>> {
        let rows = sqlx::query_as::<_, SyncSessionRow>(
            "SELECT session_id, encrypted_data, nonce, version, updated_at, deleted \
             FROM sync_sessions WHERE user_id = ? AND version > ? AND deleted = 0",
        )
        .bind(user_id)
        .bind(since_version)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!("list sync sessions: {e}"))?;
        Ok(rows)
    }

    /// Soft-delete a session (tombstone for syncing deletions across devices).
    /// Bumps `version` so incremental-sync consumers pick up the deletion.
    pub async fn delete(pool: &DbPool, user_id: &str, session_id: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE sync_sessions SET deleted = 1, version = ?, updated_at = ? \
             WHERE user_id = ? AND session_id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(user_id)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("delete sync session: {e}"))?;
        Ok(())
    }

    /// Fetch one non-deleted session blob by id.
    pub async fn get(
        pool: &DbPool,
        user_id: &str,
        session_id: &str,
    ) -> Result<Option<SyncSessionRow>> {
        let row = sqlx::query_as::<_, SyncSessionRow>(
            "SELECT session_id, encrypted_data, nonce, version, updated_at, deleted \
             FROM sync_sessions \
             WHERE user_id = ? AND session_id = ? AND deleted = 0",
        )
        .bind(user_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("get sync session: {e}"))?;
        Ok(row)
    }
}

// ── Sync settings (single encrypted blob per user) ──────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SyncSettingsRow {
    pub encrypted_data: String,
    pub nonce: String,
    pub version: i64,
    pub updated_at: i64,
}

impl SyncSettingsRow {
    pub async fn upsert(
        pool: &DbPool,
        user_id: &str,
        encrypted_data: &str,
        nonce: &str,
        version: i64,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO sync_settings (user_id, encrypted_data, nonce, version, updated_at) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(user_id) DO UPDATE SET \
               encrypted_data = excluded.encrypted_data, \
               nonce = excluded.nonce, \
               version = excluded.version, \
               updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(encrypted_data)
        .bind(nonce)
        .bind(version)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("upsert sync settings: {e}"))?;
        Ok(())
    }

    pub async fn get(pool: &DbPool, user_id: &str) -> Result<Option<SyncSettingsRow>> {
        let row = sqlx::query_as::<_, SyncSettingsRow>(
            "SELECT encrypted_data, nonce, version, updated_at FROM sync_settings WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("get sync settings: {e}"))?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> DbPool {
        let pool = connect(":memory:").await.unwrap();
        // sqlx in-memory needs a single connection to share state
        pool
    }

    #[test]
    fn lockout_schedule() {
        let now = 1000;
        assert_eq!(lockout_until(4, now), 0);
        assert_eq!(lockout_until(5, now), now + 60);
        assert_eq!(lockout_until(6, now), now + 300);
        assert_eq!(lockout_until(7, now), now + 900);
        assert_eq!(lockout_until(8, now), now + 3600);
        assert_eq!(lockout_until(100, now), now + 3600);
    }

    #[tokio::test]
    async fn failed_attempts_lock_account() {
        let pool = setup().await;
        UserRow::create(&pool, "u1", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        for _ in 0..4 {
            let lock = UserRow::record_failed_attempt(&pool, "u1").await.unwrap();
            assert_eq!(lock, 0, "not locked before 5 failures");
        }
        let lock = UserRow::record_failed_attempt(&pool, "u1").await.unwrap();
        assert!(lock > 0, "locked at 5 failures");

        let user = UserRow::find_by_username(&pool, "alice")
            .await
            .unwrap()
            .unwrap();
        assert!(user.is_locked());

        UserRow::reset_failed_attempts(&pool, "u1").await.unwrap();
        let user = UserRow::find_by_username(&pool, "alice")
            .await
            .unwrap()
            .unwrap();
        assert!(!user.is_locked());
    }

    #[tokio::test]
    async fn token_create_and_find() {
        let pool = setup().await;
        UserRow::create(&pool, "u1", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&pool, "d1", "u1", "Laptop", None)
            .await
            .unwrap();
        let tok = AuthToken::create(&pool, "u1", "d1").await.unwrap();
        let found = AuthToken::find(&pool, &tok.token).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().user_id, "u1");

        let missing = AuthToken::find(&pool, "nonexistent").await.unwrap();
        assert!(missing.is_none());
    }
}
