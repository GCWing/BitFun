use crate::daemon::{
    convert::{convert_repo_status, convert_task_kind, convert_task_phase, convert_task_status},
    protocol::{
        ClientCapabilities, Notification, ProgressNotificationParams,
        RepoStatus as RepoStatusPayload, TaskFinishedParams, WorkspaceStatusChangedParams,
    },
    repo::RepoTaskStatus,
};

#[derive(Debug, Clone)]
pub enum ServiceNotificationEvent {
    TaskProgress(RepoTaskStatus),
    WorkspaceStatus(RepoStatusPayload),
    TaskFinished(RepoTaskStatus),
}

pub(super) fn should_send_notification(
    initialized: bool,
    capabilities: &ClientCapabilities,
    notification: &Notification,
) -> bool {
    if !initialized {
        return false;
    }
    match notification {
        Notification::Progress { .. } => capabilities.progress,
        Notification::WorkspaceStatusChanged { .. } => capabilities.status_notifications,
        Notification::TaskFinished { .. } => capabilities.task_notifications,
    }
}

pub(super) fn notification_for_event(event: ServiceNotificationEvent) -> Option<Notification> {
    match event {
        ServiceNotificationEvent::TaskProgress(task) => Some(Notification::Progress {
            params: ProgressNotificationParams {
                task_id: task.task_id,
                workspace_id: task.repo_id,
                kind: convert_task_kind(task.kind),
                phase: task.phase.map(convert_task_phase),
                message: task.message,
                processed: task.processed,
                total: task.total,
            },
        }),
        ServiceNotificationEvent::WorkspaceStatus(status) => {
            Some(Notification::WorkspaceStatusChanged {
                params: WorkspaceStatusChangedParams {
                    workspace_id: status.repo_id.clone(),
                    status,
                },
            })
        }
        ServiceNotificationEvent::TaskFinished(task) => Some(Notification::TaskFinished {
            params: TaskFinishedParams {
                task: convert_task_status(task),
            },
        }),
    }
}

#[allow(dead_code)]
pub(super) fn workspace_status_event(
    status: crate::daemon::repo::RepoStatus,
) -> ServiceNotificationEvent {
    ServiceNotificationEvent::WorkspaceStatus(convert_repo_status(status))
}
