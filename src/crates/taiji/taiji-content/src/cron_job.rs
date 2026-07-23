use serde::{Deserialize, Serialize};

/// Scheduled video generation job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoCronJob {
    /// Job ID (UUID)
    pub id: String,
    /// Cron expression
    pub cron_expr: String,
    /// Instrument code (empty = all instruments)
    pub instrument: String,
    /// Candlestick interval
    pub freq: String,
    /// Whether enabled
    pub enabled: bool,
    /// Human-readable label
    pub label: String,
}

impl VideoCronJob {
    /// Create a "daily review video" job (weekdays 15:30).
    pub fn daily_review(instrument: &str, freq: &str) -> Self {
        Self {
            id: format!("taiji-video-daily-{}", instrument),
            cron_expr: "0 30 15 * * 1-5".into(), // Mon-Fri 15:30
            instrument: instrument.into(),
            freq: freq.into(),
            enabled: true,
            label: format!("{} {} Daily Review Video", instrument, freq),
        }
    }

    /// Create a "weekly summary" job (Friday 16:00).
    pub fn weekly_summary(instrument: &str, freq: &str) -> Self {
        Self {
            id: format!("taiji-video-weekly-{}", instrument),
            cron_expr: "0 0 16 * * 5".into(), // Friday 16:00
            instrument: instrument.into(),
            freq: freq.into(),
            enabled: true,
            label: format!("{} {} Weekly Summary Video", instrument, freq),
        }
    }
}

/// Cron job execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobResult {
    pub job_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub video_path: Option<String>,
    pub duration_secs: Option<f64>,
    pub publish_count: usize,
    pub error: Option<String>,
}

/// Video cron scheduler.
///
/// Integrates with BitFun CronService (via `taiji_video_cron_register` Tauri command).
pub struct VideoScheduler {
    /// Registered cron jobs
    jobs: Vec<VideoCronJob>,
}

impl VideoScheduler {
    pub fn new() -> Self {
        Self { jobs: Vec::new() }
    }

    /// Register a cron job.
    pub fn register(&mut self, job: VideoCronJob) {
        self.jobs.push(job);
    }

    /// Get all registered jobs.
    pub fn list(&self) -> &[VideoCronJob] {
        &self.jobs
    }

    /// Get default job templates (JSON format, for frontend/MiniApp use).
    pub fn default_templates() -> &'static str {
        r#"[
  {
    "id": "taiji-video-daily-template",
    "cron_expr": "0 30 15 * * 1-5",
    "label": "Daily Review Video (weekdays 15:30)",
    "description": "Auto-generate daily technical analysis video after market close and publish"
  },
  {
    "id": "taiji-video-weekly-template",
    "cron_expr": "0 0 16 * * 5",
    "label": "Weekly Summary Video (Friday 16:00)",
    "description": "Auto-generate weekly technical analysis summary video every Friday"
  }
]"#
    }
}

impl Default for VideoScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daily_review_cron_expr() {
        let job = VideoCronJob::daily_review("ag2506", "5min");
        assert_eq!(job.cron_expr, "0 30 15 * * 1-5");
        assert!(job.enabled);
        assert_eq!(job.instrument, "ag2506");
    }

    #[test]
    fn test_weekly_summary_cron_expr() {
        let job = VideoCronJob::weekly_summary("ag2506", "1day");
        assert_eq!(job.cron_expr, "0 0 16 * * 5");
        assert!(job.enabled);
    }

    #[test]
    fn test_scheduler_register_and_list() {
        let mut scheduler = VideoScheduler::new();
        scheduler.register(VideoCronJob::daily_review("rb2510", "15min"));
        scheduler.register(VideoCronJob::weekly_summary("rb2510", "1day"));
        assert_eq!(scheduler.list().len(), 2);
    }

    #[test]
    fn test_default_templates_is_valid_json() {
        let templates = VideoScheduler::default_templates();
        let parsed: serde_json::Value = serde_json::from_str(templates).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }
}

/// Register the 4 Taiji cron job types with CronService.
///
/// Idempotent — checks for existing jobs by name before creating, so
/// restarting the app does not produce duplicates.
///
/// # Panics
///
/// Will not panic. If CronService is not initialized or current directory retrieval
/// fails, only logs a warning/error and returns.
pub async fn register_taiji_cron_jobs() {
    let cron_service = match bitfun_core::service::cron::get_global_cron_service() {
        Some(svc) => svc,
        None => {
            log::warn!("CronService not initialized; skipping taiji cron job registration");
            return;
        }
    };

    let workspace_path = match std::env::current_dir() {
        Ok(path) => path.to_string_lossy().to_string(),
        Err(e) => {
            log::error!(
                "Failed to get current working directory for taiji cron jobs: {}",
                e
            );
            return;
        }
    };

    // Collect existing job names so we skip duplicates on restart.
    let existing = cron_service.list_jobs().await;
    let existing_names: std::collections::HashSet<&str> =
        existing.iter().map(|j| j.name.as_str()).collect();

    let jobs: [(&str, &str, &str); 4] = [
        (
            "Taiji-Daily Video Generation",
            "30 15 * * 1-5",
            "Based on today's market data, auto-generate today's technical analysis video",
        ),
        (
            "Taiji-Daily Report Generation",
            "30 16 * * 1-5",
            "Based on today's market data, auto-generate today's trading analysis report",
        ),
        (
            "Taiji-Daily Website Deployment",
            "0 17 * * 1-5",
            "Auto-deploy today's generated videos and reports to the teaching website",
        ),
        (
            "Taiji-Weekly Summary",
            "30 17 * * 5",
            "Generate this week's trading summary report, including comprehensive analysis of all instruments and next week's outlook",
        ),
    ];

    for (name, expr, text) in &jobs {
        if existing_names.contains(name) {
            log::info!("Taiji cron job already registered, skipping: {}", name);
            continue;
        }

        let request = bitfun_core::service::cron::CreateCronJobRequest {
            name: name.to_string(),
            schedule: bitfun_core::service::cron::CronSchedule::Cron {
                expr: expr.to_string(),
                tz: Some("Asia/Shanghai".to_string()),
            },
            payload: bitfun_core::service::cron::CronJobPayload {
                text: text.to_string(),
            },
            enabled: true,
            target: bitfun_core::service::cron::CronJobTarget::Workspace {
                workspace: bitfun_core::service::cron::CronWorkspaceRef {
                    workspace_id: None,
                    workspace_path: workspace_path.clone(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
                launch: bitfun_core::service::cron::CronLaunchSpec::default(),
            },
        };

        match cron_service.create_job(request).await {
            Ok(job) => {
                log::info!("Registered taiji cron job: {} (id={})", job.name, job.id);
            }
            Err(e) => {
                log::error!("Failed to register taiji cron job '{}': {}", name, e);
            }
        }
    }
}
