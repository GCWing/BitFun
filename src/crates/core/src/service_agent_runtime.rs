//! Core-owned bindings for service and agent runtime ports.
//!
//! Owner crates keep portable contracts and orchestration policy. This module
//! centralizes the concrete core adapters that still own scheduler execution,
//! session restore, terminal pre-warm, remote image conversion, and runtime-port
//! implementations until a reviewed port/provider migration proves equivalence.

use bitfun_runtime_ports::AgentSubmissionPort;
use bitfun_services_integrations::remote_connect::{RemoteImageContext, RemoteImageContextAdapter};

use crate::agentic::coordination::ConversationCoordinator;
use crate::agentic::image_analysis::ImageContextData;
use crate::service::remote_connect::remote_server::{
    CoreRemoteDialogRuntimeHost, RemoteExecutionDispatcher,
};

impl RemoteImageContextAdapter for ImageContextData {
    fn from_remote_image_context(context: RemoteImageContext) -> Self {
        Self {
            id: context.id,
            image_path: context.image_path,
            data_url: context.data_url,
            mime_type: context.mime_type,
            metadata: context.metadata,
        }
    }
}

pub(crate) struct CoreServiceAgentRuntime;

impl CoreServiceAgentRuntime {
    pub(crate) fn remote_dialog_host(
        dispatcher: &RemoteExecutionDispatcher,
    ) -> Result<CoreRemoteDialogRuntimeHost<'_>, String> {
        CoreRemoteDialogRuntimeHost::new(dispatcher)
    }

    pub(crate) fn remote_image_context(context: RemoteImageContext) -> ImageContextData {
        ImageContextData::from_remote_image_context(context)
    }

    pub(crate) fn agent_submission_port(
        coordinator: &ConversationCoordinator,
    ) -> &(dyn AgentSubmissionPort + '_) {
        coordinator
    }
}

#[cfg(test)]
mod tests {
    use bitfun_runtime_ports::{
        AgentTurnCancellationPort, RemoteControlStatePort, SessionTranscriptReader,
    };

    use super::*;

    #[test]
    fn core_service_agent_runtime_owner_keeps_coordinator_port_contracts() {
        fn assert_runtime_ports<T>()
        where
            T: AgentSubmissionPort
                + AgentTurnCancellationPort
                + RemoteControlStatePort
                + SessionTranscriptReader,
        {
        }

        assert_runtime_ports::<ConversationCoordinator>();
    }
}
