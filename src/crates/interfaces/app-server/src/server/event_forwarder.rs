use crate::agent::BitfunAppRuntime;
use crate::role::AppClient;
use crate::schema::{
    ConfigEventNotification, EventStream, EventStreamState, EventStreamStateNotification,
    PermissionEventNotification, ResyncDirective, SessionEventNotification,
};
use agent_client_protocol::{ConnectionTo, Result};
use bitfun_agent_runtime::sdk::{
    AgentEventReceiver, PermissionRequestEvent, PermissionRequestEventReceiver,
};
use std::sync::Arc;

pub(super) struct EventSubscriptions {
    agent: AgentEventReceiver,
    permission: Option<PermissionRequestEventReceiver>,
    config:
        Option<tokio::sync::broadcast::Receiver<bitfun_core::service::config::ConfigUpdateEvent>>,
}

impl EventSubscriptions {
    pub(super) fn subscribe(runtime: &BitfunAppRuntime) -> Self {
        Self {
            agent: runtime.event_source().subscribe(),
            permission: runtime.runtime().subscribe_permission_requests().ok(),
            config: bitfun_core::service::config::subscribe_config_updates(),
        }
    }
}

pub(super) async fn run(
    subscriptions: EventSubscriptions,
    cx: ConnectionTo<AppClient>,
    event_state: Arc<crate::server::ConnectionEventState>,
) -> Result<()> {
    let EventSubscriptions {
        agent: mut rx,
        permission: mut permission_rx,
        config: mut config_rx,
    } = subscriptions;
    loop {
        let permission_recv = async {
            match &mut permission_rx {
                Some(receiver) => Some(receiver.recv().await),
                None => {
                    std::future::pending::<
                        Option<
                            Result<
                                PermissionRequestEvent,
                                tokio::sync::broadcast::error::RecvError,
                            >,
                        >,
                    >()
                    .await
                }
            }
        };
        let config_recv = async {
            match &mut config_rx {
                Some(receiver) => Some(receiver.recv().await),
                None => {
                    std::future::pending::<
                        Option<
                            Result<
                                bitfun_core::service::config::ConfigUpdateEvent,
                                tokio::sync::broadcast::error::RecvError,
                            >,
                        >,
                    >()
                    .await
                }
            }
        };
        tokio::select! {
            recv = rx.recv() => match recv {
                Ok(envelope) => {
                    let notification = SessionEventNotification {
                        cursor: event_state.next_cursor(EventStream::Agent),
                        event: envelope,
                    };
                    if let Err(error) = cx.send_notification(notification) {
                        log::warn!("App-server agent event forwarder failed to send a notification: {:?} -- skipping this event", error);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    send_stream_state(&cx, &event_state, EventStream::Agent, EventStreamState::Lagged, Some(missed), "session/sync", false);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    send_stream_state(&cx, &event_state, EventStream::Agent, EventStreamState::Closed, None, "session/sync", false);
                    log::warn!("App-server agent event stream closed -- serve main loop exiting (client RPCs will now fail with 'receiver is gone')");
                    break;
                }
            },
            recv = permission_recv => match recv {
                Some(Ok(event)) => {
                    let result = event_state.forward_permission(|cursor| {
                        cx.send_notification(PermissionEventNotification { cursor, event })
                    });
                    if let Err(error) = result {
                        log::warn!("App-server permission event forwarder failed to send a notification: {:?} -- skipping this event", error);
                    }
                }
                Some(Err(tokio::sync::broadcast::error::RecvError::Lagged(missed))) => {
                    send_stream_state(&cx, &event_state, EventStream::Permission, EventStreamState::Lagged, Some(missed), "app/syncEvents", true);
                }
                Some(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    send_stream_state(&cx, &event_state, EventStream::Permission, EventStreamState::Closed, None, "app/syncEvents", true);
                    permission_rx = None;
                }
                None => {}
            },
            recv = config_recv => match recv {
                Some(Ok(event)) => {
                    if let Err(error) = cx.send_notification(ConfigEventNotification {
                        cursor: event_state.next_cursor(EventStream::Config),
                        event: crate::server::wire::config_update(event),
                    }) {
                        log::warn!("App-server config event forwarder failed to send a notification: {:?} -- skipping this event", error);
                    }
                },
                Some(Err(tokio::sync::broadcast::error::RecvError::Lagged(missed))) => {
                    send_stream_state(&cx, &event_state, EventStream::Config, EventStreamState::Lagged, Some(missed), "app/syncEvents", false);
                }
                Some(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    send_stream_state(&cx, &event_state, EventStream::Config, EventStreamState::Closed, None, "app/syncEvents", false);
                    config_rx = None;
                }
                None => {}
            },
        }
    }
    Ok(())
}

fn send_stream_state(
    cx: &ConnectionTo<AppClient>,
    event_state: &crate::server::ConnectionEventState,
    stream: EventStream,
    state: EventStreamState,
    missed: Option<u64>,
    method: &str,
    snapshot_available: bool,
) {
    let notification = EventStreamStateNotification {
        cursor: event_state.cursor(stream),
        stream,
        state,
        missed,
        resync: ResyncDirective {
            method: method.to_string(),
            snapshot_available,
            reason: Some("The authoritative event stream is no longer contiguous".to_string()),
        },
    };
    if let Err(error) = cx.send_notification(notification) {
        log::warn!(
            "App-server event stream state notification failed: {:?}",
            error
        );
    }
}
