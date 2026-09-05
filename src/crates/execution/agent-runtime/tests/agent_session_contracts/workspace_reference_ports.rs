use std::sync::{Arc, Mutex};

use openbitfun_agent_runtime::sdk::{
    AgentMessageWorkspaceReferencesRequest, AgentRuntimeBuilder, AgentSubmissionPort,
    AgentSubmissionRequest, AgentSubmissionResult, AgentWorkspaceReference,
    AgentWorkspaceReferenceKind, AgentWorkspaceReferencePort, AgentWorkspaceReferenceSearchEntry,
    AgentWorkspaceReferenceSearchRequest, AgentWorkspaceReferenceSearchResult,
    AgentWorkspaceReferenceSourceRange, PortResult, RuntimeError,
};

#[derive(Default)]
struct FakeSubmissionPort;

#[async_trait::async_trait]
impl AgentSubmissionPort for FakeSubmissionPort {
    async fn create_session(
        &self,
        _request: openbitfun_agent_runtime::sdk::AgentSessionCreateRequest,
    ) -> PortResult<openbitfun_agent_runtime::sdk::AgentSessionCreateResult> {
        unreachable!("workspace reference contracts do not create sessions")
    }

    async fn submit_message(
        &self,
        _request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult> {
        unreachable!("workspace reference contracts do not use legacy submission")
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(None)
    }
}

#[derive(Default)]
struct RecordingWorkspaceReferencePort {
    searches: Mutex<Vec<AgentWorkspaceReferenceSearchRequest>>,
}

#[async_trait::async_trait]
impl AgentWorkspaceReferencePort for RecordingWorkspaceReferencePort {
    async fn search_workspace_references(
        &self,
        request: AgentWorkspaceReferenceSearchRequest,
    ) -> PortResult<AgentWorkspaceReferenceSearchResult> {
        self.searches.lock().unwrap().push(request);
        Ok(AgentWorkspaceReferenceSearchResult {
            entries: vec![AgentWorkspaceReferenceSearchEntry {
                path: "src/lib.rs".to_string(),
                kind: AgentWorkspaceReferenceKind::File,
            }],
            truncated: false,
        })
    }

    async fn workspace_references_for_message(
        &self,
        request: AgentMessageWorkspaceReferencesRequest,
    ) -> PortResult<Vec<AgentWorkspaceReference>> {
        Ok(vec![AgentWorkspaceReference {
            path: "src/lib.rs".to_string(),
            kind: AgentWorkspaceReferenceKind::File,
            start_line: None,
            end_line: None,
            source: AgentWorkspaceReferenceSourceRange {
                start: request.message_id.len(),
                end: request.message_id.len() + 11,
                value: "@src/lib.rs".to_string(),
            },
        }])
    }
}

#[tokio::test]
async fn sdk_delegates_workspace_reference_search_and_message_lookup() {
    let port = Arc::new(RecordingWorkspaceReferencePort::default());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(Arc::new(FakeSubmissionPort))
        .with_workspace_reference_port(port.clone())
        .build()
        .expect("runtime");

    let search = runtime
        .search_workspace_references(AgentWorkspaceReferenceSearchRequest {
            session_id: "session-1".to_string(),
            query: "src/li".to_string(),
            limit: 20,
        })
        .await
        .expect("search");
    assert_eq!(search.entries[0].path, "src/lib.rs");
    assert_eq!(port.searches.lock().unwrap().len(), 1);

    let references = runtime
        .workspace_references_for_message(AgentMessageWorkspaceReferencesRequest {
            session_id: "session-1".to_string(),
            message_id: "message-1".to_string(),
        })
        .await
        .expect("message references");
    assert_eq!(references[0].source.start, "message-1".len());
}

#[tokio::test]
async fn sdk_reports_a_missing_workspace_reference_port() {
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(Arc::new(FakeSubmissionPort))
        .build()
        .expect("runtime");

    let error = runtime
        .search_workspace_references(AgentWorkspaceReferenceSearchRequest {
            session_id: "session-1".to_string(),
            query: String::new(),
            limit: 20,
        })
        .await
        .expect_err("missing port must fail explicitly");

    assert_eq!(error, RuntimeError::MissingWorkspaceReferencePort);
}
