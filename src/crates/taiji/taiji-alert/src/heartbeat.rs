//! Heartbeat monitor — detects system inactivity and emits alerts.

use crate::{AlertLevel, AlertMessage};
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Callback for delivering heartbeat alerts.
pub type HeartbeatAlertFn = Arc<dyn Fn(AlertMessage) + Send + Sync + 'static>;

/// Monitors system activity and emits heartbeat alerts when no activity
/// is recorded within the configured interval.
pub struct HeartbeatMonitor {
    last_activity: Arc<Mutex<Instant>>,
    interval: Duration,
    alert_callback: HeartbeatAlertFn,
    running: Arc<AtomicBool>,
}

impl HeartbeatMonitor {
    /// Create a new HeartbeatMonitor.
    ///
    /// `interval_min` is the heartbeat check interval in minutes.
    /// `alert_callback` is invoked when the heartbeat deadline is missed.
    pub fn new(interval_min: u32, alert_callback: HeartbeatAlertFn) -> Self {
        Self {
            last_activity: Arc::new(Mutex::new(Instant::now())),
            interval: Duration::from_secs((interval_min as u64).saturating_mul(60)),
            alert_callback,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Record that activity occurred, resetting the heartbeat timer.
    pub fn record_activity(&self) {
        if let Ok(mut last) = self.last_activity.lock() {
            *last = Instant::now();
        }
    }

    /// Start the heartbeat monitor loop in a background task.
    ///
    /// Returns `true` if started, `false` if already running.
    pub fn start(self: &Arc<Self>) -> bool {
        if self.running.swap(true, Ordering::SeqCst) {
            return false;
        }

        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(this.interval);
            // Skip the first immediate tick — allow the system to initialise.
            tick.tick().await;

            loop {
                tick.tick().await;

                if !this.running.load(Ordering::SeqCst) {
                    break;
                }

                let elapsed = {
                    let last = this.last_activity.lock().unwrap();
                    last.elapsed()
                };

                if elapsed >= this.interval {
                    let msg = AlertMessage {
                        level: AlertLevel::Heartbeat,
                        title: "系统心跳超时".into(),
                        body: format!(
                            "太极系统在过去 {} 分钟内无任何作业活动，请确认系统正常运行。",
                            this.interval.as_secs() / 60
                        ),
                        source: "heartbeat".into(),
                        timestamp: Utc::now(),
                        count: 1,
                    };
                    (this.alert_callback)(msg);
                }
            }
        });

        true
    }

    /// Stop the heartbeat monitor loop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[test]
    fn heartbeat_monitor_records_activity() {
        let received: Arc<StdMutex<Vec<AlertMessage>>> = Arc::new(StdMutex::new(Vec::new()));
        let received_clone = received.clone();

        let monitor = Arc::new(HeartbeatMonitor::new(
            1, // 1 minute interval (won't trigger in test)
            Arc::new(move |msg| {
                received_clone.lock().unwrap().push(msg);
            }),
        ));

        // record_activity should not panic
        monitor.record_activity();
        // Second call should also be fine
        monitor.record_activity();

        assert!(received.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn heartbeat_monitor_start_returns_true_only_once() {
        let callback: HeartbeatAlertFn = Arc::new(|_msg| {});
        let monitor = Arc::new(HeartbeatMonitor::new(1, callback));

        assert!(monitor.start());
        assert!(!monitor.start()); // already running
        monitor.stop();
    }

    #[tokio::test]
    async fn heartbeat_monitor_stop_prevents_further_spawn() {
        let callback: HeartbeatAlertFn = Arc::new(|_msg| {});
        let monitor = Arc::new(HeartbeatMonitor::new(1, callback));

        monitor.start();
        monitor.stop();
        // After stop, start should succeed again
        assert!(monitor.start());
        monitor.stop();
    }
}
