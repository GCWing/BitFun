//! Administrative listing and explicit deletion of GitHub-linked relay records.

use crate::db::{DbPool, UserRow};
use anyhow::{anyhow, Result};

/// Delete an account and all associated data.
pub async fn delete_user(pool: &DbPool, username: &str) -> Result<()> {
    let user = UserRow::find_by_username(pool, username)
        .await?
        .ok_or_else(|| anyhow!("username '{username}' not found"))?;
    UserRow::delete(pool, &user.user_id).await?;
    Ok(())
}

/// List all accounts as `(username, user_id, created_at_timestamp)`.
pub async fn list_users(pool: &DbPool) -> Result<Vec<(String, String, i64)>> {
    UserRow::list_all(pool).await
}
