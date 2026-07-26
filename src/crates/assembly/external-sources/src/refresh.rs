//! Shared asynchronous discovery scheduling for typed external-source providers.

use bitfun_product_domains::external_sources::{ExternalSourceProviderError, ProviderId};
use futures::future::{join_all, BoxFuture, Shared};
use futures::FutureExt;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub(crate) trait DiscoveryRequest: Send + 'static {
    type Result: Clone + Send + Sync + 'static;

    const DIAGNOSTIC_PREFIX: &'static str;
    const PROVIDER_LABEL: &'static str;

    fn provider_id(&self) -> ProviderId;
    fn execute(self) -> Self::Result;
    fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self::Result;
}

type SharedDiscoveryTask<T> = Shared<BoxFuture<'static, T>>;
const MAX_CONCURRENT_DISCOVERY_TASKS: usize = 8;
const MAX_DEFERRED_DISCOVERY_LIFETIME: Duration = Duration::from_secs(30);

fn process_discovery_budget() -> Arc<tokio::sync::Semaphore> {
    static BUDGET: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    Arc::clone(
        BUDGET
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DISCOVERY_TASKS))),
    )
}

struct InFlight<R: DiscoveryRequest> {
    generation: u64,
    task: SharedDiscoveryTask<R::Result>,
    deferred_claimed: bool,
    pending_request: Option<R>,
    abandoned: bool,
}

pub struct DeferredDiscovery<T: Clone> {
    provider_id: ProviderId,
    generation: u64,
    task: SharedDiscoveryTask<T>,
}

pub struct CompletedDeferredDiscovery<T: Clone> {
    provider_id: ProviderId,
    generation: u64,
    result: T,
    release_lane: bool,
}

pub struct DiscoveryBatch<T: Clone> {
    pub immediate: Vec<T>,
    pub deferred: Vec<DeferredDiscovery<T>>,
}

pub(crate) struct DiscoveryLane<R: DiscoveryRequest> {
    tasks: tokio::sync::Mutex<BTreeMap<ProviderId, InFlight<R>>>,
    next_generation: AtomicU64,
    budget: Arc<tokio::sync::Semaphore>,
    deferred_lifetime: Duration,
    request: PhantomData<fn(R)>,
}

impl<R: DiscoveryRequest> Default for DiscoveryLane<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: DiscoveryRequest> DiscoveryLane<R> {
    pub(crate) fn new() -> Self {
        Self {
            tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            next_generation: AtomicU64::new(1),
            budget: process_discovery_budget(),
            deferred_lifetime: MAX_DEFERRED_DISCOVERY_LIFETIME,
            request: PhantomData,
        }
    }

    #[cfg(test)]
    fn with_limits(max_concurrent: usize, deferred_lifetime: Duration) -> Self {
        Self {
            tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            next_generation: AtomicU64::new(1),
            budget: Arc::new(tokio::sync::Semaphore::new(max_concurrent)),
            deferred_lifetime,
            request: PhantomData,
        }
    }

    pub(crate) async fn discover(
        &self,
        requests: Vec<R>,
        timeout: Duration,
    ) -> DiscoveryBatch<R::Result> {
        let mut scheduled = Vec::with_capacity(requests.len());
        {
            let mut tasks = self.tasks.lock().await;
            for request in requests {
                let provider_id = request.provider_id();
                if let Some(in_flight) = tasks.get_mut(&provider_id) {
                    // Coalesce repeated watcher/manual refreshes to the newest
                    // request, but never publish the generation that started
                    // before the newest request was observed.
                    in_flight.pending_request = Some(request);
                    scheduled.push(Scheduled {
                        provider_id,
                        generation: in_flight.generation,
                        task: in_flight.task.clone(),
                        is_new: false,
                        abandoned: in_flight.abandoned,
                    });
                    continue;
                }
                let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
                let task = spawn_discovery_task::<R>(
                    request,
                    provider_id.clone(),
                    Arc::clone(&self.budget),
                );
                tasks.insert(
                    provider_id.clone(),
                    InFlight {
                        generation,
                        task: task.clone(),
                        deferred_claimed: false,
                        pending_request: None,
                        abandoned: false,
                    },
                );
                scheduled.push(Scheduled {
                    provider_id,
                    generation,
                    task,
                    is_new: true,
                    abandoned: false,
                });
            }
        }

        let polled = join_all(scheduled.into_iter().map(|scheduled| async move {
            if !scheduled.is_new {
                return match scheduled.task.clone().now_or_never() {
                    Some(result) => RawPoll::Complete {
                        provider_id: scheduled.provider_id,
                        generation: scheduled.generation,
                        result,
                    },
                    None => RawPoll::InFlight {
                        provider_id: scheduled.provider_id,
                        generation: scheduled.generation,
                        abandoned: scheduled.abandoned,
                    },
                };
            }
            match tokio::time::timeout(timeout, scheduled.task.clone()).await {
                Ok(result) => RawPoll::Complete {
                    provider_id: scheduled.provider_id,
                    generation: scheduled.generation,
                    result,
                },
                Err(_) => RawPoll::TimedOut {
                    provider_id: scheduled.provider_id,
                    generation: scheduled.generation,
                    task: scheduled.task,
                },
            }
        }))
        .await;

        let mut immediate = Vec::with_capacity(polled.len());
        let mut deferred = Vec::new();
        let mut tasks = self.tasks.lock().await;
        for poll in polled {
            match poll {
                RawPoll::Complete {
                    provider_id,
                    generation,
                    result,
                } => {
                    if let Some(request) =
                        take_pending_request(&mut tasks, &provider_id, generation)
                    {
                        let next_generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
                        let task = spawn_discovery_task::<R>(
                            request,
                            provider_id.clone(),
                            Arc::clone(&self.budget),
                        );
                        tasks.insert(
                            provider_id.clone(),
                            InFlight {
                                generation: next_generation,
                                task: task.clone(),
                                deferred_claimed: true,
                                pending_request: None,
                                abandoned: false,
                            },
                        );
                        deferred.push(DeferredDiscovery {
                            provider_id: provider_id.clone(),
                            generation: next_generation,
                            task,
                        });
                        immediate.push(R::failed(
                            provider_id,
                            discovery_error::<R>(
                                "discovery_in_progress",
                                format!(
                                    "{} provider changed during discovery; checking its newest version",
                                    R::PROVIDER_LABEL
                                ),
                            ),
                        ));
                    } else if remove_generation(&mut tasks, &provider_id, generation) {
                        immediate.push(result);
                    }
                }
                RawPoll::InFlight {
                    provider_id,
                    generation,
                    abandoned,
                } => {
                    if generation_is_current(&tasks, &provider_id, generation) {
                        let (suffix, message) = if abandoned {
                            (
                                "discovery_abandoned",
                                format!(
                                    "{} provider discovery exceeded its maximum background lifetime and remains isolated until its worker exits",
                                    R::PROVIDER_LABEL
                                ),
                            )
                        } else {
                            (
                                "discovery_in_progress",
                                format!(
                                    "{} provider discovery is still running; using its last valid version",
                                    R::PROVIDER_LABEL
                                ),
                            )
                        };
                        immediate.push(R::failed(
                            provider_id,
                            discovery_error::<R>(suffix, message),
                        ));
                    }
                }
                RawPoll::TimedOut {
                    provider_id,
                    generation,
                    task,
                } => {
                    let claim = tasks
                        .get_mut(&provider_id)
                        .filter(|in_flight| in_flight.generation == generation)
                        .is_some_and(|in_flight| {
                            if in_flight.deferred_claimed {
                                false
                            } else {
                                in_flight.deferred_claimed = true;
                                true
                            }
                        });
                    if claim {
                        deferred.push(DeferredDiscovery {
                            provider_id: provider_id.clone(),
                            generation,
                            task,
                        });
                    }
                    immediate.push(R::failed(
                        provider_id,
                        discovery_error::<R>(
                            "discovery_timeout",
                            format!(
                                "{} provider discovery exceeded the {} ms deadline",
                                R::PROVIDER_LABEL,
                                timeout.as_millis()
                            ),
                        ),
                    ));
                }
            }
        }
        DiscoveryBatch {
            immediate,
            deferred,
        }
    }

    pub(crate) async fn complete_deferred(
        &self,
        mut deferred: DeferredDiscovery<R::Result>,
    ) -> Option<(
        CompletedDeferredDiscovery<R::Result>,
        Option<DeferredDiscovery<R::Result>>,
    )> {
        loop {
            let provider_id = deferred.provider_id.clone();
            let generation = deferred.generation;
            let result = match tokio::time::timeout(self.deferred_lifetime, deferred.task.clone())
                .await
            {
                Ok(result) => result,
                Err(_) => {
                    let mut tasks = self.tasks.lock().await;
                    let in_flight = tasks
                        .get_mut(&provider_id)
                        .filter(|in_flight| in_flight.generation == generation)?;
                    // `spawn_blocking` cannot be cancelled safely. Keep this
                    // generation as a tombstone so repeated refreshes cannot
                    // accumulate workers or process-wide permits. A later
                    // refresh may replace it only after the worker really exits.
                    in_flight.abandoned = true;
                    return Some((
                        CompletedDeferredDiscovery {
                            provider_id: provider_id.clone(),
                            generation,
                            result: R::failed(
                                provider_id,
                                discovery_error::<R>(
                                    "discovery_abandoned",
                                    format!(
                                        "{} provider discovery exceeded its maximum background lifetime",
                                        R::PROVIDER_LABEL
                                    ),
                                ),
                            ),
                            release_lane: false,
                        },
                        Some(deferred),
                    ));
                }
            };
            let mut tasks = self.tasks.lock().await;
            if let Some(request) =
                take_pending_request(&mut tasks, &deferred.provider_id, deferred.generation)
            {
                let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
                let task = spawn_discovery_task::<R>(
                    request,
                    deferred.provider_id.clone(),
                    Arc::clone(&self.budget),
                );
                tasks.insert(
                    deferred.provider_id.clone(),
                    InFlight {
                        generation,
                        task: task.clone(),
                        deferred_claimed: true,
                        pending_request: None,
                        abandoned: false,
                    },
                );
                deferred = DeferredDiscovery {
                    provider_id: deferred.provider_id,
                    generation,
                    task,
                };
                drop(tasks);
                continue;
            }
            if !generation_is_current(&tasks, &deferred.provider_id, deferred.generation) {
                return None;
            }
            return Some((
                CompletedDeferredDiscovery {
                    provider_id: deferred.provider_id,
                    generation: deferred.generation,
                    result,
                    release_lane: true,
                },
                None,
            ));
        }
    }

    pub(crate) async fn resume_abandoned(
        &self,
        deferred: DeferredDiscovery<R::Result>,
    ) -> Option<DeferredDiscovery<R::Result>> {
        let provider_id = deferred.provider_id;
        let generation = deferred.generation;
        // `spawn_blocking` cannot be cancelled. This is the single observer for
        // the shared task: it retains no additional semaphore permit and only
        // releases the provider lane after the original worker really exits.
        let _ = deferred.task.await;

        let mut tasks = self.tasks.lock().await;
        let in_flight = tasks
            .get(&provider_id)
            .filter(|in_flight| in_flight.generation == generation && in_flight.abandoned)?;
        if in_flight.pending_request.is_none() {
            tasks.remove(&provider_id);
            return None;
        }
        let request = take_pending_request(&mut tasks, &provider_id, generation)?;
        let next_generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let task =
            spawn_discovery_task::<R>(request, provider_id.clone(), Arc::clone(&self.budget));
        tasks.insert(
            provider_id.clone(),
            InFlight {
                generation: next_generation,
                task: task.clone(),
                deferred_claimed: true,
                pending_request: None,
                abandoned: false,
            },
        );
        Some(DeferredDiscovery {
            provider_id,
            generation: next_generation,
            task,
        })
    }

    pub(crate) async fn finalize_deferred(
        &self,
        completed: CompletedDeferredDiscovery<R::Result>,
    ) -> Option<R::Result> {
        let mut tasks = self.tasks.lock().await;
        if !generation_is_current(&tasks, &completed.provider_id, completed.generation) {
            return None;
        }
        if completed.release_lane {
            tasks.remove(&completed.provider_id);
        }
        Some(completed.result)
    }

    pub(crate) async fn has_in_flight(&self) -> bool {
        !self.tasks.lock().await.is_empty()
    }

    #[cfg(test)]
    async fn cancel(&self, provider_id: &str) -> bool {
        let mut tasks = self.tasks.lock().await;
        let Some(key) = tasks
            .keys()
            .find(|candidate| candidate.as_str() == provider_id)
            .cloned()
        else {
            return false;
        };
        tasks.remove(&key).is_some()
    }
}

struct Scheduled<T: Clone> {
    provider_id: ProviderId,
    generation: u64,
    task: SharedDiscoveryTask<T>,
    is_new: bool,
    abandoned: bool,
}

enum RawPoll<T: Clone> {
    Complete {
        provider_id: ProviderId,
        generation: u64,
        result: T,
    },
    InFlight {
        provider_id: ProviderId,
        generation: u64,
        abandoned: bool,
    },
    TimedOut {
        provider_id: ProviderId,
        generation: u64,
        task: SharedDiscoveryTask<T>,
    },
}

fn discovery_error<R: DiscoveryRequest>(
    suffix: &str,
    message: String,
) -> ExternalSourceProviderError {
    ExternalSourceProviderError::new(
        format!("{}.{}", R::DIAGNOSTIC_PREFIX, suffix),
        message,
        true,
    )
}

fn spawn_discovery_task<R: DiscoveryRequest>(
    request: R,
    provider_id: ProviderId,
    budget: Arc<tokio::sync::Semaphore>,
) -> SharedDiscoveryTask<R::Result> {
    async move {
        let permit = match budget.try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                return R::failed(
                    provider_id,
                    discovery_error::<R>(
                        "discovery_overloaded",
                        format!(
                            "{} provider discovery could not start because the process-wide discovery budget is full",
                            R::PROVIDER_LABEL
                        ),
                    ),
                );
            }
        };
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            request.execute()
        })
        .await
        {
            Ok(result) => result,
            Err(error) => R::failed(
                provider_id,
                discovery_error::<R>(
                    "discovery_task_failed",
                    format!(
                        "{} provider discovery task failed: {error}",
                        R::PROVIDER_LABEL
                    ),
                ),
            ),
        }
    }
    .boxed()
    .shared()
}

fn generation_is_current<R: DiscoveryRequest>(
    tasks: &BTreeMap<ProviderId, InFlight<R>>,
    provider_id: &ProviderId,
    generation: u64,
) -> bool {
    tasks
        .get(provider_id)
        .is_some_and(|in_flight| in_flight.generation == generation)
}

fn take_pending_request<R: DiscoveryRequest>(
    tasks: &mut BTreeMap<ProviderId, InFlight<R>>,
    provider_id: &ProviderId,
    generation: u64,
) -> Option<R> {
    tasks
        .get_mut(provider_id)
        .filter(|in_flight| in_flight.generation == generation)
        .and_then(|in_flight| in_flight.pending_request.take())
}

fn remove_generation<R: DiscoveryRequest>(
    tasks: &mut BTreeMap<ProviderId, InFlight<R>>,
    provider_id: &ProviderId,
    generation: u64,
) -> bool {
    if generation_is_current(tasks, provider_id, generation) {
        tasks.remove(provider_id);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{DiscoveryLane, DiscoveryRequest};
    use bitfun_product_domains::external_sources::{ExternalSourceProviderError, ProviderId};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FakeResultKind {
        Success,
        InFlight,
        TimedOut,
        Abandoned,
        Overloaded,
        TaskFailed,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FakeResult {
        provider_id: ProviderId,
        kind: FakeResultKind,
        version: u64,
    }

    struct FakeRequest {
        provider_id: ProviderId,
        released: Arc<AtomicBool>,
        version: u64,
    }

    impl FakeRequest {
        fn blocked(provider_id: &str, released: Arc<AtomicBool>) -> Self {
            Self {
                provider_id: ProviderId::new(provider_id).unwrap(),
                released,
                version: 1,
            }
        }

        fn blocked_version(provider_id: &str, released: Arc<AtomicBool>, version: u64) -> Self {
            Self {
                provider_id: ProviderId::new(provider_id).unwrap(),
                released,
                version,
            }
        }
    }

    impl DiscoveryRequest for FakeRequest {
        type Result = FakeResult;

        const DIAGNOSTIC_PREFIX: &'static str = "fake";
        const PROVIDER_LABEL: &'static str = "fake";

        fn provider_id(&self) -> ProviderId {
            self.provider_id.clone()
        }

        fn execute(self) -> Self::Result {
            while !self.released.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            FakeResult {
                provider_id: self.provider_id,
                kind: FakeResultKind::Success,
                version: self.version,
            }
        }

        fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self::Result {
            let kind = match error.code.as_str() {
                "fake.discovery_in_progress" => FakeResultKind::InFlight,
                "fake.discovery_timeout" => FakeResultKind::TimedOut,
                "fake.discovery_abandoned" => FakeResultKind::Abandoned,
                "fake.discovery_overloaded" => FakeResultKind::Overloaded,
                "fake.discovery_task_failed" => FakeResultKind::TaskFailed,
                code => panic!("unexpected failure code: {code}"),
            };
            FakeResult {
                provider_id,
                kind,
                version: 0,
            }
        }
    }

    #[tokio::test]
    async fn timed_out_request_is_reused_and_completes_once() {
        let lane = DiscoveryLane::<FakeRequest>::new();
        let released = Arc::new(AtomicBool::new(false));

        let first = lane
            .discover(
                vec![FakeRequest::blocked("provider", Arc::clone(&released))],
                Duration::from_millis(10),
            )
            .await;
        assert_eq!(first.immediate[0].kind, FakeResultKind::TimedOut);
        assert_eq!(first.deferred.len(), 1);

        let second = lane
            .discover(
                vec![FakeRequest::blocked("provider", Arc::clone(&released))],
                Duration::from_millis(10),
            )
            .await;
        assert_eq!(second.immediate[0].kind, FakeResultKind::InFlight);
        assert!(second.deferred.is_empty());

        released.store(true, Ordering::Release);
        let completed = lane
            .complete_deferred(first.deferred.into_iter().next().unwrap())
            .await
            .expect("the original generation completes once")
            .0;
        let completed = lane
            .finalize_deferred(completed)
            .await
            .expect("the original generation is still current at publication");
        assert_eq!(completed.kind, FakeResultKind::Success);

        let third = lane
            .discover(
                vec![FakeRequest::blocked("provider", released)],
                Duration::from_millis(10),
            )
            .await;
        assert_eq!(third.immediate[0].kind, FakeResultKind::Success);
    }

    #[tokio::test]
    async fn stale_deferred_completion_does_not_remove_a_newer_generation() {
        let lane = DiscoveryLane::<FakeRequest>::new();
        let first_release = Arc::new(AtomicBool::new(false));
        let first = lane
            .discover(
                vec![FakeRequest::blocked("provider", Arc::clone(&first_release))],
                Duration::from_millis(10),
            )
            .await;
        lane.cancel("provider").await;

        let second_release = Arc::new(AtomicBool::new(false));
        let second = lane
            .discover(
                vec![FakeRequest::blocked(
                    "provider",
                    Arc::clone(&second_release),
                )],
                Duration::from_millis(10),
            )
            .await;

        first_release.store(true, Ordering::Release);
        assert!(lane
            .complete_deferred(first.deferred.into_iter().next().unwrap())
            .await
            .is_none());

        let reused = lane
            .discover(
                vec![FakeRequest::blocked(
                    "provider",
                    Arc::clone(&second_release),
                )],
                Duration::from_millis(10),
            )
            .await;
        assert_eq!(reused.immediate[0].kind, FakeResultKind::InFlight);

        second_release.store(true, Ordering::Release);
        let completed = lane
            .complete_deferred(second.deferred.into_iter().next().unwrap())
            .await
            .expect("new generation remains registered")
            .0;
        let completed = lane
            .finalize_deferred(completed)
            .await
            .expect("new generation remains current at publication");
        assert_eq!(completed.kind, FakeResultKind::Success);
    }

    #[tokio::test]
    async fn refresh_during_in_flight_discovery_publishes_only_the_newest_request() {
        let lane = DiscoveryLane::<FakeRequest>::new();
        let first_release = Arc::new(AtomicBool::new(false));
        let first = lane
            .discover(
                vec![FakeRequest::blocked_version(
                    "provider",
                    Arc::clone(&first_release),
                    1,
                )],
                Duration::from_millis(10),
            )
            .await;
        assert_eq!(first.immediate[0].kind, FakeResultKind::TimedOut);

        let second_release = Arc::new(AtomicBool::new(true));
        let second = lane
            .discover(
                vec![FakeRequest::blocked_version("provider", second_release, 2)],
                Duration::from_millis(10),
            )
            .await;
        assert_eq!(second.immediate[0].kind, FakeResultKind::InFlight);

        first_release.store(true, Ordering::Release);
        let completed = lane
            .complete_deferred(first.deferred.into_iter().next().unwrap())
            .await
            .expect("the dirty lane reruns its newest request")
            .0;
        let completed = lane
            .finalize_deferred(completed)
            .await
            .expect("the newest request remains current at publication");
        assert_eq!(completed.version, 2);
    }

    #[tokio::test]
    async fn deferred_completion_is_rechecked_at_the_publication_boundary() {
        let lane = DiscoveryLane::<FakeRequest>::new();
        let first_release = Arc::new(AtomicBool::new(false));
        let first = lane
            .discover(
                vec![FakeRequest::blocked_version(
                    "provider",
                    Arc::clone(&first_release),
                    1,
                )],
                Duration::from_millis(10),
            )
            .await;

        first_release.store(true, Ordering::Release);
        let stale_completion = lane
            .complete_deferred(first.deferred.into_iter().next().unwrap())
            .await
            .expect("first generation finishes before publication")
            .0;

        let newer = lane
            .discover(
                vec![FakeRequest::blocked_version(
                    "provider",
                    Arc::new(AtomicBool::new(true)),
                    2,
                )],
                Duration::from_millis(10),
            )
            .await;
        assert_eq!(newer.immediate[0].kind, FakeResultKind::InFlight);
        assert!(lane.finalize_deferred(stale_completion).await.is_none());

        let newer_completion = lane
            .complete_deferred(newer.deferred.into_iter().next().unwrap())
            .await
            .expect("newer generation finishes")
            .0;
        let newer_result = lane
            .finalize_deferred(newer_completion)
            .await
            .expect("newer generation is published");
        assert_eq!(newer_result.version, 2);
    }

    #[tokio::test]
    async fn permanently_blocked_discovery_keeps_one_provider_tombstone() {
        let lane = DiscoveryLane::<FakeRequest>::with_limits(1, Duration::from_millis(20));
        let blocked_release = Arc::new(AtomicBool::new(false));
        let first = lane
            .discover(
                vec![FakeRequest::blocked(
                    "blocked",
                    Arc::clone(&blocked_release),
                )],
                Duration::from_millis(5),
            )
            .await;
        let (expired, observer) = lane
            .complete_deferred(first.deferred.into_iter().next().unwrap())
            .await
            .expect("expired discovery publishes a bounded failure");
        let expired = lane
            .finalize_deferred(expired)
            .await
            .expect("the abandoned generation remains current");
        assert_eq!(expired.kind, FakeResultKind::Abandoned);

        let repeated = lane
            .discover(
                vec![FakeRequest::blocked_version(
                    "blocked",
                    Arc::clone(&blocked_release),
                    3,
                )],
                Duration::from_millis(5),
            )
            .await;
        assert_eq!(repeated.immediate[0].kind, FakeResultKind::Abandoned);
        assert!(repeated.deferred.is_empty());

        let second = lane
            .discover(
                vec![FakeRequest::blocked(
                    "replacement",
                    Arc::new(AtomicBool::new(true)),
                )],
                Duration::from_millis(5),
            )
            .await;
        assert_eq!(second.immediate[0].kind, FakeResultKind::Overloaded);
        assert!(second.deferred.is_empty());

        blocked_release.store(true, Ordering::Release);
        let resumed = lane
            .resume_abandoned(observer.expect("one exit observer is retained"))
            .await;
        let resumed = resumed.expect("the pending generation starts when the old worker exits");
        let resumed = lane
            .complete_deferred(resumed)
            .await
            .expect("the newest pending request runs after the abandoned worker exits")
            .0;
        let resumed = lane
            .finalize_deferred(resumed)
            .await
            .expect("the resumed generation remains current");
        assert_eq!(resumed.version, 3);
    }
}
