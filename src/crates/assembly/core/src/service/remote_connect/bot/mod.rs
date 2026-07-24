//! Bot integration compatibility facade for Remote Connect.
//!
//! Provider-neutral bot DTOs, menu and locale rendering, persistence, and file
//! delivery helpers live in `bitfun-services-integrations`. Core keeps the
//! command router and platform bot adapters because they still call concrete
//! session, coordinator, image, and runtime services.

pub mod command_router;
pub mod feishu;
pub mod locale;
pub mod menu;
pub mod telegram;
pub mod weixin;

pub use bitfun_services_integrations::remote_connect::bot::{
    auto_push_failed_message, auto_push_intro, auto_push_skip_too_large_message,
    collect_auto_push_files, detect_mime_type, extract_computer_file_paths,
    extract_downloadable_file_paths, format_file_size, get_file_metadata, load_bot_persistence,
    read_workspace_file, resolve_workspace_path, save_bot_persistence, update_bot_persistence,
    AutoPushFile, BotConfig, BotLanguage, BotPairingInfo, BotPersistenceData, MenuItem,
    MenuItemStyle, MenuView, RemoteConnectFormState, RemoteDeviceTarget, SavedBotConnection,
    WorkspaceFileContent,
};
pub use command_router::{
    set_delegated_identity_provider, BotChatState, ForwardRequest, ForwardedTurnResult,
    HandleResult,
};

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

/// Serializes persistence commits with lifecycle generation changes for one
/// bot platform. A retired polling task can finish later, but it can no longer
/// overwrite the replacement instance's persisted state.
#[derive(Default)]
pub(crate) struct BotSlotFence {
    generation: StdMutex<u64>,
}

impl BotSlotFence {
    pub(crate) fn advance(&self) -> u64 {
        let mut generation = self
            .generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *generation = generation.wrapping_add(1);
        if *generation == 0 {
            *generation = 1;
        }
        *generation
    }

    pub(crate) fn is_current(&self, expected_generation: u64) -> bool {
        *self
            .generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            == expected_generation
    }

    fn commit_if_current<R>(
        &self,
        expected_generation: u64,
        commit: impl FnOnce() -> R,
    ) -> Option<R> {
        let generation = self
            .generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *generation != expected_generation {
            return None;
        }
        Some(commit())
    }
}

/// Shared account-identity epoch plus a per-platform lifecycle owner. Network
/// work is not awaited during account replacement; instead every state commit
/// is fenced and sanitized if it started under an older account epoch.
pub(crate) struct BotRuntimeFence {
    account_identity_epoch: Arc<AtomicU64>,
    observed_identity_epoch: AtomicU64,
    slot: Arc<BotSlotFence>,
    lifecycle_generation: u64,
}

impl BotRuntimeFence {
    pub(crate) fn new(
        account_identity_epoch: Arc<AtomicU64>,
        slot: Arc<BotSlotFence>,
        lifecycle_generation: u64,
    ) -> Self {
        let observed_identity_epoch = account_identity_epoch.load(Ordering::Acquire);
        Self {
            account_identity_epoch,
            observed_identity_epoch: AtomicU64::new(observed_identity_epoch),
            slot,
            lifecycle_generation,
        }
    }

    pub(crate) fn standalone() -> Self {
        let account_identity_epoch = Arc::new(AtomicU64::new(0));
        let slot = Arc::new(BotSlotFence::default());
        let lifecycle_generation = slot.advance();
        Self::new(account_identity_epoch, slot, lifecycle_generation)
    }

    pub(crate) fn identity_epoch(&self) -> u64 {
        self.account_identity_epoch.load(Ordering::Acquire)
    }

    pub(crate) fn is_lifecycle_current(&self) -> bool {
        self.slot.is_current(self.lifecycle_generation)
    }

    pub(crate) fn reconcile_states<K>(&self, states: &mut HashMap<K, BotChatState>) -> bool
    where
        K: Eq + Hash,
    {
        let current = self.identity_epoch();
        if self.observed_identity_epoch.load(Ordering::Acquire) == current {
            return false;
        }
        for state in states.values_mut() {
            state.clear_delegated_identity();
        }
        self.observed_identity_epoch
            .store(current, Ordering::Release);
        true
    }

    pub(crate) fn clear_states<K>(&self, states: &mut HashMap<K, BotChatState>)
    where
        K: Eq + Hash,
    {
        let current = self.identity_epoch();
        for state in states.values_mut() {
            state.clear_delegated_identity();
        }
        self.observed_identity_epoch
            .store(current, Ordering::Release);
    }

    pub(crate) fn sanitize_after_epoch(&self, started_epoch: u64, state: &mut BotChatState) {
        if started_epoch != self.identity_epoch() {
            state.clear_delegated_identity();
        }
    }

    pub(crate) fn persistence_snapshot(&self, state: &BotChatState) -> BotChatState {
        let mut snapshot = state.clone();
        if self.observed_identity_epoch.load(Ordering::Acquire) != self.identity_epoch() {
            snapshot.clear_delegated_identity();
        }
        snapshot
    }

    pub(crate) fn commit_if_current<R>(&self, commit: impl FnOnce() -> R) -> Option<R> {
        self.slot
            .commit_if_current(self.lifecycle_generation, commit)
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn account_epoch_sanitizes_a_late_state_commit() {
        let account_epoch = Arc::new(AtomicU64::new(3));
        let slot = Arc::new(BotSlotFence::default());
        let generation = slot.advance();
        let fence = BotRuntimeFence::new(account_epoch.clone(), slot, generation);
        let mut states = HashMap::from([("chat", {
            let mut state = BotChatState::new("chat".into());
            state.relay_url = Some("https://relay-a.example".into());
            state.set_delegated_identity("token-a".into(), vec![1; 32]);
            state
        })]);

        account_epoch.fetch_add(1, Ordering::AcqRel);

        assert!(fence.reconcile_states(&mut states));
        let state = states.get("chat").expect("chat state should remain");
        assert!(state.relay_url.is_none());
        assert!(!state.has_delegated_identity());
    }

    #[test]
    fn account_epoch_rejects_identity_returned_by_late_pairing() {
        let account_epoch = Arc::new(AtomicU64::new(11));
        let slot = Arc::new(BotSlotFence::default());
        let generation = slot.advance();
        let fence = BotRuntimeFence::new(account_epoch.clone(), slot, generation);
        let started_epoch = fence.identity_epoch();
        let mut late_state = BotChatState::new("late-chat".into());
        late_state.relay_url = Some("https://relay-a.example".into());
        late_state.set_delegated_identity("token-a".into(), vec![2; 32]);

        account_epoch.fetch_add(1, Ordering::AcqRel);
        fence.sanitize_after_epoch(started_epoch, &mut late_state);

        assert!(late_state.relay_url.is_none());
        assert!(!late_state.has_delegated_identity());
    }

    #[test]
    fn retired_slot_cannot_commit_after_replacement() {
        let account_epoch = Arc::new(AtomicU64::new(0));
        let slot = Arc::new(BotSlotFence::default());
        let old_generation = slot.advance();
        let old = BotRuntimeFence::new(account_epoch.clone(), slot.clone(), old_generation);
        let new_generation = slot.advance();
        let replacement = BotRuntimeFence::new(account_epoch, slot, new_generation);
        let committed = AtomicU64::new(0);

        assert!(old
            .commit_if_current(|| committed.store(1, Ordering::Release))
            .is_none());
        assert!(replacement
            .commit_if_current(|| committed.store(2, Ordering::Release))
            .is_some());
        assert_eq!(committed.load(Ordering::Acquire), 2);
    }
}
