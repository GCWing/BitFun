use crate::error::{MarketError, MarketResult};
use chrono::Utc;
use openbitfun_product_domains::miniapp::market::MarketUserSummary;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, Row, Sqlite};
use std::path::Path;

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_init.sql");

#[derive(Debug, Clone)]
pub(crate) struct Database {
    pool: Pool<Sqlite>,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedUser {
    pub internal_id: i64,
    pub profile: MarketUserSummary,
}

impl Database {
    pub(crate) async fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;
        let db = Self { pool };
        db.migrate().await?;
        db.cleanup_expired_auth().await?;
        Ok(db)
    }

    pub(crate) fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;
        let applied: Option<(i64,)> =
            sqlx::query_as("SELECT version FROM schema_migrations WHERE version = 1")
                .fetch_optional(&self.pool)
                .await?;
        if applied.is_none() {
            let mut transaction = self.pool.begin().await?;
            sqlx::raw_sql(INITIAL_MIGRATION)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("INSERT INTO schema_migrations(version, applied_at) VALUES(1, ?)")
                .bind(Utc::now().timestamp())
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
        }
        Ok(())
    }

    pub(crate) async fn cleanup_expired_auth(&self) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query("DELETE FROM web_sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM oauth_flows WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "UPDATE desktop_auth_transactions
             SET status = 'expired', updated_at = ?
             WHERE expires_at <= ? AND status IN ('pending', 'authorized')",
        )
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn upsert_github_user(
        &self,
        github_id: i64,
        login: &str,
        avatar_url: &str,
    ) -> MarketResult<AuthenticatedUser> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO users(github_id, login, avatar_url, created_at, updated_at)
             VALUES(?, ?, ?, ?, ?)
             ON CONFLICT(github_id) DO UPDATE
             SET login = excluded.login, avatar_url = excluded.avatar_url, updated_at = excluded.updated_at",
        )
        .bind(github_id)
        .bind(login)
        .bind(avatar_url)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(MarketError::internal)?;
        self.user_by_github_id(github_id)
            .await?
            .ok_or_else(|| MarketError::internal("GitHub user disappeared after upsert"))
    }

    pub(crate) async fn user_by_github_id(
        &self,
        github_id: i64,
    ) -> MarketResult<Option<AuthenticatedUser>> {
        let row =
            sqlx::query("SELECT id, github_id, login, avatar_url FROM users WHERE github_id = ?")
                .bind(github_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(MarketError::internal)?;
        Ok(row.map(user_from_row))
    }

    pub(crate) async fn create_web_session(
        &self,
        user_id: i64,
        token: &str,
        csrf_token: &str,
        expires_at: i64,
    ) -> MarketResult<()> {
        sqlx::query(
            "INSERT INTO web_sessions(token_hash, user_id, csrf_hash, expires_at, created_at)
             VALUES(?, ?, ?, ?, ?)",
        )
        .bind(token_hash(token))
        .bind(user_id)
        .bind(token_hash(csrf_token))
        .bind(expires_at)
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(MarketError::internal)?;
        Ok(())
    }

    pub(crate) async fn web_session_user(
        &self,
        token: &str,
    ) -> MarketResult<Option<(AuthenticatedUser, String, i64)>> {
        let row = sqlx::query(
            "SELECT u.id, u.github_id, u.login, u.avatar_url, s.csrf_hash, s.expires_at
             FROM web_sessions s
             JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = ? AND s.expires_at > ?",
        )
        .bind(token_hash(token))
        .bind(Utc::now().timestamp())
        .fetch_optional(&self.pool)
        .await
        .map_err(MarketError::internal)?;
        Ok(row.map(|row| {
            let csrf_hash = row.get::<String, _>("csrf_hash");
            let expires_at = row.get::<i64, _>("expires_at");
            (user_from_row(row), csrf_hash, expires_at)
        }))
    }

    pub(crate) async fn delete_web_session(&self, token: &str) -> MarketResult<()> {
        sqlx::query("DELETE FROM web_sessions WHERE token_hash = ?")
            .bind(token_hash(token))
            .execute(&self.pool)
            .await
            .map_err(MarketError::internal)?;
        Ok(())
    }

    pub(crate) async fn create_api_token(
        &self,
        user_id: i64,
        token: &str,
        token_type: &str,
        family_id: &str,
        expires_at: i64,
    ) -> MarketResult<()> {
        sqlx::query(
            "INSERT INTO api_tokens(token_hash, user_id, token_type, family_id, expires_at, created_at)
             VALUES(?, ?, ?, ?, ?, ?)",
        )
        .bind(token_hash(token))
        .bind(user_id)
        .bind(token_type)
        .bind(family_id)
        .bind(expires_at)
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .map_err(MarketError::internal)?;
        Ok(())
    }

    pub(crate) async fn api_token_user(
        &self,
        token: &str,
        token_type: &str,
    ) -> MarketResult<Option<(AuthenticatedUser, String)>> {
        let row = sqlx::query(
            "SELECT u.id, u.github_id, u.login, u.avatar_url, t.family_id
             FROM api_tokens t
             JOIN users u ON u.id = t.user_id
             WHERE t.token_hash = ? AND t.token_type = ? AND t.expires_at > ?
               AND t.revoked_at IS NULL",
        )
        .bind(token_hash(token))
        .bind(token_type)
        .bind(Utc::now().timestamp())
        .fetch_optional(&self.pool)
        .await
        .map_err(MarketError::internal)?;
        Ok(row.map(|row| {
            let family_id = row.get::<String, _>("family_id");
            (user_from_row(row), family_id)
        }))
    }

    pub(crate) async fn revoke_token_family(&self, family_id: &str) -> MarketResult<()> {
        sqlx::query(
            "UPDATE api_tokens SET revoked_at = ? WHERE family_id = ? AND revoked_at IS NULL",
        )
        .bind(Utc::now().timestamp())
        .bind(family_id)
        .execute(&self.pool)
        .await
        .map_err(MarketError::internal)?;
        Ok(())
    }
}

fn user_from_row(row: sqlx::sqlite::SqliteRow) -> AuthenticatedUser {
    AuthenticatedUser {
        internal_id: row.get("id"),
        profile: MarketUserSummary {
            github_id: row.get("github_id"),
            login: row.get("login"),
            avatar_url: row.get("avatar_url"),
        },
    }
}

pub(crate) fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
