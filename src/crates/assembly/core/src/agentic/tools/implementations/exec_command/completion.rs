use bitfun_runtime_ports::{
    RemoteExecSessionCompletion, RemoteExecSessionCompletionSource,
    RemoteExecSessionCompletionStatus, TerminalExecSessionCompletion,
    TerminalExecSessionCompletionSource, TerminalExecSessionCompletionStatus,
};
use tool_runtime::exec_command::{
    ExecCommandCompletion, ExecCommandCompletionSource, ExecCommandCompletionStatus,
};

pub(super) fn exec_command_local_completion(
    completion: TerminalExecSessionCompletion,
) -> ExecCommandCompletion {
    ExecCommandCompletion {
        status: match completion.status {
            TerminalExecSessionCompletionStatus::Exited => ExecCommandCompletionStatus::Exited,
            TerminalExecSessionCompletionStatus::Interrupted => {
                ExecCommandCompletionStatus::Interrupted
            }
            TerminalExecSessionCompletionStatus::Killed => ExecCommandCompletionStatus::Killed,
            TerminalExecSessionCompletionStatus::Pruned => ExecCommandCompletionStatus::Pruned,
        },
        source: match completion.source {
            TerminalExecSessionCompletionSource::Process => ExecCommandCompletionSource::Process,
            TerminalExecSessionCompletionSource::OutOfBandControl => {
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
