//! Admission control for public Page Function execution.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const MAX_PAGE_FUNCTION_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const GLOBAL_CONCURRENCY: usize = 64;
const USER_CONCURRENCY: usize = 16;
const PAGE_CONCURRENCY: usize = 8;
const USER_REQUESTS_PER_WINDOW: usize = 3_000;
const PAGE_REQUESTS_PER_WINDOW: usize = 600;
const RATE_WINDOW: Duration = Duration::from_secs(60);
const MAX_TRACKED_IDENTITIES: usize = 10_000;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PageExecutionRejection {
    Busy,
    RateLimited,
}

#[derive(Clone, Copy)]
struct Limits {
    global_concurrency: usize,
    user_concurrency: usize,
    page_concurrency: usize,
    user_requests_per_window: usize,
    page_requests_per_window: usize,
    rate_window: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            global_concurrency: GLOBAL_CONCURRENCY,
            user_concurrency: USER_CONCURRENCY,
            page_concurrency: PAGE_CONCURRENCY,
            user_requests_per_window: USER_REQUESTS_PER_WINDOW,
            page_requests_per_window: PAGE_REQUESTS_PER_WINDOW,
            rate_window: RATE_WINDOW,
        }
    }
}

/// Process-local guard against one account or page exhausting blocking workers.
pub struct PageExecutionGuard {
    limits: Limits,
    global: Arc<Semaphore>,
    users: DashMap<String, Arc<Semaphore>>,
    pages: DashMap<String, Arc<Semaphore>>,
    rate_windows: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Default for PageExecutionGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl PageExecutionGuard {
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    fn with_limits(limits: Limits) -> Self {
        Self {
            global: Arc::new(Semaphore::new(limits.global_concurrency)),
            users: DashMap::new(),
            pages: DashMap::new(),
            rate_windows: Mutex::new(HashMap::new()),
            limits,
        }
    }

    pub fn try_acquire(
        &self,
        user_id: &str,
        slug: &str,
    ) -> Result<PageExecutionPermit, PageExecutionRejection> {
        self.check_rate(user_id, slug)?;

        let global = Arc::clone(&self.global)
            .try_acquire_owned()
            .map_err(|_| PageExecutionRejection::Busy)?;
        let user = self.user_semaphore(user_id);
        let user = user
            .try_acquire_owned()
            .map_err(|_| PageExecutionRejection::Busy)?;
        let page = self.page_semaphore(user_id, slug);
        let page = page
            .try_acquire_owned()
            .map_err(|_| PageExecutionRejection::Busy)?;

        Ok(PageExecutionPermit {
            _global: global,
            _user: user,
            _page: page,
        })
    }

    fn user_semaphore(&self, user_id: &str) -> Arc<Semaphore> {
        self.prune_idle_semaphores();
        Arc::clone(
            self.users
                .entry(user_id.to_string())
                .or_insert_with(|| Arc::new(Semaphore::new(self.limits.user_concurrency)))
                .value(),
        )
    }

    fn page_semaphore(&self, user_id: &str, slug: &str) -> Arc<Semaphore> {
        self.prune_idle_semaphores();
        let key = format!("{user_id}\0{slug}");
        Arc::clone(
            self.pages
                .entry(key)
                .or_insert_with(|| Arc::new(Semaphore::new(self.limits.page_concurrency)))
                .value(),
        )
    }

    fn prune_idle_semaphores(&self) {
        if self.users.len() > MAX_TRACKED_IDENTITIES {
            self.users
                .retain(|_, semaphore| Arc::strong_count(semaphore) > 1);
        }
        if self.pages.len() > MAX_TRACKED_IDENTITIES {
            self.pages
                .retain(|_, semaphore| Arc::strong_count(semaphore) > 1);
        }
    }

    fn check_rate(&self, user_id: &str, slug: &str) -> Result<(), PageExecutionRejection> {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.limits.rate_window).unwrap_or(now);
        let mut windows = self
            .rate_windows
            .lock()
            .map_err(|_| PageExecutionRejection::Busy)?;
        if windows.len() > MAX_TRACKED_IDENTITIES * 2 {
            windows.retain(|_, entries| {
                entries.retain(|timestamp| *timestamp > cutoff);
                !entries.is_empty()
            });
        }

        record_request(
            &mut windows,
            format!("user\0{user_id}"),
            cutoff,
            now,
            self.limits.user_requests_per_window,
        )?;
        if let Err(error) = record_request(
            &mut windows,
            format!("page\0{user_id}\0{slug}"),
            cutoff,
            now,
            self.limits.page_requests_per_window,
        ) {
            if let Some(user_window) = windows.get_mut(&format!("user\0{user_id}")) {
                if user_window.back() == Some(&now) {
                    user_window.pop_back();
                }
            }
            return Err(error);
        }
        Ok(())
    }
}

fn record_request(
    windows: &mut HashMap<String, VecDeque<Instant>>,
    key: String,
    cutoff: Instant,
    now: Instant,
    limit: usize,
) -> Result<(), PageExecutionRejection> {
    let entries = windows.entry(key).or_default();
    while entries
        .front()
        .is_some_and(|timestamp| *timestamp <= cutoff)
    {
        entries.pop_front();
    }
    if entries.len() >= limit {
        return Err(PageExecutionRejection::RateLimited);
    }
    entries.push_back(now);
    Ok(())
}

#[derive(Debug)]
pub struct PageExecutionPermit {
    _global: OwnedSemaphorePermit,
    _user: OwnedSemaphorePermit,
    _page: OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_guard() -> PageExecutionGuard {
        PageExecutionGuard::with_limits(Limits {
            global_concurrency: 3,
            user_concurrency: 2,
            page_concurrency: 1,
            user_requests_per_window: 100,
            page_requests_per_window: 100,
            rate_window: Duration::from_secs(60),
        })
    }

    #[test]
    fn page_concurrency_is_isolated_and_permits_recover() {
        let guard = test_guard();
        let first = guard.try_acquire("user", "one").unwrap();
        assert_eq!(
            guard.try_acquire("user", "one").unwrap_err(),
            PageExecutionRejection::Busy
        );
        let second_page = guard.try_acquire("user", "two").unwrap();
        assert_eq!(
            guard.try_acquire("user", "three").unwrap_err(),
            PageExecutionRejection::Busy
        );
        drop((first, second_page));
        assert!(guard.try_acquire("user", "one").is_ok());
    }

    #[test]
    fn page_and_account_rate_windows_are_enforced() {
        let guard = PageExecutionGuard::with_limits(Limits {
            global_concurrency: 3,
            user_concurrency: 2,
            page_concurrency: 1,
            user_requests_per_window: 4,
            page_requests_per_window: 2,
            rate_window: Duration::from_secs(60),
        });
        drop(guard.try_acquire("user", "one").unwrap());
        drop(guard.try_acquire("user", "one").unwrap());
        assert_eq!(
            guard.try_acquire("user", "one").unwrap_err(),
            PageExecutionRejection::RateLimited
        );

        drop(guard.try_acquire("user", "two").unwrap());
        drop(guard.try_acquire("user", "two").unwrap());
        assert_eq!(
            guard.try_acquire("user", "three").unwrap_err(),
            PageExecutionRejection::RateLimited
        );
        assert!(guard.try_acquire("other", "one").is_ok());
    }
}
