use crate::{PlatformPublisher, PublishResult, PublishStatus, VideoAsset};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

/// Publish orchestrator: multi-platform parallel publishing + exponential backoff retry.
pub struct PublishScheduler {
    /// Registered publishing platforms
    publishers: Vec<Arc<dyn PlatformPublisher>>,
    /// Maximum concurrent publishes
    max_concurrent: usize,
    /// Maximum retry count (per platform)
    max_retries: u32,
    /// Initial backoff interval
    base_backoff: Duration,
}

/// Per-platform publish attempt record.
#[derive(Debug, Clone)]
pub struct PublishAttempt {
    pub platform: String,
    pub attempt: u32,
    pub result: Result<PublishResult, String>,
    pub elapsed: Duration,
}

impl PublishScheduler {
    pub fn new(publishers: Vec<Box<dyn PlatformPublisher>>) -> Self {
        Self {
            publishers: publishers.into_iter().map(Arc::from).collect(),
            max_concurrent: 3,
            max_retries: 3,
            base_backoff: Duration::from_secs(1),
        }
    }

    /// Set maximum concurrency.
    pub fn with_max_concurrent(mut self, n: usize) -> Self {
        self.max_concurrent = n;
        self
    }

    /// Set retry parameters.
    pub fn with_retry(mut self, max_retries: u32, base_backoff: Duration) -> Self {
        self.max_retries = max_retries;
        self.base_backoff = base_backoff;
        self
    }

    /// Compute backoff interval for the nth retry: base_backoff * 2^(n-1)
    pub fn backoff(&self, attempt: u32) -> Duration {
        let multiplier = 2u64.pow(attempt.saturating_sub(1));
        self.base_backoff * multiplier as u32
    }

    /// Publish to a single platform (with retries). Serial version for direct external calls.
    pub async fn publish_to_platform(
        &self,
        publisher: &dyn PlatformPublisher,
        video: &VideoAsset,
    ) -> Vec<PublishAttempt> {
        publish_one(publisher, video, self.max_retries, self.base_backoff).await
    }

    /// Concurrently publish to all registered platforms.
    ///
    /// Uses `tokio::task::JoinSet` for concurrent scheduling, `max_concurrent` controls
    /// maximum parallelism via `Semaphore`.
    pub async fn publish_all(&self, video: &VideoAsset) -> Vec<Vec<PublishAttempt>> {
        let max_concurrent = if self.max_concurrent == 0 {
            1
        } else {
            self.max_concurrent
        };

        if self.publishers.is_empty() {
            return Vec::new();
        }

        let video_arc = Arc::new(video.clone());
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
        let max_retries = self.max_retries;
        let base_backoff = self.base_backoff;

        let mut join_set = JoinSet::new();

        for publisher in &self.publishers {
            let pub_arc = Arc::clone(publisher);
            let v_arc = Arc::clone(&video_arc);
            let sem = Arc::clone(&semaphore);

            join_set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                publish_one(pub_arc.as_ref(), &v_arc, max_retries, base_backoff).await
            });
        }

        let mut results = Vec::new();
        while let Some(outcome) = join_set.join_next().await {
            match outcome {
                Ok(attempts) => results.push(attempts),
                Err(e) => {
                    results.push(vec![PublishAttempt {
                        platform: "unknown".into(),
                        attempt: 1,
                        result: Err(format!("spawn error: {}", e)),
                        elapsed: Duration::default(),
                    }]);
                }
            }
        }

        results
    }

    /// Summarize publish results.
    pub fn summarize(&self, all_attempts: &[Vec<PublishAttempt>]) -> PublishSummary {
        let mut summary = PublishSummary::default();

        for attempts in all_attempts {
            if let Some(last) = attempts.last() {
                match &last.result {
                    Ok(pr) => match &pr.status {
                        PublishStatus::Uploading { .. }
                        | PublishStatus::Processing
                        | PublishStatus::Published { .. } => {
                            summary.success_count += 1;
                            summary.success_platforms.push(pr.platform.clone());
                        }
                        PublishStatus::Failed { error } => {
                            summary.failed_count += 1;
                            summary
                                .failed_platforms
                                .push((pr.platform.clone(), error.clone()));
                        }
                    },
                    Err(e) => {
                        summary.failed_count += 1;
                        summary.failed_platforms.push((
                            attempts
                                .first()
                                .map(|a| a.platform.clone())
                                .unwrap_or_default(),
                            e.clone(),
                        ));
                    }
                }
            }
        }

        summary
    }
}

/// Execute publishing for a single platform (with retries). Standalone function for use in spawn.
async fn publish_one(
    publisher: &dyn PlatformPublisher,
    video: &VideoAsset,
    max_retries: u32,
    base_backoff: Duration,
) -> Vec<PublishAttempt> {
    let mut attempts = Vec::new();

    for n in 0..=max_retries {
        let start = std::time::Instant::now();

        // Check if authentication is still valid before retrying
        if n > 0 {
            match publisher.check_auth().await {
                Ok(true) => {}
                Ok(false) => {
                    attempts.push(PublishAttempt {
                        platform: publisher.platform_name().into(),
                        attempt: n + 1,
                        result: Ok(PublishResult {
                            platform: publisher.platform_name().into(),
                            publish_id: String::new(),
                            url: None,
                            status: PublishStatus::Failed {
                                error: "Authentication failed, giving up retry".into(),
                            },
                        }),
                        elapsed: start.elapsed(),
                    });
                    break;
                }
                Err(e) => {
                    attempts.push(PublishAttempt {
                        platform: publisher.platform_name().into(),
                        attempt: n + 1,
                        result: Err(e),
                        elapsed: start.elapsed(),
                    });
                    break;
                }
            }
        }

        // Execute upload
        let result = publisher.upload(video).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(pr) => {
                let is_success = !matches!(pr.status, PublishStatus::Failed { .. });
                let attempt = PublishAttempt {
                    platform: publisher.platform_name().into(),
                    attempt: n + 1,
                    result: Ok(pr.clone()),
                    elapsed,
                };
                attempts.push(attempt);

                if is_success {
                    break;
                }
                if n < max_retries {
                    let multiplier = 2u64.pow(n.saturating_sub(1));
                    tokio::time::sleep(base_backoff * multiplier as u32).await;
                }
            }
            Err(_) => {
                attempts.push(PublishAttempt {
                    platform: publisher.platform_name().into(),
                    attempt: n + 1,
                    result,
                    elapsed,
                });
                if n < max_retries {
                    let multiplier = 2u64.pow(n.saturating_sub(1));
                    tokio::time::sleep(base_backoff * multiplier as u32).await;
                }
            }
        }
    }

    attempts
}

/// Publish summary.
#[derive(Debug, Clone, Default)]
pub struct PublishSummary {
    pub success_count: usize,
    pub failed_count: usize,
    pub success_platforms: Vec<String>,
    pub failed_platforms: Vec<(String, String)>,
}

impl PublishSummary {
    pub fn all_success(&self) -> bool {
        self.failed_count == 0
    }

    pub fn any_success(&self) -> bool {
        self.success_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Mock publisher for testing.
    struct MockPublisher {
        name: String,
        should_fail: Mutex<bool>,
    }

    impl MockPublisher {
        fn new(name: &str) -> Self {
            Self {
                name: name.into(),
                should_fail: Mutex::new(false),
            }
        }

        #[allow(dead_code)]
        fn set_fail(&self, fail: bool) {
            *self.should_fail.lock().unwrap() = fail;
        }
    }

    #[async_trait::async_trait]
    impl PlatformPublisher for MockPublisher {
        fn platform_name(&self) -> &str {
            &self.name
        }

        async fn check_auth(&self) -> Result<bool, String> {
            Ok(true)
        }

        async fn upload(&self, _video: &VideoAsset) -> Result<PublishResult, String> {
            let fail = *self.should_fail.lock().unwrap();
            if fail {
                Ok(PublishResult {
                    platform: self.name.clone(),
                    publish_id: String::new(),
                    url: None,
                    status: PublishStatus::Failed {
                        error: "mock failure".into(),
                    },
                })
            } else {
                Ok(PublishResult {
                    platform: self.name.clone(),
                    publish_id: "mock-id".into(),
                    url: None,
                    status: PublishStatus::Published {
                        url: "https://example.com".into(),
                    },
                })
            }
        }

        async fn status(&self, _publish_id: &str) -> Result<PublishStatus, String> {
            Ok(PublishStatus::Published {
                url: "https://example.com".into(),
            })
        }
    }

    fn test_video() -> VideoAsset {
        VideoAsset {
            id: "test".into(),
            instrument: "ag2506".into(),
            freq: "5min".into(),
            date_range: crate::DateRange {
                start: chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                end: chrono::NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
            },
            duration_secs: 60.0,
            video_path: PathBuf::from("test.mp4"),
            video_size_bytes: 1024,
            title: "Test".into(),
            description: "Test description".into(),
            tags: vec!["futures".into()],
            created_at: Utc::now(),
            thumbnail_path: None,
            category: None,
            content_body: None,
            seo_title: None,
            seo_description: None,
        }
    }

    #[test]
    fn test_backoff_sequence() {
        let scheduler = PublishScheduler::new(Vec::new()).with_retry(3, Duration::from_secs(1));

        assert_eq!(scheduler.backoff(1), Duration::from_secs(1));
        assert_eq!(scheduler.backoff(2), Duration::from_secs(2));
        assert_eq!(scheduler.backoff(3), Duration::from_secs(4));
        assert_eq!(scheduler.backoff(4), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_attempt_zero_handled() {
        let scheduler = PublishScheduler::new(Vec::new()).with_retry(3, Duration::from_secs(1));
        assert_eq!(scheduler.backoff(0), Duration::from_secs(1));
    }

    #[test]
    fn test_publish_summary_all_success() {
        let summary = PublishSummary {
            success_count: 3,
            failed_count: 0,
            success_platforms: vec!["bilibili".into(), "douyin".into(), "xiaohongshu".into()],
            failed_platforms: Vec::new(),
        };
        assert!(summary.all_success());
        assert!(summary.any_success());
    }

    #[test]
    fn test_publish_summary_partial_failure() {
        let summary = PublishSummary {
            success_count: 2,
            failed_count: 1,
            success_platforms: vec!["bilibili".into(), "douyin".into()],
            failed_platforms: vec![("xiaohongshu".into(), "Cookie expired".into())],
        };
        assert!(!summary.all_success());
        assert!(summary.any_success());
    }

    #[test]
    fn test_publish_summary_all_failed() {
        let summary = PublishSummary {
            success_count: 0,
            failed_count: 3,
            success_platforms: Vec::new(),
            failed_platforms: vec![("bilibili".into(), "timeout".into())],
        };
        assert!(!summary.all_success());
        assert!(!summary.any_success());
    }

    #[test]
    fn test_scheduler_builder_pattern() {
        let scheduler = PublishScheduler::new(Vec::new())
            .with_max_concurrent(5)
            .with_retry(5, Duration::from_millis(500));

        assert_eq!(scheduler.max_concurrent, 5);
        assert_eq!(scheduler.max_retries, 5);
        assert_eq!(scheduler.base_backoff, Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_publish_all_concurrent() {
        let p1 = Box::new(MockPublisher::new("platform-a"));
        let p2 = Box::new(MockPublisher::new("platform-b"));
        let p3 = Box::new(MockPublisher::new("platform-c"));

        let scheduler = PublishScheduler::new(vec![p1, p2, p3])
            .with_max_concurrent(3)
            .with_retry(1, Duration::from_millis(10));

        let video = test_video();
        let results = scheduler.publish_all(&video).await;

        assert_eq!(results.len(), 3);
        for attempts in &results {
            assert!(!attempts.is_empty());
            let last = attempts.last().unwrap();
            assert!(last.result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_publish_all_empty() {
        let scheduler = PublishScheduler::new(Vec::new());
        let video = test_video();
        let results = scheduler.publish_all(&video).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_publish_to_platform_serial() {
        let publisher = MockPublisher::new("serial-test");
        let scheduler = PublishScheduler::new(Vec::new()).with_retry(1, Duration::from_millis(10));
        let video = test_video();

        let attempts = scheduler.publish_to_platform(&publisher, &video).await;
        assert_eq!(attempts.len(), 1);
        assert!(attempts.last().unwrap().result.is_ok());
    }
}
