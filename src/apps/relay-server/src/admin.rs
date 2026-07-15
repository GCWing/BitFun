//! Admin account provisioning module.
//!
//! Provides the cryptographic primitives needed to create/update user accounts
//! out-of-band. The relay server itself never uses this code at runtime — it
//! is consumed only by the `relay-admin` CLI binary.
//!
//! The key-derivation logic mirrors `account.rs` in `services-integrations`
//! exactly: same Argon2id parameters, same salt lengths, same AES-256-GCM
//! wrapping format. This ensures accounts provisioned here can log in via the
//! normal client flow.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::db::{DbPool, UserRow};

const MASTER_KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Argon2id parameters. Must match `KdfParams::default()` in `account.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminKdfParams {
    pub m: u32,
    pub t: u32,
    pub p: u32,
}

impl Default for AdminKdfParams {
    fn default() -> Self {
        Self {
            m: 65536,
            t: 3,
            p: 4,
        }
    }
}

impl AdminKdfParams {
    fn build(&self) -> Result<Argon2<'static>> {
        let params = Params::new(self.m, self.t, self.p, Some(MASTER_KEY_LEN))
            .map_err(|e| anyhow!("invalid argon2 params: {e}"))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| anyhow!("serialize kdf params: {e}"))
    }
}

/// The complete set of values needed to insert/update a user row.
pub struct ProvisionedAccount {
    pub user_id: String,
    pub salt: String,
    pub kdf_salt: String,
    pub argon2_params: String,
    pub password_hash: String,
    pub wrapped_master_key: String,
}

/// Derive all cryptographic artifacts for a username + password.
///
/// This is the provisioning primitive: it generates two random salts, derives
/// the KEK (to wrap a new random master key) and the server-verifiable password
/// hash, and returns everything the DB needs. The plaintext password is not
/// retained after this function returns.
pub fn provision(_username: &str, password: &str) -> Result<ProvisionedAccount> {
    let params = AdminKdfParams::default();
    let argon2 = params.build()?;

    // Two independent random salts.
    let mut salt = [0u8; SALT_LEN];
    let mut kdf_salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut kdf_salt);

    // 1. Derive KEK from password + salt.
    let mut kek = [0u8; MASTER_KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut kek)
        .map_err(|e| anyhow!("derive kek: {e}"))?;

    // 2. Generate a random master key and wrap it with the KEK.
    let mut master_key = [0u8; MASTER_KEY_LEN];
    OsRng.fill_bytes(&mut master_key);

    let cipher = Aes256Gcm::new_from_slice(&kek).map_err(|e| anyhow!("aes init: {e}"))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let wrapped_ct = cipher
        .encrypt(nonce, master_key.as_slice())
        .map_err(|e| anyhow!("wrap master key: {e}"))?;

    // Packed format: "ct_b64.nonce_b64" — matches account.rs `pack_wrapped`.
    let wrapped_master_key = format!(
        "{}.{}",
        BASE64.encode(&wrapped_ct),
        BASE64.encode(&nonce_bytes)
    );

    // 3. Derive the server-verifiable password hash (separate salt).
    let mut pwd_hash = [0u8; MASTER_KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), &kdf_salt, &mut pwd_hash)
        .map_err(|e| anyhow!("derive password hash: {e}"))?;

    Ok(ProvisionedAccount {
        user_id: uuid::Uuid::new_v4().to_string(),
        salt: BASE64.encode(salt),
        kdf_salt: BASE64.encode(kdf_salt),
        argon2_params: params.to_json()?,
        password_hash: BASE64.encode(pwd_hash),
        wrapped_master_key,
    })
}

// ── DB operations ──────────────────────────────────────────────────────────

/// Create a new account. Fails if the username already exists.
pub async fn add_user(pool: &DbPool, username: &str, password: &str) -> Result<String> {
    if UserRow::find_by_username(pool, username).await?.is_some() {
        return Err(anyhow!("username '{username}' already exists"));
    }
    let acct = provision(username, password)?;
    UserRow::create(
        pool,
        &acct.user_id,
        username,
        &acct.salt,
        &acct.kdf_salt,
        &acct.argon2_params,
        &acct.password_hash,
        &acct.wrapped_master_key,
    )
    .await?;
    Ok(acct.user_id)
}

/// Reset the password for an existing account. Generates new salts and a new
/// master key — **all existing synced data (encrypted with the old master key)
/// becomes unreadable**. The user must re-sync after logging in.
pub async fn reset_password(pool: &DbPool, username: &str, password: &str) -> Result<()> {
    let user = UserRow::find_by_username(pool, username)
        .await?
        .ok_or_else(|| anyhow!("username '{username}' not found"))?;
    let acct = provision(username, password)?;
    UserRow::update_credentials(
        pool,
        &user.user_id,
        &acct.salt,
        &acct.kdf_salt,
        &acct.argon2_params,
        &acct.password_hash,
        &acct.wrapped_master_key,
    )
    .await?;
    // Wipe old sync data since it was encrypted with the old master key.
    let _ = sqlx::query("DELETE FROM sync_sessions WHERE user_id = ?")
        .bind(&user.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM sync_settings WHERE user_id = ?")
        .bind(&user.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM auth_tokens WHERE user_id = ?")
        .bind(&user.user_id)
        .execute(pool)
        .await;
    Ok(())
}

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

/// Rename an existing account. Fails if the new username is already taken.
/// The user_id and all credentials remain unchanged.
pub async fn rename_user(pool: &DbPool, old_username: &str, new_username: &str) -> Result<()> {
    let user = UserRow::find_by_username(pool, old_username)
        .await?
        .ok_or_else(|| anyhow!("username '{old_username}' not found"))?;
    if UserRow::find_by_username(pool, new_username)
        .await?
        .is_some()
    {
        return Err(anyhow!("username '{new_username}' already exists"));
    }
    UserRow::rename(pool, &user.user_id, new_username).await
}
