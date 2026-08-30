//! In-process transport admission for a provider credential scope. Normal
//! requests remain parallel; an explicit provider concurrency rejection makes
//! subsequent streams queue behind an adaptive concurrency window. Complete
//! successful responses grow the window; another rejection reduces it.
//! This is not an Agent/task limiter and never holds capacity during tool work.

use anyhow::{Context, Result};
use log::debug;
use reqwest::RequestBuilder;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, Notify};

const IDLE_SCOPE_RETENTION: Duration = Duration::from_secs(300);
// Add one slot after two successful windows at the current capacity. Requiring
// observed demand at that capacity avoids inflating limits on serial workloads.
const SUCCESS_WINDOWS_TO_GROW: usize = 2;
type ScopeKey = [u8; 32];
static SCOPES: OnceLock<Mutex<HashMap<ScopeKey, Arc<RequestCapacity>>>> = OnceLock::new();

struct State {
    active: usize,
    limit: Option<usize>,
    successes: usize,
    demand_at_limit: bool,
    generation: u64,
    not_before: Instant,
    last_activity: Instant,
}

pub(super) struct RequestCapacity {
    state: Mutex<State>,
    queue: AsyncMutex<()>,
    changed: Notify,
}

impl RequestCapacity {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            state: Mutex::new(State {
                active: 0,
                limit: None,
                successes: 0,
                demand_at_limit: false,
                generation: 0,
                not_before: now,
                last_activity: now,
            }),
            queue: AsyncMutex::new(()),
            changed: Notify::new(),
        }
    }

    pub(super) fn for_request(builder: &RequestBuilder) -> Result<Arc<Self>> {
        let request = builder
            .try_clone()
            .context("Cannot identify provider request capacity scope")?
            .build()?;
        // Different clients/models on the same provider credential share quota.
        // Retain only a digest, never credentials or a URL containing secrets.
        let mut hasher = Sha256::new();
        hasher.update(request.url().origin().ascii_serialization().as_bytes());
        for name in ["authorization", "x-api-key", "api-key", "x-goog-api-key"] {
            hasher.update([0]);
            hasher.update(name.as_bytes());
            if let Some(value) = request.headers().get(name) {
                hasher.update(value.as_bytes());
            }
        }
        for (name, value) in request.url().query_pairs() {
            if matches!(
                name.as_ref(),
                "key" | "api_key" | "api-key" | "access_token"
            ) {
                hasher.update([0]);
                hasher.update(name.as_bytes());
                hasher.update([0]);
                hasher.update(value.as_bytes());
            }
        }
        let key: ScopeKey = hasher.finalize().into();
        let mut scopes = SCOPES.get_or_init(Mutex::default).lock().unwrap();
        scopes.retain(|_, scope| {
            Arc::strong_count(scope) > 1
                || scope.state.lock().unwrap().last_activity.elapsed() < IDLE_SCOPE_RETENTION
        });
        Ok(Arc::clone(
            scopes.entry(key).or_insert_with(|| Arc::new(Self::new())),
        ))
    }

    fn concurrency_rejected(&self, generation: u64, delay: Duration) {
        let mut state = self.state.lock().unwrap();
        // A batch of concurrent requests can all be rejected together. Reduce
        // once for that admission generation, not once per late response.
        if generation == state.generation {
            let limit = state.limit.map(|limit| (limit / 2).max(1)).unwrap_or(1);
            state.limit = Some(limit);
            state.successes = 0;
            state.demand_at_limit = false;
            state.generation = state.generation.wrapping_add(1);
            debug!("Provider capacity reduced: concurrency_limit={limit}");
        }
        state.last_activity = Instant::now();
        state.not_before = state.not_before.max(state.last_activity + delay);
        drop(state);
        self.changed.notify_one();
    }

    fn completed(&self, generation: u64, successful: bool) {
        let mut state = self.state.lock().unwrap();
        if generation != state.generation {
            return; // An old stream must not undo a more recent rejection.
        }
        let Some(limit) = state.limit else {
            return;
        };
        if !successful {
            state.successes = 0;
            return;
        }
        if !state.demand_at_limit || Instant::now() < state.not_before {
            return;
        }
        state.successes += 1;
        if state.successes >= limit.saturating_mul(SUCCESS_WINDOWS_TO_GROW) {
            let next_limit = limit.saturating_add(1);
            state.limit = Some(next_limit);
            state.successes = 0;
            state.demand_at_limit = false;
            debug!("Provider capacity recovering: concurrency_limit={next_limit}");
            drop(state);
            self.changed.notify_one();
        }
    }

    pub(super) async fn acquire(self: &Arc<Self>) -> RequestPermit {
        // FIFO admission; cancellation drops the waiting future/lock without
        // leaving a ticket or consuming an active slot.
        let _queue = self.queue.lock().await;
        loop {
            let changed = self.changed.notified();
            let wait_until = {
                let mut state = self.state.lock().unwrap();
                let now = Instant::now();
                if state.active == 0 && state.last_activity.elapsed() >= IDLE_SCOPE_RETENTION {
                    state.limit = None;
                    state.successes = 0;
                    state.demand_at_limit = false;
                    state.generation = state.generation.wrapping_add(1);
                }
                if state.limit.is_none_or(|limit| state.active < limit) && now >= state.not_before {
                    state.active += 1;
                    state.demand_at_limit |= state.limit.is_some_and(|limit| state.active >= limit);
                    state.last_activity = now;
                    return RequestPermit {
                        capacity: Arc::clone(self),
                        generation: state.generation,
                        completion: Some(RequestCompletion {
                            capacity: Arc::clone(self),
                            generation: state.generation,
                            successful: false,
                        }),
                    };
                }
                (state.not_before > now).then_some(state.not_before)
            };
            match wait_until {
                Some(deadline) => tokio::select! {
                    _ = changed => {},
                    _ = tokio::time::sleep_until(deadline.into()) => {},
                },
                None => changed.await,
            }
        }
    }
}

pub(super) struct RequestPermit {
    capacity: Arc<RequestCapacity>,
    generation: u64,
    completion: Option<RequestCompletion>,
}

impl RequestPermit {
    pub(super) fn concurrency_rejected(&self, delay: Duration) {
        self.capacity.concurrency_rejected(self.generation, delay);
    }

    // Transport capacity is released when the handler ends, even if nobody
    // drains the response. Success feedback must additionally verify the
    // normalized terminal chunk and absence of any later stream error.
    pub(super) fn take_completion(&mut self) -> RequestCompletion {
        self.completion
            .take()
            .expect("request completion taken once")
    }
}

impl Drop for RequestPermit {
    fn drop(&mut self) {
        let mut state = self.capacity.state.lock().unwrap();
        state.active -= 1;
        state.last_activity = Instant::now();
        drop(state);
        self.capacity.changed.notify_one();
    }
}

pub(super) struct RequestCompletion {
    capacity: Arc<RequestCapacity>,
    generation: u64,
    successful: bool,
}

impl RequestCompletion {
    pub(super) fn succeeded(mut self) {
        self.successful = true;
    }
}

impl Drop for RequestCompletion {
    fn drop(&mut self) {
        // HTTP errors, interrupted streams, and cancellation release capacity
        // but reset recovery progress; they are not successful samples.
        self.capacity.completed(self.generation, self.successful);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    fn reject(scope: &RequestCapacity, delay: Duration) {
        let generation = scope.state.lock().unwrap().generation;
        scope.concurrency_rejected(generation, delay);
    }

    async fn successful_window(scope: &Arc<RequestCapacity>, size: usize) {
        let mut requests = Vec::new();
        for _ in 0..size {
            requests.push(scope.acquire().await);
        }
        for mut request in requests {
            request.take_completion().succeeded();
        }
    }

    async fn recover_to(scope: &Arc<RequestCapacity>, target: usize) {
        reject(scope, Duration::ZERO);
        for limit in 1..target {
            for _ in 0..SUCCESS_WINDOWS_TO_GROW {
                successful_window(scope, limit).await;
            }
            assert_eq!(scope.state.lock().unwrap().limit, Some(limit + 1));
        }
    }

    #[tokio::test]
    async fn successful_windows_grow_one_slot_at_a_time_without_idle() {
        let scope = Arc::new(RequestCapacity::new());
        reject(&scope, Duration::ZERO);
        for limit in 1..5 {
            successful_window(&scope, limit).await;
            assert_eq!(scope.state.lock().unwrap().limit, Some(limit));
            successful_window(&scope, limit).await;
            assert_eq!(scope.state.lock().unwrap().limit, Some(limit + 1));
        }
    }

    #[tokio::test]
    async fn low_demand_does_not_inflate_the_recovered_limit() {
        let scope = Arc::new(RequestCapacity::new());
        recover_to(&scope, 2).await;
        for _ in 0..12 {
            successful_window(&scope, 1).await;
        }
        assert_eq!(scope.state.lock().unwrap().limit, Some(2));
    }

    #[tokio::test]
    async fn rejection_halves_once_per_generation_and_ignores_old_successes() {
        let scope = Arc::new(RequestCapacity::new());
        recover_to(&scope, 4).await;
        let first = scope.acquire().await;
        let second = scope.acquire().await;
        let mut old_success = scope.acquire().await;
        let mut another_old_success = scope.acquire().await;
        first.concurrency_rejected(Duration::ZERO);
        assert_eq!(scope.state.lock().unwrap().limit, Some(2));
        second.concurrency_rejected(Duration::ZERO);
        assert_eq!(scope.state.lock().unwrap().limit, Some(2));
        old_success.take_completion().succeeded();
        another_old_success.take_completion().succeeded();
        assert_eq!(scope.state.lock().unwrap().successes, 0);
        let mut queued = Box::pin(scope.acquire());
        assert!(timeout(Duration::from_millis(20), &mut queued)
            .await
            .is_err());
        // Existing streams are not killed when the window shrinks. Wait until
        // active requests fall below the new limit before admitting another.
        drop((first, second));
        assert!(timeout(Duration::from_millis(20), &mut queued)
            .await
            .is_err());
        drop(old_success);
        let retry = timeout(Duration::from_millis(100), queued).await.unwrap();
        drop(another_old_success);
        retry.concurrency_rejected(Duration::ZERO);
        assert_eq!(scope.state.lock().unwrap().limit, Some(1));
        drop(retry);
        successful_window(&scope, 1).await;
        successful_window(&scope, 1).await;
        assert_eq!(scope.state.lock().unwrap().limit, Some(2));
    }

    #[tokio::test]
    async fn failed_or_cancelled_requests_reset_success_progress_and_release_capacity() {
        let scope = Arc::new(RequestCapacity::new());
        reject(&scope, Duration::ZERO);
        successful_window(&scope, 1).await;
        assert_eq!(scope.state.lock().unwrap().successes, 1);
        drop(scope.acquire().await);
        assert_eq!(scope.state.lock().unwrap().active, 0);
        assert_eq!(scope.state.lock().unwrap().successes, 0);
        successful_window(&scope, 1).await;
        assert_eq!(scope.state.lock().unwrap().limit, Some(1));
        successful_window(&scope, 1).await;
        assert_eq!(scope.state.lock().unwrap().limit, Some(2));
    }

    #[tokio::test]
    async fn a_late_batch_rejection_still_extends_provider_cooldown() {
        let scope = Arc::new(RequestCapacity::new());
        let first = scope.acquire().await;
        let second = scope.acquire().await;
        first.concurrency_rejected(Duration::ZERO);
        second.concurrency_rejected(Duration::from_millis(60));
        drop((first, second));
        assert!(timeout(Duration::from_millis(20), scope.acquire())
            .await
            .is_err());
        let _permit = timeout(Duration::from_millis(200), scope.acquire())
            .await
            .unwrap();
        assert_eq!(scope.state.lock().unwrap().limit, Some(1));
    }

    #[tokio::test]
    async fn growth_wakes_a_queued_request_while_another_stream_is_active() {
        let scope = Arc::new(RequestCapacity::new());
        reject(&scope, Duration::ZERO);
        successful_window(&scope, 1).await;
        let mut active = scope.acquire().await;
        let mut queued = Box::pin(scope.acquire());
        assert!(timeout(Duration::from_millis(20), &mut queued)
            .await
            .is_err());
        active.take_completion().succeeded();
        let _admitted = timeout(Duration::from_millis(100), queued).await.unwrap();
        assert_eq!(scope.state.lock().unwrap().active, 2);
    }

    #[tokio::test]
    async fn normal_requests_remain_parallel() {
        let scope = Arc::new(RequestCapacity::new());
        let _first = scope.acquire().await;
        let _second = timeout(Duration::from_millis(100), scope.acquire())
            .await
            .unwrap();
        assert_eq!(scope.state.lock().unwrap().active, 2);
    }

    #[tokio::test]
    async fn rejected_scope_waits_for_active_stream_and_cancellation_does_not_leak() {
        let scope = Arc::new(RequestCapacity::new());
        let first = scope.acquire().await;
        reject(&scope, Duration::ZERO);
        assert!(timeout(Duration::from_millis(20), scope.acquire())
            .await
            .is_err());
        assert_eq!(scope.state.lock().unwrap().active, 1);
        drop(first);
        let next = timeout(Duration::from_millis(100), scope.acquire())
            .await
            .unwrap();
        assert_eq!(scope.state.lock().unwrap().active, 1);
        drop(next);
        assert_eq!(scope.state.lock().unwrap().active, 0);
    }

    #[tokio::test]
    async fn provider_cooldown_is_honored_even_without_local_active_requests() {
        let scope = Arc::new(RequestCapacity::new());
        reject(&scope, Duration::from_millis(60));
        assert!(timeout(Duration::from_millis(20), scope.acquire())
            .await
            .is_err());
        let _permit = timeout(Duration::from_millis(200), scope.acquire())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn learned_limit_expires_after_idle_period() {
        let scope = Arc::new(RequestCapacity::new());
        reject(&scope, Duration::ZERO);
        scope.state.lock().unwrap().last_activity = Instant::now() - IDLE_SCOPE_RETENTION;
        let _first = scope.acquire().await;
        let _second = timeout(Duration::from_millis(100), scope.acquire())
            .await
            .unwrap();
    }

    #[test]
    fn credential_scope_is_shared_across_models_but_isolated_across_accounts_and_hosts() {
        let client = crate::client::http::create_http_client(None, false);
        let scope =
            |url, key| RequestCapacity::for_request(&client.post(url).bearer_auth(key)).unwrap();
        let first = scope("https://capacity.example/v1/chat/completions", "fixture-a");
        let other_model = scope("https://capacity.example/v1/responses", "fixture-a");
        let other_key = scope("https://capacity.example/v1/chat/completions", "fixture-b");
        let other_host = scope("https://other.example/v1/chat/completions", "fixture-a");
        assert!(Arc::ptr_eq(&first, &other_model));
        assert!(!Arc::ptr_eq(&first, &other_key));
        assert!(!Arc::ptr_eq(&first, &other_host));
        let query_a =
            RequestCapacity::for_request(&client.post("https://capacity.example/v1?key=a"))
                .unwrap();
        let query_b =
            RequestCapacity::for_request(&client.post("https://capacity.example/v1?key=b"))
                .unwrap();
        assert!(!Arc::ptr_eq(&query_a, &query_b));
    }
}
