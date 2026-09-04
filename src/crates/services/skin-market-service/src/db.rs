use crate::error::{SkinMarketError, SkinMarketResult};
use chrono::Utc;
use openbitfun_product_domains::appearance_market::AppearanceMarketUserSummary;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, Row, Sqlite};
use std::path::Path;

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_init.sql");

#[derive(Debug, Clone)]
pub(crate) struct Database {
    pool: Pool<Sqlite>,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalUser {
    pub internal_id: i64,
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
        let database = Self { pool };
        database.migrate().await?;
        Ok(database)
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

    pub(crate) async fn upsert_user(
        &self,
        profile: &AppearanceMarketUserSummary,
    ) -> SkinMarketResult<LocalUser> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO users(github_id, login, avatar_url, created_at, updated_at)
             VALUES(?, ?, ?, ?, ?)
             ON CONFLICT(github_id) DO UPDATE SET
               login = excluded.login,
               avatar_url = excluded.avatar_url,
               updated_at = excluded.updated_at",
        )
        .bind(profile.github_id)
        .bind(&profile.login)
        .bind(&profile.avatar_url)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(SkinMarketError::internal)?;
        let row =
            sqlx::query("SELECT id, github_id, login, avatar_url FROM users WHERE github_id = ?")
                .bind(profile.github_id)
                .fetch_one(&self.pool)
                .await
                .map_err(SkinMarketError::internal)?;
        Ok(LocalUser {
            internal_id: row.get("id"),
        })
    }
}
