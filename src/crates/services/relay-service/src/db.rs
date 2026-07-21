//! SQLite-backed account storage for the relay server.
//!
//! The relay remains zero-knowledge: it stores only password-derived hashes
//! and AES-GCM-wrapped master keys (encrypted client-side). It never holds a
//! plaintext master key and cannot decrypt synced session/settings blobs.

use anyhow::{anyhow, Result};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use std::time::Duration;

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
  device_id    TEXT NOT NULL,
  user_id      TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
  device_name  TEXT,
  public_key   TEXT,
  last_seen_at INTEGER,
  online       INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, device_id)
);
CREATE TABLE IF NOT EXISTS auth_tokens (
  token       TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
  device_id   TEXT NOT NULL,
  token_kind  TEXT NOT NULL DEFAULT 'device',
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL,
  FOREIGN KEY (user_id, device_id) REFERENCES devices(user_id, device_id) ON DELETE CASCADE
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
CREATE TABLE IF NOT EXISTS pages (
  user_id     TEXT NOT NULL REFERENCES users(user_id),
  slug        TEXT NOT NULL,
  visibility  TEXT NOT NULL DEFAULT 'private',
  title       TEXT NOT NULL DEFAULT '',
  file_count  INTEGER NOT NULL DEFAULT 0,
  total_bytes INTEGER NOT NULL DEFAULT 0,
  deployed_version_id TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  PRIMARY KEY (user_id, slug)
);
CREATE INDEX IF NOT EXISTS idx_pages_user ON pages(user_id);
CREATE TABLE IF NOT EXISTS page_versions (
  user_id     TEXT NOT NULL,
  slug        TEXT NOT NULL,
  version_id  TEXT NOT NULL,
  title       TEXT NOT NULL DEFAULT '',
  file_count  INTEGER NOT NULL DEFAULT 0,
  total_bytes INTEGER NOT NULL DEFAULT 0,
  has_worker  INTEGER NOT NULL DEFAULT 0,
  note        TEXT NOT NULL DEFAULT '',
  created_at  INTEGER NOT NULL,
  PRIMARY KEY (user_id, slug, version_id),
  FOREIGN KEY (user_id, slug) REFERENCES pages(user_id, slug) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_page_versions_page ON page_versions(user_id, slug);
CREATE TABLE IF NOT EXISTS page_kv (
  user_id    TEXT NOT NULL,
  slug       TEXT NOT NULL,
  key        TEXT NOT NULL,
  value      TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, slug, key)
);
CREATE TABLE IF NOT EXISTS page_blobs (
  user_id    TEXT NOT NULL,
  slug       TEXT NOT NULL,
  blob_id    TEXT NOT NULL,
  content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
  size_bytes INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, slug, blob_id)
);
"#;

const MIGRATE_PAGES_DEPLOYED_VERSION: &str = r#"
ALTER TABLE pages ADD COLUMN deployed_version_id TEXT;
"#;

const MIGRATE_AUTH_TOKEN_KIND: &str = r#"
ALTER TABLE auth_tokens ADD COLUMN token_kind TEXT NOT NULL DEFAULT 'device';
"#;

/// Open (or create) the SQLite database and ensure the schema exists.
pub async fn connect(db_path: &str) -> Result<DbPool> {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{db_path}"))?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    sqlx::query(SCHEMA).execute(&pool).await?;
    // Older DBs created pages without deployed_version_id.
    let _ = sqlx::query(MIGRATE_PAGES_DEPLOYED_VERSION)
        .execute(&pool)
        .await;
    // Existing tokens predate delegation scopes and are full device tokens.
    if let Err(error) = sqlx::query(MIGRATE_AUTH_TOKEN_KIND).execute(&pool).await {
        if !error.to_string().contains("duplicate column name") {
            return Err(anyhow!("migrate auth token capabilities: {error}"));
        }
    }
    migrate_account_scoped_devices(&pool).await?;
    let now = Utc::now().timestamp();
    sqlx::query("DELETE FROM auth_tokens WHERE expires_at <= ?")
        .bind(now)
        .execute(&pool)
        .await
        .map_err(|e| anyhow!("clean expired auth tokens: {e}"))?;
    // Online presence is owned by the in-memory connection registry. A fresh
    // process has no live sockets, so never carry stale online flags across a
    // restart or crash.
    sqlx::query("UPDATE devices SET online = 0")
        .execute(&pool)
        .await
        .map_err(|e| anyhow!("reset stale device presence: {e}"))?;
    tracing::info!("Account database initialized at {db_path}");
    Ok(pool)
}

/// Migrate the original globally-keyed `devices(device_id)` table to the
/// account-scoped `(user_id, device_id)` identity model. A physical install id
/// is intentionally stable across logins, so it must be legal for two accounts
/// to register the same value without one account reassigning the other's row.
async fn migrate_account_scoped_devices(pool: &DbPool) -> Result<()> {
    let columns = sqlx::query("PRAGMA table_info(devices)")
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!("inspect devices schema: {e}"))?;
    let mut user_pk_order = 0_i64;
    let mut device_pk_order = 0_i64;
    for column in columns {
        let name: String = sqlx::Row::get(&column, "name");
        let pk_order: i64 = sqlx::Row::get(&column, "pk");
        match name.as_str() {
            "user_id" => user_pk_order = pk_order,
            "device_id" => device_pk_order = pk_order,
            _ => {}
        }
    }
    if user_pk_order == 1 && device_pk_order == 2 {
        return Ok(());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| anyhow!("begin account-scoped device migration: {e}"))?;
    sqlx::query("ALTER TABLE auth_tokens RENAME TO auth_tokens_legacy")
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!("rename legacy auth_tokens: {e}"))?;
    sqlx::query("ALTER TABLE devices RENAME TO devices_legacy")
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!("rename legacy devices: {e}"))?;
    sqlx::query(
        "CREATE TABLE devices (\
           device_id TEXT NOT NULL,\
           user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,\
           device_name TEXT, public_key TEXT, last_seen_at INTEGER,\
           online INTEGER NOT NULL DEFAULT 0,\
           PRIMARY KEY (user_id, device_id)\
         )",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow!("create account-scoped devices: {e}"))?;
    sqlx::query(
        "CREATE TABLE auth_tokens (\
           token TEXT PRIMARY KEY,\
           user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,\
           device_id TEXT NOT NULL,\
           token_kind TEXT NOT NULL DEFAULT 'device',\
           created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,\
           FOREIGN KEY (user_id, device_id)\
             REFERENCES devices(user_id, device_id) ON DELETE CASCADE\
         )",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow!("create scoped auth_tokens: {e}"))?;
    sqlx::query(
        "INSERT INTO devices \
         (device_id, user_id, device_name, public_key, last_seen_at, online) \
         SELECT device_id, user_id, device_name, public_key, last_seen_at, online \
         FROM devices_legacy",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow!("copy legacy devices: {e}"))?;
    // Discard any historically inconsistent token whose user_id no longer
    // matches the device row. Such a token could only arise from the old global
    // upsert behavior and must not survive the ownership migration.
    sqlx::query(
        "INSERT INTO auth_tokens \
         (token, user_id, device_id, token_kind, created_at, expires_at) \
         SELECT a.token, a.user_id, a.device_id, a.token_kind, a.created_at, a.expires_at \
         FROM auth_tokens_legacy a \
         INNER JOIN devices_legacy d \
           ON d.user_id = a.user_id AND d.device_id = a.device_id",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow!("copy consistent legacy auth tokens: {e}"))?;
    sqlx::query("DROP TABLE auth_tokens_legacy")
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!("drop legacy auth_tokens: {e}"))?;
    sqlx::query("DROP TABLE devices_legacy")
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!("drop legacy devices: {e}"))?;
    sqlx::query("CREATE INDEX idx_auth_tokens_user ON auth_tokens(user_id)")
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!("recreate auth token index: {e}"))?;
    sqlx::query("CREATE INDEX idx_devices_user ON devices(user_id)")
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!("recreate device index: {e}"))?;
    tx.commit()
        .await
        .map_err(|e| anyhow!("commit account-scoped device migration: {e}"))?;
    Ok(())
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

    pub async fn find_by_user_id(pool: &DbPool, user_id: &str) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT user_id, username, salt, kdf_salt, argon2_params, password_hash, \
             wrapped_master_key, failed_attempts, locked_until, created_at, updated_at \
             FROM users WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("find user by id: {e}"))?;
        Ok(row)
    }

    /// Resolve username for a user id (convenience for page URL construction).
    pub async fn find_by_username_for_user_id(
        pool: &DbPool,
        user_id: &str,
    ) -> Result<Option<String>> {
        Ok(Self::find_by_user_id(pool, user_id)
            .await?
            .map(|u| u.username))
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
    /// sync blobs, pages). Cascading deletes handle FK-linked rows.
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
        sqlx::query("DELETE FROM page_kv WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete page_kv: {e}"))?;
        sqlx::query("DELETE FROM page_blobs WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete page_blobs: {e}"))?;
        sqlx::query("DELETE FROM page_versions WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete page_versions: {e}"))?;
        sqlx::query("DELETE FROM pages WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete pages: {e}"))?;
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
             ON CONFLICT(user_id, device_id) DO UPDATE SET \
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

    pub async fn set_online(
        pool: &DbPool,
        user_id: &str,
        device_id: &str,
        online: bool,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE devices SET online = ?, last_seen_at = ? WHERE user_id = ? AND device_id = ?",
        )
        .bind(online as i64)
        .bind(now)
        .bind(user_id)
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

    /// Delete a device owned by `user_id` and revoke all of its auth tokens.
    ///
    /// Tokens must be removed before the device because `auth_tokens.device_id`
    /// references `devices.device_id`. Keep both operations in one transaction
    /// so a partial deletion cannot leave the account in an inconsistent state.
    pub async fn delete_for_user(pool: &DbPool, user_id: &str, device_id: &str) -> Result<bool> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| anyhow!("begin device deletion: {e}"))?;

        sqlx::query("DELETE FROM auth_tokens WHERE device_id = ? AND user_id = ?")
            .bind(device_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow!("revoke device tokens: {e}"))?;

        let result = sqlx::query("DELETE FROM devices WHERE device_id = ? AND user_id = ?")
            .bind(device_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow!("delete device: {e}"))?;

        tx.commit()
            .await
            .map_err(|e| anyhow!("commit device deletion: {e}"))?;
        Ok(result.rows_affected() > 0)
    }
}

// ── Auth tokens ─────────────────────────────────────────────────────────

const DEVICE_TOKEN_TTL_SECS: i64 = 30 * 24 * 3600;
const DELEGATED_TOKEN_TTL_SECS: i64 = 24 * 3600;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthToken {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
    pub token_kind: String,
    pub created_at: i64,
    pub expires_at: i64,
}

impl AuthToken {
    pub async fn create(pool: &DbPool, user_id: &str, device_id: &str) -> Result<AuthToken> {
        Self::create_with_kind(pool, user_id, device_id, "device").await
    }

    pub async fn create_delegated(
        pool: &DbPool,
        user_id: &str,
        device_id: &str,
    ) -> Result<AuthToken> {
        let now = Utc::now().timestamp();
        if let Some(existing) = sqlx::query_as::<_, AuthToken>(
            "SELECT token, user_id, device_id, token_kind, created_at, expires_at \
             FROM auth_tokens \
             WHERE user_id = ? AND device_id = ? \
               AND token_kind = 'delegated_control' AND expires_at > ? \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(now)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("find reusable delegated token: {e}"))?
        {
            return Ok(existing);
        }
        Self::create_with_kind(pool, user_id, device_id, "delegated_control").await
    }

    async fn create_with_kind(
        pool: &DbPool,
        user_id: &str,
        device_id: &str,
        token_kind: &str,
    ) -> Result<AuthToken> {
        let token = generate_token();
        let now = Utc::now().timestamp();
        let ttl = if token_kind == "delegated_control" {
            DELEGATED_TOKEN_TTL_SECS
        } else {
            DEVICE_TOKEN_TTL_SECS
        };
        let expires_at = now + ttl;
        sqlx::query(
            "INSERT INTO auth_tokens \
             (token, user_id, device_id, token_kind, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&token)
        .bind(user_id)
        .bind(device_id)
        .bind(token_kind)
        .bind(now)
        .bind(expires_at)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("create token: {e}"))?;
        Ok(AuthToken {
            token,
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            token_kind: token_kind.to_string(),
            created_at: now,
            expires_at,
        })
    }

    pub fn is_device_token(&self) -> bool {
        self.token_kind == "device"
    }

    pub fn can_control_devices(&self) -> bool {
        self.is_device_token() || self.token_kind == "delegated_control"
    }

    /// Look up a token; returns None if missing or expired (expired rows are
    /// deleted as a side effect).
    pub async fn find(pool: &DbPool, token: &str) -> Result<Option<AuthToken>> {
        if !is_valid_auth_token(token) {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, AuthToken>(
            "SELECT token, user_id, device_id, token_kind, created_at, expires_at \
             FROM auth_tokens WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("find token: {e}"))?;

        let Some(auth_token) = row else {
            return Ok(None);
        };

        if auth_token.expires_at <= Utc::now().timestamp() {
            let _ = sqlx::query("DELETE FROM auth_tokens WHERE token = ?")
                .bind(token)
                .execute(pool)
                .await;
            return Ok(None);
        }
        Ok(Some(auth_token))
    }

    /// Revoke (delete) all tokens belonging to a specific device.
    pub async fn revoke_by_device(pool: &DbPool, user_id: &str, device_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM auth_tokens WHERE user_id = ? AND device_id = ?")
            .bind(user_id)
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

/// Account tokens are fixed-size lowercase hexadecimal bearer secrets. Check
/// the shape before hitting SQLite so oversized or malformed untrusted input
/// cannot become a database lookup key.
pub fn is_valid_auth_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    pub async fn upsert_with_quota(
        pool: &DbPool,
        user_id: &str,
        session_id: &str,
        encrypted_data: &str,
        nonce: &str,
        version: i64,
        max_sessions: i64,
        max_total_bytes: i64,
    ) -> Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            "INSERT INTO sync_sessions (user_id, session_id, encrypted_data, nonce, version, updated_at, deleted) \
             SELECT ?, ?, ?, ?, ?, ?, 0 \
             WHERE (SELECT COUNT(*) FROM sync_sessions \
                    WHERE user_id = ? AND deleted = 0 AND session_id <> ?) < ? \
               AND (SELECT COALESCE(SUM(LENGTH(encrypted_data)), 0) FROM sync_sessions \
                    WHERE user_id = ? AND deleted = 0 AND session_id <> ?) + ? <= ? \
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
        .bind(user_id)
        .bind(session_id)
        .bind(max_sessions)
        .bind(user_id)
        .bind(session_id)
        .bind(encrypted_data.len() as i64)
        .bind(max_total_bytes)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("upsert sync session: {e}"))?;
        Ok(result.rows_affected() > 0)
    }

    /// Fetch all non-deleted sessions for a user updated after `since_version`.
    pub async fn list_since(
        pool: &DbPool,
        user_id: &str,
        since_version: i64,
    ) -> Result<Vec<SyncSessionRow>> {
        let rows = sqlx::query_as::<_, SyncSessionRow>(
            "SELECT session_id, encrypted_data, nonce, version, updated_at, deleted \
             FROM sync_sessions WHERE user_id = ? AND version > ? AND deleted = 0 \
             ORDER BY version ASC, session_id ASC",
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

// ── Pages (published static sites) ──────────────────────────────────────

/// Visibility levels for a published BitFun Page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageVisibility {
    /// Only the page owner (with their token) can access.
    Private,
    /// Any authenticated user on this relay can access.
    Relay,
    /// Anyone on the internet can access without credentials.
    Public,
}

impl PageVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Relay => "relay",
            Self::Public => "public",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "private" => Some(Self::Private),
            "relay" => Some(Self::Relay),
            "public" => Some(Self::Public),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PageRow {
    pub user_id: String,
    pub slug: String,
    pub visibility: String,
    pub title: String,
    pub file_count: i64,
    pub total_bytes: i64,
    pub deployed_version_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Page row joined with the owning username (for public URL serving).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PageWithUsername {
    pub user_id: String,
    pub username: String,
    pub slug: String,
    pub visibility: String,
    pub title: String,
    pub file_count: i64,
    pub total_bytes: i64,
    pub deployed_version_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PageVersionRow {
    pub user_id: String,
    pub slug: String,
    pub version_id: String,
    pub title: String,
    pub file_count: i64,
    pub total_bytes: i64,
    pub has_worker: i64,
    pub note: String,
    pub created_at: i64,
}

impl PageRow {
    pub fn visibility_enum(&self) -> Option<PageVisibility> {
        PageVisibility::parse(&self.visibility)
    }

    const SELECT_COLS: &'static str = "user_id, slug, visibility, title, file_count, total_bytes, \
             deployed_version_id, created_at, updated_at";

    /// Ensure a page metadata row exists (draft upload / first save).
    pub async fn ensure(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        visibility: PageVisibility,
        title: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO pages \
             (user_id, slug, visibility, title, file_count, total_bytes, deployed_version_id, \
              created_at, updated_at) \
             VALUES (?, ?, ?, ?, 0, 0, NULL, ?, ?) \
             ON CONFLICT(user_id, slug) DO UPDATE SET \
               visibility = excluded.visibility, \
               title = CASE WHEN excluded.title = '' THEN pages.title ELSE excluded.title END, \
               updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(slug)
        .bind(visibility.as_str())
        .bind(title)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("ensure page: {e}"))?;
        Ok(())
    }

    pub async fn get(pool: &DbPool, user_id: &str, slug: &str) -> Result<Option<PageRow>> {
        let row = sqlx::query_as::<_, PageRow>(&format!(
            "SELECT {} FROM pages WHERE user_id = ? AND slug = ?",
            Self::SELECT_COLS
        ))
        .bind(user_id)
        .bind(slug)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("get page: {e}"))?;
        Ok(row)
    }

    pub async fn list_for_user(pool: &DbPool, user_id: &str) -> Result<Vec<PageRow>> {
        let rows = sqlx::query_as::<_, PageRow>(&format!(
            "SELECT {} FROM pages WHERE user_id = ? ORDER BY updated_at DESC",
            Self::SELECT_COLS
        ))
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!("list pages: {e}"))?;
        Ok(rows)
    }

    pub async fn count_for_user(pool: &DbPool, user_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pages WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow!("count pages: {e}"))?;
        Ok(row.0)
    }

    pub async fn update_meta(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        visibility: Option<PageVisibility>,
        title: Option<&str>,
    ) -> Result<bool> {
        let existing = Self::get(pool, user_id, slug).await?;
        let Some(page) = existing else {
            return Ok(false);
        };
        let now = Utc::now().timestamp();
        let new_vis = visibility
            .map(|v| v.as_str().to_string())
            .unwrap_or(page.visibility);
        let new_title = title.map(|t| t.to_string()).unwrap_or(page.title);
        let result = sqlx::query(
            "UPDATE pages SET visibility = ?, title = ?, updated_at = ? \
             WHERE user_id = ? AND slug = ?",
        )
        .bind(&new_vis)
        .bind(&new_title)
        .bind(now)
        .bind(user_id)
        .bind(slug)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("update page meta: {e}"))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_deployed_version(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        version_id: &str,
        file_count: i64,
        total_bytes: i64,
        title: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE pages SET deployed_version_id = ?, file_count = ?, total_bytes = ?, \
             title = CASE WHEN ? = '' THEN title ELSE ? END, updated_at = ? \
             WHERE user_id = ? AND slug = ?",
        )
        .bind(version_id)
        .bind(file_count)
        .bind(total_bytes)
        .bind(title)
        .bind(title)
        .bind(now)
        .bind(user_id)
        .bind(slug)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("set deployed version: {e}"))?;
        Ok(())
    }

    pub async fn delete(pool: &DbPool, user_id: &str, slug: &str) -> Result<bool> {
        sqlx::query("DELETE FROM page_kv WHERE user_id = ? AND slug = ?")
            .bind(user_id)
            .bind(slug)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete page_kv: {e}"))?;
        sqlx::query("DELETE FROM page_blobs WHERE user_id = ? AND slug = ?")
            .bind(user_id)
            .bind(slug)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete page_blobs: {e}"))?;
        sqlx::query("DELETE FROM page_versions WHERE user_id = ? AND slug = ?")
            .bind(user_id)
            .bind(slug)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete page_versions: {e}"))?;
        let result = sqlx::query("DELETE FROM pages WHERE user_id = ? AND slug = ?")
            .bind(user_id)
            .bind(slug)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("delete page: {e}"))?;
        Ok(result.rows_affected() > 0)
    }

    /// Resolve a page by public URL components `(username, slug)`.
    pub async fn get_by_username(
        pool: &DbPool,
        username: &str,
        slug: &str,
    ) -> Result<Option<PageWithUsername>> {
        let row = sqlx::query_as::<_, PageWithUsername>(
            "SELECT p.user_id, u.username, p.slug, p.visibility, p.title, \
             p.file_count, p.total_bytes, p.deployed_version_id, p.created_at, p.updated_at \
             FROM pages p JOIN users u ON u.user_id = p.user_id \
             WHERE u.username = ? AND p.slug = ?",
        )
        .bind(username)
        .bind(slug)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("get page by username: {e}"))?;
        Ok(row)
    }
}

impl PageWithUsername {
    pub fn visibility_enum(&self) -> Option<PageVisibility> {
        PageVisibility::parse(&self.visibility)
    }
}

impl PageVersionRow {
    pub async fn insert(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        version_id: &str,
        title: &str,
        file_count: i64,
        total_bytes: i64,
        has_worker: bool,
        note: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO page_versions \
             (user_id, slug, version_id, title, file_count, total_bytes, has_worker, note, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(slug)
        .bind(version_id)
        .bind(title)
        .bind(file_count)
        .bind(total_bytes)
        .bind(has_worker as i64)
        .bind(note)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("insert page version: {e}"))?;
        Ok(())
    }

    pub async fn get(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        version_id: &str,
    ) -> Result<Option<PageVersionRow>> {
        let row = sqlx::query_as::<_, PageVersionRow>(
            "SELECT user_id, slug, version_id, title, file_count, total_bytes, has_worker, note, created_at \
             FROM page_versions WHERE user_id = ? AND slug = ? AND version_id = ?",
        )
        .bind(user_id)
        .bind(slug)
        .bind(version_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!("get page version: {e}"))?;
        Ok(row)
    }

    pub async fn list_for_page(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
    ) -> Result<Vec<PageVersionRow>> {
        let rows = sqlx::query_as::<_, PageVersionRow>(
            "SELECT user_id, slug, version_id, title, file_count, total_bytes, has_worker, note, created_at \
             FROM page_versions WHERE user_id = ? AND slug = ? ORDER BY created_at DESC",
        )
        .bind(user_id)
        .bind(slug)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!("list page versions: {e}"))?;
        Ok(rows)
    }

    pub async fn count_for_page(pool: &DbPool, user_id: &str, slug: &str) -> Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM page_versions WHERE user_id = ? AND slug = ?")
                .bind(user_id)
                .bind(slug)
                .fetch_one(pool)
                .await
                .map_err(|e| anyhow!("count page versions: {e}"))?;
        Ok(row.0)
    }

    pub async fn delete(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        version_id: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM page_versions WHERE user_id = ? AND slug = ? AND version_id = ?",
        )
        .bind(user_id)
        .bind(slug)
        .bind(version_id)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("delete page version: {e}"))?;
        Ok(result.rows_affected() > 0)
    }
}

/// Page KV helpers (mutable runtime data, keyed by page not version).
pub mod page_kv {
    use super::*;

    pub async fn get(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        key: &str,
    ) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM page_kv WHERE user_id = ? AND slug = ? AND key = ?")
                .bind(user_id)
                .bind(slug)
                .bind(key)
                .fetch_optional(pool)
                .await
                .map_err(|e| anyhow!("page_kv get: {e}"))?;
        Ok(row.map(|r| r.0))
    }

    pub async fn put(
        pool: &DbPool,
        user_id: &str,
        slug: &str,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO page_kv (user_id, slug, key, value, updated_at) VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(user_id, slug, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(slug)
        .bind(key)
        .bind(value)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| anyhow!("page_kv put: {e}"))?;
        Ok(())
    }

    pub async fn delete(pool: &DbPool, user_id: &str, slug: &str, key: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM page_kv WHERE user_id = ? AND slug = ? AND key = ?")
            .bind(user_id)
            .bind(slug)
            .bind(key)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("page_kv delete: {e}"))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_keys(pool: &DbPool, user_id: &str, slug: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT key FROM page_kv WHERE user_id = ? AND slug = ? ORDER BY key")
                .bind(user_id)
                .bind(slug)
                .fetch_all(pool)
                .await
                .map_err(|e| anyhow!("page_kv list: {e}"))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}

/// Legacy single-room asset key (pre-versioning). Used only for one-time migration.
pub fn page_legacy_asset_key(user_id: &str, slug: &str) -> String {
    format!("pages/{user_id}/{slug}")
}

/// Draft upload namespace (mutable until freeze).
pub fn page_draft_asset_key(user_id: &str, slug: &str) -> String {
    format!("pages/{user_id}/{slug}/draft")
}

/// Immutable version namespace.
pub fn page_version_asset_key(user_id: &str, slug: &str, version_id: &str) -> String {
    format!("pages/{user_id}/{slug}/v/{version_id}")
}

/// Generate a new version id (`v` + 12 hex chars).
pub fn new_page_version_id() -> String {
    let bytes: [u8; 6] = rand::random();
    format!(
        "v{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
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
        let found = found.unwrap();
        assert_eq!(found.user_id, "u1");
        assert!(found.is_device_token());

        let delegated = AuthToken::create_delegated(&pool, "u1", "d1")
            .await
            .unwrap();
        let delegated = AuthToken::find(&pool, &delegated.token)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(delegated.token_kind, "delegated_control");
        assert!(!delegated.is_device_token());

        let missing = AuthToken::find(&pool, "nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn the_same_install_device_id_is_isolated_between_accounts() {
        let pool = setup().await;
        UserRow::create(&pool, "u1", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        UserRow::create(&pool, "u2", "bob", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();

        DeviceRow::upsert(&pool, "shared-install", "u1", "Alice laptop", None)
            .await
            .unwrap();
        DeviceRow::upsert(&pool, "shared-install", "u2", "Bob laptop", None)
            .await
            .unwrap();
        let token_u1 = AuthToken::create(&pool, "u1", "shared-install")
            .await
            .unwrap();
        let token_u2 = AuthToken::create(&pool, "u2", "shared-install")
            .await
            .unwrap();

        assert_eq!(DeviceRow::list_by_user(&pool, "u1").await.unwrap().len(), 1);
        assert_eq!(DeviceRow::list_by_user(&pool, "u2").await.unwrap().len(), 1);
        DeviceRow::delete_for_user(&pool, "u1", "shared-install")
            .await
            .unwrap();
        assert!(AuthToken::find(&pool, &token_u1.token)
            .await
            .unwrap()
            .is_none());
        assert!(AuthToken::find(&pool, &token_u2.token)
            .await
            .unwrap()
            .is_some());
        assert_eq!(DeviceRow::list_by_user(&pool, "u2").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn sync_session_upsert_enforces_count_and_byte_quotas_atomically() {
        let pool = setup().await;
        UserRow::create(&pool, "u1", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();

        assert!(
            SyncSessionRow::upsert_with_quota(&pool, "u1", "s1", "1234", "n", 1, 1, 5,)
                .await
                .unwrap()
        );
        assert!(
            SyncSessionRow::upsert_with_quota(&pool, "u1", "s1", "12345", "n", 2, 1, 5,)
                .await
                .unwrap()
        );
        assert!(
            !SyncSessionRow::upsert_with_quota(&pool, "u1", "s2", "1", "n", 1, 1, 5,)
                .await
                .unwrap()
        );
        assert!(
            !SyncSessionRow::upsert_with_quota(&pool, "u1", "s1", "123456", "n", 3, 1, 5,)
                .await
                .unwrap()
        );

        let stored = SyncSessionRow::get(&pool, "u1", "s1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.encrypted_data, "12345");
    }

    #[tokio::test]
    async fn legacy_global_device_schema_is_migrated_without_ambiguous_tokens() {
        let db_path = std::env::temp_dir().join(format!(
            "bitfun-relay-device-migration-{}-{}.db",
            std::process::id(),
            rand::random::<u64>()
        ));
        let db_path_text = db_path.to_string_lossy().to_string();
        let legacy_options = SqliteConnectOptions::from_str(&format!("sqlite://{db_path_text}"))
            .unwrap()
            .create_if_missing(true)
            .foreign_keys(false);
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(legacy_options)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE users (\
               user_id TEXT PRIMARY KEY, username TEXT UNIQUE NOT NULL,\
               salt TEXT NOT NULL, kdf_salt TEXT NOT NULL, argon2_params TEXT NOT NULL,\
               password_hash TEXT NOT NULL, wrapped_master_key TEXT NOT NULL,\
               failed_attempts INTEGER NOT NULL DEFAULT 0, locked_until INTEGER NOT NULL DEFAULT 0,\
               created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL\
             )",
        )
        .execute(&legacy)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE devices (\
               device_id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(user_id),\
               device_name TEXT, public_key TEXT, last_seen_at INTEGER,\
               online INTEGER NOT NULL DEFAULT 0\
             )",
        )
        .execute(&legacy)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE auth_tokens (\
               token TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(user_id),\
               device_id TEXT NOT NULL REFERENCES devices(device_id),\
               created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL\
             )",
        )
        .execute(&legacy)
        .await
        .unwrap();
        for (user_id, username) in [("u1", "alice"), ("u2", "bob")] {
            sqlx::query(
                "INSERT INTO users \
                 (user_id, username, salt, kdf_salt, argon2_params, password_hash,\
                  wrapped_master_key, created_at, updated_at) \
                 VALUES (?, ?, 's', 'ks', '{}', 'hash', 'wmk', 1, 1)",
            )
            .bind(user_id)
            .bind(username)
            .execute(&legacy)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO devices (device_id, user_id, device_name, online) \
             VALUES ('shared-install', 'u2', 'Bob laptop', 1)",
        )
        .execute(&legacy)
        .await
        .unwrap();
        let consistent_token = "a".repeat(64);
        let inconsistent_token = "b".repeat(64);
        for (token, user_id) in [(&consistent_token, "u2"), (&inconsistent_token, "u1")] {
            sqlx::query(
                "INSERT INTO auth_tokens \
                 (token, user_id, device_id, created_at, expires_at) \
                 VALUES (?, ?, 'shared-install', 1, 4102444800)",
            )
            .bind(token)
            .bind(user_id)
            .execute(&legacy)
            .await
            .unwrap();
        }
        legacy.close().await;

        let migrated = connect(&db_path_text).await.unwrap();
        assert!(AuthToken::find(&migrated, &consistent_token)
            .await
            .unwrap()
            .is_some());
        assert!(AuthToken::find(&migrated, &inconsistent_token)
            .await
            .unwrap()
            .is_none());
        DeviceRow::upsert(&migrated, "shared-install", "u1", "Alice laptop", None)
            .await
            .unwrap();
        assert_eq!(
            DeviceRow::list_by_user(&migrated, "u1")
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            DeviceRow::list_by_user(&migrated, "u2")
                .await
                .unwrap()
                .len(),
            1
        );
        migrated.close().await;
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn page_ensure_version_deploy_and_resolve() {
        let pool = setup().await;
        UserRow::create(&pool, "u1", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        PageRow::ensure(&pool, "u1", "my-site", PageVisibility::Public, "My Site")
            .await
            .unwrap();
        PageVersionRow::insert(
            &pool, "u1", "my-site", "vabc", "My Site", 3, 1024, false, "first",
        )
        .await
        .unwrap();
        PageRow::set_deployed_version(&pool, "u1", "my-site", "vabc", 3, 1024, "My Site")
            .await
            .unwrap();

        let listed = PageRow::list_for_user(&pool, "u1").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].deployed_version_id.as_deref(), Some("vabc"));

        let versions = PageVersionRow::list_for_page(&pool, "u1", "my-site")
            .await
            .unwrap();
        assert_eq!(versions.len(), 1);

        let by_name = PageRow::get_by_username(&pool, "alice", "my-site")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_name.user_id, "u1");
        assert_eq!(by_name.deployed_version_id.as_deref(), Some("vabc"));

        page_kv::put(&pool, "u1", "my-site", "k", "v")
            .await
            .unwrap();
        assert_eq!(
            page_kv::get(&pool, "u1", "my-site", "k")
                .await
                .unwrap()
                .as_deref(),
            Some("v")
        );

        assert!(PageRow::delete(&pool, "u1", "my-site").await.unwrap());
        assert!(PageRow::get(&pool, "u1", "my-site")
            .await
            .unwrap()
            .is_none());
    }
}
