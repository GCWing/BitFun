use bitfun_core::service::remote_connect::embedded_relay_host::EmbeddedRelayHost;
use bitfun_core::service::remote_connect::{
    ConnectionMethod, RemoteConnectConfig, RemoteConnectService,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Default)]
struct RecordingEmbeddedRelayHost {
    start_calls: AtomicUsize,
    stop_calls: AtomicUsize,
    cleanup_stops: AtomicUsize,
    active: AtomicBool,
}

#[derive(Default)]
struct BlockingEmbeddedRelayHost {
    start_calls: AtomicUsize,
    starts_in_flight: AtomicUsize,
    overlapping_starts: AtomicUsize,
    stop_while_starting: AtomicUsize,
    active: AtomicBool,
    first_start_entered: tokio::sync::Notify,
    release_first_start: tokio::sync::Notify,
    stop_called: tokio::sync::Notify,
    lifecycle_violation: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl EmbeddedRelayHost for RecordingEmbeddedRelayHost {
    async fn start(&self, _port: u16, _static_dir: Option<String>) -> anyhow::Result<()> {
        self.start_calls.fetch_add(1, Ordering::SeqCst);
        self.active.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) {
        self.stop_calls.fetch_add(1, Ordering::SeqCst);
        if self.active.swap(false, Ordering::SeqCst) {
            self.cleanup_stops.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[async_trait::async_trait]
impl EmbeddedRelayHost for BlockingEmbeddedRelayHost {
    async fn start(&self, _port: u16, _static_dir: Option<String>) -> anyhow::Result<()> {
        let call_index = self.start_calls.fetch_add(1, Ordering::SeqCst);
        if self.starts_in_flight.fetch_add(1, Ordering::SeqCst) > 0 {
            self.overlapping_starts.fetch_add(1, Ordering::SeqCst);
            self.lifecycle_violation.notify_one();
        }

        if call_index == 0 {
            self.first_start_entered.notify_one();
            self.release_first_start.notified().await;
        }

        self.active.store(true, Ordering::SeqCst);
        self.starts_in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) {
        if self.starts_in_flight.load(Ordering::SeqCst) > 0 {
            self.stop_while_starting.fetch_add(1, Ordering::SeqCst);
            self.lifecycle_violation.notify_one();
        }
        self.active.store(false, Ordering::SeqCst);
        self.stop_called.notify_one();
    }
}

async fn unused_port() -> u16 {
    let reserved = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test should reserve an unused port");
    reserved
        .local_addr()
        .expect("reserved listener should have an address")
        .port()
}

fn lan_config(port: u16) -> RemoteConnectConfig {
    RemoteConnectConfig {
        lan_port: port,
        ..RemoteConnectConfig::default()
    }
}

fn lan_method() -> ConnectionMethod {
    ConnectionMethod::Lan {
        ip: Some("127.0.0.1".to_string()),
    }
}

#[tokio::test]
async fn remote_connect_stop_delegates_concrete_cleanup_to_host() {
    let host = Arc::new(RecordingEmbeddedRelayHost::default());
    let service = RemoteConnectService::new(RemoteConnectConfig::default(), host.clone())
        .expect("remote connect service should initialize");

    service.stop_relay().await;
    service.stop_relay().await;

    assert_eq!(host.stop_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn remote_connect_start_failure_rolls_back_started_host() {
    let port = unused_port().await;

    let host = Arc::new(RecordingEmbeddedRelayHost::default());
    let service = RemoteConnectService::new(lan_config(port), host.clone())
        .expect("remote connect service should initialize");

    service
        .start(lan_method())
        .await
        .expect_err("downstream relay connection should fail without a real host listener");

    assert_eq!(host.start_calls.load(Ordering::SeqCst), 1);
    assert_eq!(host.cleanup_stops.load(Ordering::SeqCst), 1);
    assert!(!host.active.load(Ordering::SeqCst));
}

#[tokio::test]
async fn concurrent_relay_starts_do_not_cleanup_or_enter_the_host_concurrently() {
    let host = Arc::new(BlockingEmbeddedRelayHost::default());
    let service = Arc::new(
        RemoteConnectService::new(lan_config(unused_port().await), host.clone())
            .expect("remote connect service should initialize"),
    );

    let first = tokio::spawn({
        let service = service.clone();
        async move { service.start(lan_method()).await }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        host.first_start_entered.notified(),
    )
    .await
    .expect("first start should enter the host");

    let second = tokio::spawn({
        let service = service.clone();
        async move { service.start(lan_method()).await }
    });
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            host.lifecycle_violation.notified(),
        )
        .await
        .is_err(),
        "a concurrent start must wait instead of stopping or entering the active host start"
    );

    host.release_first_start.notify_one();
    let (first_result, second_result) =
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(first, second)
        })
        .await
        .expect("serialized starts should complete");
    first_result
        .expect("first start task should join")
        .expect_err("fake host does not create a relay listener");
    second_result
        .expect("second start task should join")
        .expect_err("fake host does not create a relay listener");

    assert_eq!(host.start_calls.load(Ordering::SeqCst), 2);
    assert_eq!(host.overlapping_starts.load(Ordering::SeqCst), 0);
    assert_eq!(host.stop_while_starting.load(Ordering::SeqCst), 0);
    assert!(!host.active.load(Ordering::SeqCst));
}

#[tokio::test]
async fn relay_stop_waits_for_an_in_progress_start_to_settle() {
    let host = Arc::new(BlockingEmbeddedRelayHost::default());
    let service = Arc::new(
        RemoteConnectService::new(lan_config(unused_port().await), host.clone())
            .expect("remote connect service should initialize"),
    );

    let start = tokio::spawn({
        let service = service.clone();
        async move { service.start(lan_method()).await }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        host.first_start_entered.notified(),
    )
    .await
    .expect("start should enter the host");
    host.stop_called.notified().await;

    let stop = tokio::spawn({
        let service = service.clone();
        async move { service.stop_relay().await }
    });
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            host.stop_called.notified(),
        )
        .await
        .is_err(),
        "stop must wait behind the in-progress start lifecycle"
    );

    host.release_first_start.notify_one();
    let (start_result, stop_result) =
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(start, stop)
        })
        .await
        .expect("start and stop should complete after release");
    start_result
        .expect("start task should join")
        .expect_err("fake host does not create a relay listener");
    stop_result.expect("stop task should join");

    assert_eq!(host.stop_while_starting.load(Ordering::SeqCst), 0);
    assert!(!host.active.load(Ordering::SeqCst));
}
