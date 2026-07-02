use crate::service::remote_ssh::{
    RemoteExecSessionCompletion, RemoteExecSessionCompletionSource,
    RemoteExecSessionCompletionStatus,
};
use terminal_core::{
    LocalExecSessionCompletion, LocalExecSessionCompletionSource, LocalExecSessionCompletionStatus,
};
use tool_runtime::exec_command::{
    ExecCommandCompletion, ExecCommandCompletionSource, ExecCommandCompletionStatus,
};

pub(super) fn exec_command_local_completion(
    completion: LocalExecSessionCompletion,
) -> ExecCommandCompletion {
    ExecCommandCompletion {
        status: match completion.status {
            LocalExecSessionCompletionStatus::Exited => ExecCommandCompletionStatus::Exited,
            LocalExecSessionCompletionStatus::Interrupted => {
                ExecCommandCompletionStatus::Interrupted
            }
            LocalExecSessionCompletionStatus::Killed => ExecCommandCompletionStatus::Killed,
            LocalExecSessionCompletionStatus::Pruned => ExecCommandCompletionStatus::Pruned,
        },
        source: match completion.source {
            LocalExecSessionCompletionSource::Process => ExecCommandCompletionSource::Process,
            LocalExecSessionCompletionSource::OutOfBandControl => {
                ExecCommandCompletionSource::OutOfBandControl
            }
        },
    }
}

pub(super) fn exec_command_remote_completion(
    completion: RemoteExecSessionCompletion,
) -> ExecCommandCompletion {
    ExecCommandCompletion {
        status: match completion.status {
            RemoteExecSessionCompletionStatus::Exited => ExecCommandCompletionStatus::Exited,
            RemoteExecSessionCompletionStatus::Interrupted => {
                ExecCommandCompletionStatus::Interrupted
            }
            RemoteExecSessionCompletionStatus::Killed => ExecCommandCompletionStatus::Killed,
            RemoteExecSessionCompletionStatus::Pruned => ExecCommandCompletionStatus::Pruned,
        },
        source: match completion.source {
            RemoteExecSessionCompletionSource::Process => ExecCommandCompletionSource::Process,
            RemoteExecSessionCompletionSource::OutOfBandControl => {
                ExecCommandCompletionSource::OutOfBandControl
            }
        },
    }
}
