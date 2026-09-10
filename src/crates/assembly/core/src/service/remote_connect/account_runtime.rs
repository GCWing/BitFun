//! Shared account runtime owner for product Hosts.
//!
//! The runtime owns account identity transitions, persisted credentials,
//! and authenticated device connections. Product Hosts
//! inject device-routing and background-owner lifecycle effects without
//! exposing App Server wire DTOs to this owner.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::sync::{Mutex, MutexGuard, RwLock};

use openbitfun_services_integrations::remote_connect::account::{AccountClient, AccountSession};
use openbitfun_services_integrations::remote_connect::{session_store, DeviceIdentity};

use super::validate_relay_base_url;

#[derive(Debug, Clone)]
struct AccountContextState {
    session: AccountSession,
    relay_url: String,
}

#[derive(Debug, Clone)]
pub struct AccountRoutingStartRequest {
    pub session: AccountSession,
    pub relay_url: String,
    pub device_name: String,
    pub account_generation: u64,
}

#[derive(Debug)]
pub struct BackgroundRoutingOwnerRetirementError {
    pub error: anyhow::Error,
    pub owner_may_exit: bool,
}

#[async_trait]
pub trait AccountRuntimeHost: Send + Sync {
    async fn retire_background_routing_owner(
        &self,
    ) -> std::result::Result<bool, BackgroundRoutingOwnerRetirementError>;

    fn background_routing_owner_is_running(&self) -> bool;

    fn request_background_routing_owner_shutdown(&self) -> bool;

    async fn start_device_routing(&self, request: AccountRoutingStartRequest) -> Result<()>;

    async fn stop_device_routing(&self);
}

pub enum AccountLoginProgress {
    Authorization(openbitfun_product_domains::account::GitHubAuthStart),
    Waiting,
    Complete(AccountLoginResult),
}

#[derive(Debug, Clone)]
pub struct AccountLoginResult {
    pub user_id: String,
    pub relay_url: String,
    pub routing_owner_replaced: bool,
    pub routing_connected: bool,
    pub routing_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub user_id: String,
    pub relay_url: String,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Clone)]
pub struct AccountDevice {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
}

#[derive(Debug, Clone)]
pub struct AccountSnapshot {
    pub logged_in: bool,
    pub info: Option<AccountInfo>,
    pub devices: Vec<AccountDevice>,
}

pub struct AccountRuntime {
    host: Arc<dyn AccountRuntimeHost>,
    account_context: RwLock<Option<AccountContextState>>,
    account_context_generation: AtomicU64,
    account_context_transitions: AtomicUsize,
    account_login_lock: Mutex<()>,
    account_context_transition_lock: Mutex<()>,
    routing_recovery_generation: AtomicU64,
    token_expired: AtomicBool,
}

impl AccountRuntime {
    pub fn new(host: Arc<dyn AccountRuntimeHost>) -> Arc<Self> {
        Arc::new(Self {
            host,
            account_context: RwLock::new(None),
            account_context_generation: AtomicU64::new(1),
            account_context_transitions: AtomicUsize::new(0),
            account_login_lock: Mutex::new(()),
            account_context_transition_lock: Mutex::new(()),
            routing_recovery_generation: AtomicU64::new(0),
            token_expired: AtomicBool::new(false),
        })
    }

    pub fn account_context_generation(&self) -> u64 {
        self.account_context_generation.load(Ordering::Acquire)
    }

    pub fn account_context_is_current(&self, generation: u64) -> bool {
        self.account_context_transitions.load(Ordering::Acquire) == 0
            && self.account_context_generation() == generation
    }

    pub fn is_token_expired(&self) -> bool {
        self.token_expired.load(Ordering::Relaxed)
    }

    pub fn mark_token_expired(&self) {
        self.token_expired.store(true, Ordering::Relaxed);
    }

    async fn begin_account_transition(&self) -> AccountContextTransitionGuard<'_> {
        let transition_guard = self.account_context_transition_lock.lock().await;
        self.account_context_transitions
            .fetch_add(1, Ordering::AcqRel);
        self.account_context_generation
            .fetch_add(1, Ordering::AcqRel);
        AccountContextTransitionGuard {
            runtime: self,
            transition_guard: Some(transition_guard),
            active: true,
        }
    }

    async fn begin_account_transition_if_current(
        &self,
        expected_generation: u64,
    ) -> Option<AccountContextTransitionGuard<'_>> {
        let transition_guard = self.account_context_transition_lock.lock().await;
        if !self.account_context_is_current(expected_generation) {
            return None;
        }
        self.account_context_transitions
            .fetch_add(1, Ordering::AcqRel);
        self.account_context_generation
            .fetch_add(1, Ordering::AcqRel);
        Some(AccountContextTransitionGuard {
            runtime: self,
            transition_guard: Some(transition_guard),
            active: true,
        })
    }

    async fn read_account_context_raw(&self) -> Result<(AccountSession, String)> {
        self.account_context
            .read()
            .await
            .clone()
            .map(|context| (context.session, context.relay_url))
            .ok_or_else(|| anyhow!("not logged in"))
    }

    pub async fn read_account_context(&self) -> Result<(AccountSession, String)> {
        let generation = self.account_context_generation();
        self.read_account_context_for_generation(generation).await
    }

    pub async fn read_account_context_for_generation(
        &self,
        generation: u64,
    ) -> Result<(AccountSession, String)> {
        if !self.account_context_is_current(generation) {
            return Err(anyhow!("account context changed"));
        }
        let context = self.read_account_context_raw().await?;
        if !self.account_context_is_current(generation) {
            return Err(anyhow!("account context changed"));
        }
        Ok(context)
    }

    pub async fn is_logged_in(&self) -> bool {
        self.read_account_context().await.is_ok()
    }

    pub async fn try_restore_session(&self) -> Option<String> {
        let transition = self.begin_account_transition().await;
        self.host.stop_device_routing().await;
        let restored = match session_store::load_session_detailed() {
            Ok(Some(loaded)) => {
                let relay_url = match normalize_relay_url(&loaded.relay_url) {
                    Ok(url) => url,
                    Err(error) => {
                        log::warn!("Ignoring invalid persisted relay URL: {error}");
                        session_store::clear_session();
                        transition.finish();
                        return None;
                    }
                };
                let user_id = loaded.user_id.clone();
                if let Some(device_id) = loaded.device_id.as_deref() {
                    if let Err(error) = DeviceIdentity::adopt_account_device_id(device_id) {
                        log::warn!("Failed to adopt restored session device_id: {error}");
                    }
                }
                let session = AccountSession::new(loaded.token, user_id.clone(), loaded.master_key);
                *self.account_context.write().await =
                    Some(AccountContextState { session, relay_url });
                log::info!("Restored account session for user {user_id}");
                Some(user_id)
            }
            Ok(None) => None,
            Err(error) => {
                log::warn!("Failed to load persisted session: {error}");
                None
            }
        };
        transition.finish();
        restored
    }

    pub async fn advance_github_login(
        self: &Arc<Self>,
        transaction_id: Option<String>,
    ) -> Result<AccountLoginProgress> {
        if let Some(transaction_id) = transaction_id {
            let response = self.poll_github_auth(transaction_id).await?;
            match response.status.as_str() {
                "pending" => return Ok(AccountLoginProgress::Waiting),
                "authorized" => {}
                _ => {
                    return Err(anyhow!(
                        "GitHub authorization expired or was not completed; restart sign-in"
                    ))
                }
            }
        } else {
            let mut identity = openbitfun_services_integrations::account_identity::AccountIdentityClient::from_environment().await?;
            if identity.me().await?.is_none() {
                return Ok(AccountLoginProgress::Authorization(
                    self.start_github_auth().await?,
                ));
            }
        }
        Ok(AccountLoginProgress::Complete(
            self.login_with_identity().await?,
        ))
    }

    pub async fn start_github_auth(
        &self,
    ) -> Result<openbitfun_product_domains::account::GitHubAuthStart> {
        Ok(openbitfun_services_integrations::account_identity::start_auth_flow().await?)
    }

    pub async fn poll_github_auth(
        &self,
        transaction_id: String,
    ) -> Result<openbitfun_product_domains::account::GitHubAuthPollResponse> {
        Ok(
            openbitfun_services_integrations::account_identity::poll_auth_flow(
                openbitfun_product_domains::account::GitHubAuthPollRequest { transaction_id },
            )
            .await?,
        )
    }

    pub async fn login_with_identity(self: &Arc<Self>) -> Result<AccountLoginResult> {
        let _login_guard = self.account_login_lock.lock().await;
        let relay_url = openbitfun_product_domains::account::DEFAULT_RELAY_URL.to_string();
        let expected_generation = self.account_context_generation();
        if !self.account_context_is_current(expected_generation) {
            return Err(anyhow!("account context changed"));
        }

        let device = current_device_identity()?;
        let client = AccountClient::new();
        let (session, profile) = client
            .login_with_identity(&relay_url, &device)
            .await
            .map_err(|error| anyhow!("login failed: {error}"))?;
        let previous_account_context = self.account_context.read().await.clone();
        let retired_background_owner = match self.host.retire_background_routing_owner().await {
            Ok(retired) => retired,
            Err(failure) => {
                if failure.owner_may_exit {
                    self.schedule_routing_recovery_after_background_owner_exit(
                        expected_generation,
                        device.device_name.clone(),
                    );
                }
                revoke_rejected_login_candidate(&client, &relay_url, &session).await;
                return Err(failure.error);
            }
        };
        let Some(transition) = self
            .begin_account_transition_if_current(expected_generation)
            .await
        else {
            revoke_rejected_login_candidate(&client, &relay_url, &session).await;
            return Err(anyhow!("account context changed"));
        };
        self.host.stop_device_routing().await;
        session_store::clear_session();

        let user_id = session.user_id.clone();
        let token = session.token.clone();
        let master_key = session.master_key;
        *self.account_context.write().await = Some(AccountContextState {
            session: session.clone(),
            relay_url: relay_url.clone(),
        });
        session_store::save_credential_hint(&profile.login, &relay_url);
        self.token_expired.store(false, Ordering::Relaxed);

        if let Err(error) = session_store::save_session_with_device(
            &token,
            &user_id,
            &master_key,
            &relay_url,
            Some(device.device_id.as_str()),
        ) {
            log::warn!("Failed to persist session: {error}");
        }
        let generation = transition.finish();
        let routing = self
            .host
            .start_device_routing(AccountRoutingStartRequest {
                session,
                relay_url: relay_url.clone(),
                device_name: device.device_name,
                account_generation: generation,
            })
            .await;
        revoke_replaced_account_context(&client, previous_account_context, &relay_url, &token)
            .await;

        Ok(AccountLoginResult {
            user_id,
            relay_url,
            routing_owner_replaced: retired_background_owner,
            routing_connected: routing.is_ok(),
            routing_error: routing.err().map(|error| error.to_string()),
        })
    }

    pub async fn restore_device_routing(self: &Arc<Self>, device_name: &str) -> Result<()> {
        let generation = self.account_context_generation();
        let (session, relay_url) = self.read_account_context_for_generation(generation).await?;
        self.host
            .start_device_routing(AccountRoutingStartRequest {
                session,
                relay_url,
                device_name: device_name.to_string(),
                account_generation: generation,
            })
            .await
    }

    pub async fn logout(&self) -> Result<()> {
        let mut identity = openbitfun_services_integrations::account_identity::AccountIdentityClient::from_environment().await?;
        identity.logout().await?;
        let transition = self.begin_account_transition().await;
        self.host.stop_device_routing().await;
        if self.host.request_background_routing_owner_shutdown() {
            log::info!("Signalled the background account routing owner to shut down");
        }
        if let Ok((session, relay_url)) = self.read_account_context_raw().await {
            let _ = AccountClient::new()
                .revoke_token(&relay_url, &session)
                .await;
        }
        *self.account_context.write().await = None;
        session_store::clear_session();
        session_store::clear_credential_hint();
        self.token_expired.store(false, Ordering::Relaxed);
        transition.finish();
        Ok(())
    }

    pub async fn expire_rejected_context(
        &self,
        account_generation: u64,
        expected_token: &str,
    ) -> bool {
        let Some(transition) = self
            .begin_account_transition_if_current(account_generation)
            .await
        else {
            return false;
        };
        self.host.stop_device_routing().await;
        let mut context = self.account_context.write().await;
        if context
            .as_ref()
            .is_none_or(|context| context.session.token != expected_token)
        {
            transition.finish();
            return false;
        }
        *context = None;
        drop(context);
        self.token_expired.store(true, Ordering::Relaxed);
        session_store::clear_session();
        transition.finish();
        true
    }

    pub async fn account_info(&self) -> Result<AccountInfo> {
        let (session, relay_url) = self.read_account_context().await?;
        let device = current_device_identity()?;
        Ok(AccountInfo {
            user_id: session.user_id,
            relay_url,
            device_id: device.device_id,
            device_name: device.device_name,
        })
    }

    pub async fn list_devices(&self) -> Result<Vec<AccountDevice>> {
        let (session, relay_url) = self.read_account_context().await?;
        let devices = AccountClient::new()
            .list_devices(&relay_url, &session)
            .await?;
        Ok(devices
            .into_iter()
            .map(|device| AccountDevice {
                device_id: device.device_id,
                device_name: device.device_name,
                online: device.online,
            })
            .collect())
    }

    pub async fn snapshot(&self) -> AccountSnapshot {
        let logged_in = self.is_logged_in().await;
        let info = if logged_in {
            self.account_info().await.ok()
        } else {
            None
        };
        let devices = if logged_in {
            self.list_devices().await.unwrap_or_default()
        } else {
            Vec::new()
        };
        AccountSnapshot {
            logged_in,
            info,
            devices,
        }
    }

    fn schedule_routing_recovery_after_background_owner_exit(
        self: &Arc<Self>,
        expected_generation: u64,
        device_name: String,
    ) {
        if !self.account_context_is_current(expected_generation)
            || self
                .routing_recovery_generation
                .swap(expected_generation, Ordering::AcqRel)
                == expected_generation
        {
            return;
        }
        let runtime = Arc::clone(self);
        tokio::spawn(async move {
            while runtime.routing_recovery_generation.load(Ordering::Acquire) == expected_generation
                && runtime.account_context_is_current(expected_generation)
                && runtime.host.background_routing_owner_is_running()
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if runtime.routing_recovery_generation.load(Ordering::Acquire) == expected_generation
                && runtime.account_context_is_current(expected_generation)
                && !runtime.host.background_routing_owner_is_running()
            {
                if let Err(error) = runtime.restore_device_routing(&device_name).await {
                    log::warn!(
                        "Failed to restore account routing after background owner exit: {error}"
                    );
                }
            }
            let _ = runtime.routing_recovery_generation.compare_exchange(
                expected_generation,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        });
    }
}

struct AccountContextTransitionGuard<'a> {
    runtime: &'a AccountRuntime,
    transition_guard: Option<MutexGuard<'a, ()>>,
    active: bool,
}

impl AccountContextTransitionGuard<'_> {
    fn finish(mut self) -> u64 {
        self.release()
    }

    fn release(&mut self) -> u64 {
        if self.active {
            self.runtime
                .account_context_generation
                .fetch_add(1, Ordering::AcqRel);
            self.runtime
                .account_context_transitions
                .fetch_sub(1, Ordering::AcqRel);
            self.active = false;
        }
        let generation = self.runtime.account_context_generation();
        drop(self.transition_guard.take());
        generation
    }
}

impl Drop for AccountContextTransitionGuard<'_> {
    fn drop(&mut self) {
        self.release();
    }
}

fn normalize_relay_url(relay_url: &str) -> Result<String> {
    let parsed = validate_relay_base_url(relay_url.trim())?;
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn current_device_identity() -> Result<DeviceIdentity> {
    DeviceIdentity::from_current_machine().map_err(|error| anyhow!("detect device: {error}"))
}

async fn revoke_rejected_login_candidate(
    client: &AccountClient,
    relay_url: &str,
    session: &AccountSession,
) {
    if let Err(error) = client.revoke_token(relay_url, session).await {
        log::warn!("Failed to revoke rejected login candidate token: {error}");
    }
}

fn replaced_account_revocation_target(
    previous: Option<AccountContextState>,
    replacement_relay_url: &str,
    replacement_token: &str,
) -> Option<AccountContextState> {
    previous.filter(|context| {
        context.relay_url != replacement_relay_url || context.session.token != replacement_token
    })
}

async fn revoke_replaced_account_context(
    client: &AccountClient,
    previous: Option<AccountContextState>,
    replacement_relay_url: &str,
    replacement_token: &str,
) {
    let Some(previous) =
        replaced_account_revocation_target(previous, replacement_relay_url, replacement_token)
    else {
        return;
    };
    if let Err(error) = client
        .revoke_token(&previous.relay_url, &previous.session)
        .await
    {
        log::warn!("Failed to revoke replaced account token: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAccountRuntimeHost;

    #[async_trait]
    impl AccountRuntimeHost for TestAccountRuntimeHost {
        async fn retire_background_routing_owner(
            &self,
        ) -> std::result::Result<bool, BackgroundRoutingOwnerRetirementError> {
            Ok(false)
        }

        fn background_routing_owner_is_running(&self) -> bool {
            false
        }

        fn request_background_routing_owner_shutdown(&self) -> bool {
            false
        }

        async fn start_device_routing(&self, _request: AccountRoutingStartRequest) -> Result<()> {
            Ok(())
        }

        async fn stop_device_routing(&self) {}
    }

    fn test_runtime() -> Arc<AccountRuntime> {
        AccountRuntime::new(Arc::new(TestAccountRuntimeHost))
    }

    #[tokio::test]
    async fn rejected_account_transition_does_not_advance_the_generation() {
        let runtime = test_runtime();
        let generation = runtime.account_context_generation();

        assert!(runtime
            .begin_account_transition_if_current(generation + 1)
            .await
            .is_none());
        assert_eq!(runtime.account_context_generation(), generation);
    }
}
