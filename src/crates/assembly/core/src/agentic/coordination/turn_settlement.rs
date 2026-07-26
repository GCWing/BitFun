use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use tokio::sync::watch;

type TurnKey = (String, String);
const COMPLETED_TURN_CAPACITY: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnSettlementWait {
    Settled,
    TimedOut,
    Unknown,
}

#[derive(Default)]
pub(super) struct TurnSettlementTracker {
    active: DashMap<TurnKey, Arc<TurnSettlementEntry>>,
    completed: DashMap<TurnKey, Arc<TurnSettlementEntry>>,
    completed_order: Mutex<VecDeque<(TurnKey, Arc<TurnSettlementEntry>)>>,
}

struct TurnSettlementEntry {
    sender: watch::Sender<bool>,
    registrations: AtomicUsize,
    accepted: AtomicBool,
}

impl TurnSettlementTracker {
    pub(super) fn try_register_pending(
        self: &Arc<Self>,
        session_id: impl Into<String>,
        turn_id: impl Into<String>,
    ) -> Option<TurnSettlementRegistration> {
        let key = (session_id.into(), turn_id.into());
        let entry = match self.active.entry(key.clone()) {
            Entry::Occupied(_) => return None,
            Entry::Vacant(vacant) => {
                if self.completed.contains_key(&key) {
                    return None;
                }
                let (sender, _) = watch::channel(false);
                let entry = Arc::new(TurnSettlementEntry {
                    sender,
                    registrations: AtomicUsize::new(1),
                    accepted: AtomicBool::new(false),
                });
                vacant.insert(Arc::clone(&entry));
                entry
            }
        };
        Some(TurnSettlementRegistration {
            tracker: Arc::clone(self),
            key,
            entry,
        })
    }

    pub(super) fn register_accepted(
        self: &Arc<Self>,
        session_id: impl Into<String>,
        turn_id: impl Into<String>,
    ) -> TurnSettlementRegistration {
        let key = (session_id.into(), turn_id.into());
        let entry = match self.active.entry(key.clone()) {
            Entry::Occupied(occupied) => {
                let entry = Arc::clone(occupied.get());
                entry.registrations.fetch_add(1, Ordering::AcqRel);
                entry.accepted.store(true, Ordering::Release);
                entry
            }
            Entry::Vacant(vacant) => {
                self.completed.remove(&key);
                let (sender, _) = watch::channel(false);
                let entry = Arc::new(TurnSettlementEntry {
                    sender,
                    registrations: AtomicUsize::new(1),
                    accepted: AtomicBool::new(true),
                });
                vacant.insert(Arc::clone(&entry));
                entry
            }
        };
        TurnSettlementRegistration {
            tracker: Arc::clone(self),
            key,
            entry,
        }
    }

    pub(super) async fn wait(
        &self,
        session_id: &str,
        turn_id: &str,
        max_wait: Duration,
    ) -> TurnSettlementWait {
        let key = (session_id.to_string(), turn_id.to_string());
        let mut receiver = {
            let Some(entry) = self.active.get(&key) else {
                return if self.completed.contains_key(&key) {
                    TurnSettlementWait::Settled
                } else {
                    TurnSettlementWait::Unknown
                };
            };
            entry.sender.subscribe()
        };
        if *receiver.borrow() {
            return TurnSettlementWait::Settled;
        }

        let outcome =
            match tokio::time::timeout(max_wait, receiver.wait_for(|settled| *settled)).await {
                Ok(Ok(_)) => TurnSettlementWait::Settled,
                Ok(Err(_)) => TurnSettlementWait::Unknown,
                Err(_) => TurnSettlementWait::TimedOut,
            };
        outcome
    }

    fn remember_completed(&self, key: TurnKey, entry: Arc<TurnSettlementEntry>) {
        self.completed.insert(key.clone(), Arc::clone(&entry));
        let mut order = self
            .completed_order
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        order.push_back((key, entry));
        while order.len() > COMPLETED_TURN_CAPACITY {
            let Some((old_key, old_entry)) = order.pop_front() else {
                break;
            };
            if self
                .completed
                .get(&old_key)
                .is_some_and(|current| Arc::ptr_eq(current.value(), &old_entry))
            {
                self.completed.remove(&old_key);
            }
        }
    }
}

pub(super) struct TurnSettlementRegistration {
    tracker: Arc<TurnSettlementTracker>,
    key: TurnKey,
    entry: Arc<TurnSettlementEntry>,
}

impl std::fmt::Debug for TurnSettlementRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TurnSettlementRegistration")
            .field("session_id", &self.key.0)
            .field("turn_id", &self.key.1)
            .finish_non_exhaustive()
    }
}

impl Clone for TurnSettlementRegistration {
    fn clone(&self) -> Self {
        self.entry.registrations.fetch_add(1, Ordering::AcqRel);
        Self {
            tracker: Arc::clone(&self.tracker),
            key: self.key.clone(),
            entry: Arc::clone(&self.entry),
        }
    }
}

impl TurnSettlementRegistration {
    pub(super) fn accept(&self) {
        self.entry.accepted.store(true, Ordering::Release);
    }
}

impl Drop for TurnSettlementRegistration {
    fn drop(&mut self) {
        let Entry::Occupied(occupied) = self.tracker.active.entry(self.key.clone()) else {
            return;
        };
        if !Arc::ptr_eq(occupied.get(), &self.entry) {
            return;
        }
        let previous = self.entry.registrations.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "turn settlement registration underflow");
        if previous == 1 {
            if self.entry.accepted.load(Ordering::Acquire) {
                self.entry.sender.send_replace(true);
                self.tracker
                    .remember_completed(self.key.clone(), Arc::clone(&self.entry));
            }
            occupied.remove();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::{TurnSettlementTracker, TurnSettlementWait};

    #[tokio::test]
    async fn settlement_is_scoped_to_the_exact_session_and_turn() {
        let tracker = Arc::new(TurnSettlementTracker::default());
        let first = tracker.register_accepted("session-1", "turn-1");
        let second = tracker.register_accepted("session-1", "turn-2");

        drop(second);

        assert_eq!(
            tracker
                .wait("session-1", "turn-2", Duration::from_millis(10))
                .await,
            TurnSettlementWait::Settled
        );
        assert_eq!(
            tracker
                .wait("session-1", "turn-1", Duration::from_millis(10))
                .await,
            TurnSettlementWait::TimedOut
        );

        drop(first);
        assert_eq!(
            tracker
                .wait("session-1", "turn-1", Duration::from_millis(10))
                .await,
            TurnSettlementWait::Settled
        );
    }

    #[tokio::test]
    async fn recent_finished_turn_is_settled_and_unknown_turn_is_distinct() {
        let tracker = Arc::new(TurnSettlementTracker::default());
        let registration = tracker.register_accepted("session-1", "turn-1");
        drop(registration);

        assert_eq!(
            tracker
                .wait("session-1", "turn-1", Duration::from_secs(1))
                .await,
            TurnSettlementWait::Settled
        );
        assert_eq!(
            tracker
                .wait("session-1", "turn-never-registered", Duration::from_secs(1))
                .await,
            TurnSettlementWait::Unknown
        );
    }

    #[test]
    fn completion_before_subscribe_keeps_the_settled_value() {
        let tracker = Arc::new(TurnSettlementTracker::default());
        let registration = tracker.register_accepted("session-1", "turn-1");
        let entry = Arc::clone(
            tracker
                .active
                .get(&("session-1".to_string(), "turn-1".to_string()))
                .expect("registered turn")
                .value(),
        );

        drop(registration);
        let receiver = entry.sender.subscribe();

        assert!(*receiver.borrow());
    }

    #[tokio::test]
    async fn cloned_registration_covers_queued_and_active_lifetimes() {
        let tracker = Arc::new(TurnSettlementTracker::default());
        let queued = tracker.register_accepted("session-1", "turn-1");
        let active = queued.clone();

        drop(queued);
        assert_eq!(
            tracker
                .wait("session-1", "turn-1", Duration::from_millis(10))
                .await,
            TurnSettlementWait::TimedOut
        );

        drop(active);
        assert_eq!(
            tracker
                .wait("session-1", "turn-1", Duration::from_millis(10))
                .await,
            TurnSettlementWait::Settled
        );
    }

    #[tokio::test]
    async fn rejected_registration_is_not_recorded_as_settled() {
        let tracker = Arc::new(TurnSettlementTracker::default());
        let registration = tracker
            .try_register_pending("session-1", "turn-rejected")
            .expect("first registration");

        drop(registration);

        assert_eq!(
            tracker
                .wait("session-1", "turn-rejected", Duration::from_millis(10),)
                .await,
            TurnSettlementWait::Unknown
        );
    }

    #[test]
    fn pending_registration_rejects_active_and_completed_turn_ids() {
        let tracker = Arc::new(TurnSettlementTracker::default());
        let active = tracker
            .try_register_pending("session-1", "turn-1")
            .expect("first registration");
        assert!(
            tracker
                .try_register_pending("session-1", "turn-1")
                .is_none(),
            "an active turn ID must not create a second settlement generation"
        );

        active.accept();
        drop(active);
        assert!(
            tracker
                .try_register_pending("session-1", "turn-1")
                .is_none(),
            "a recently completed turn ID must not be reused"
        );
    }
}
