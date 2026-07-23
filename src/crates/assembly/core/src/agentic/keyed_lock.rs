use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, Weak};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

pub(crate) struct KeyedAsyncLockGuard {
    guard: Option<OwnedMutexGuard<()>>,
    registry: Arc<StdMutex<HashMap<String, Weak<AsyncMutex<()>>>>>,
    key: String,
    generation: Weak<AsyncMutex<()>>,
}

impl Drop for KeyedAsyncLockGuard {
    fn drop(&mut self) {
        // Release the async lock before removing its dead weak entry. A new
        // waiter either upgrades this generation first or creates the next one
        // after removal; two live generations cannot overlap.
        drop(self.guard.take());
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if registry
            .get(&self.key)
            .is_some_and(|current| current.ptr_eq(&self.generation) && current.upgrade().is_none())
        {
            registry.remove(&self.key);
        }
    }
}

/// Race-safe keyed async lock registry.
///
/// The registry owns only weak references. Waiters keep the lock generation
/// alive through their strong references, while dead keys are reclaimed on the
/// next acquisition without allowing two live generations for one key.
#[derive(Clone, Default)]
pub(crate) struct KeyedAsyncLock {
    registry: Arc<StdMutex<HashMap<String, Weak<AsyncMutex<()>>>>>,
}

impl KeyedAsyncLock {
    pub(crate) async fn lock(&self, key: &str) -> KeyedAsyncLockGuard {
        let (lock, generation) = {
            let mut registry = self
                .registry
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(lock) = registry.get(key).and_then(Weak::upgrade) {
                let generation = Arc::downgrade(&lock);
                (lock, generation)
            } else {
                let lock = Arc::new(AsyncMutex::new(()));
                let generation = Arc::downgrade(&lock);
                registry.insert(key.to_string(), generation.clone());
                (lock, generation)
            }
        };
        let guard = lock.lock_owned().await;
        KeyedAsyncLockGuard {
            guard: Some(guard),
            registry: self.registry.clone(),
            key: key.to_string(),
            generation,
        }
    }

    #[cfg(test)]
    fn registry_len(&self) -> usize {
        self.registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::KeyedAsyncLock;

    #[tokio::test]
    async fn dead_key_generations_are_reclaimed() {
        let locks = KeyedAsyncLock::default();
        for index in 0..64 {
            drop(locks.lock(&format!("missing-{index}")).await);
        }

        let survivor = locks.lock("survivor").await;

        assert_eq!(locks.registry_len(), 1);
        drop(survivor);
        assert_eq!(locks.registry_len(), 0);
    }
}
