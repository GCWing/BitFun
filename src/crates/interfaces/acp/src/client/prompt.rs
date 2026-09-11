use std::future::Future;
use std::pin::Pin;

use agent_client_protocol::schema::{PromptRequest, PromptResponse};
use agent_client_protocol::{ActiveSession, Agent, Error, SessionMessage};

/// Own the prompt response separately from the session notification queue.
/// ACP 0.12's ActiveSession::send_prompt only queues successful stop reasons;
/// its error callback terminates the connection without waking read_update.
pub(super) struct AcpPrompt {
    response: Pin<Box<dyn Future<Output = Result<PromptResponse, Error>> + Send>>,
}

impl AcpPrompt {
    pub(super) fn start(active: &ActiveSession<'_, Agent>, prompt: String) -> Self {
        let request = PromptRequest::new(active.session_id().clone(), vec![prompt.into()]);
        let response = active.connection().send_request(request).block_task();
        Self {
            response: Box::pin(response),
        }
    }

    pub(super) async fn read_update(
        &mut self,
        active: &mut ActiveSession<'_, Agent>,
    ) -> Result<SessionMessage, Error> {
        tokio::select! {
            // Preserve notifications queued before the response, including any
            // partial output preceding an error. The response also wakes us
            // when the connection drops its pending request sender.
            biased;
            update = active.read_update() => update,
            response = &mut self.response => {
                response.map(|response| SessionMessage::StopReason(response.stop_reason))
            }
        }
    }
}

#[cfg(test)]
#[path = "prompt/tests.rs"]
mod tests;
