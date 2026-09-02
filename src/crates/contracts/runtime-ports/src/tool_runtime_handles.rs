use super::*;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Runtime handles injected into tool execution contexts.
///
/// This bundle is intentionally handle-only. Concrete local or remote
/// implementations are still assembled by product/runtime owners outside this
/// crate.
#[derive(Clone, Default)]
pub struct ToolRuntimeHandles {
    workspace_services: Option<WorkspaceServices>,
    cancellation_token: Option<CancellationToken>,
    round_injection_preemption_token: Option<CancellationToken>,
    terminal_port: Option<Arc<dyn TerminalPort>>,
    remote_exec_port: Option<Arc<dyn RemoteExecPort>>,
    #[cfg(feature = "web-search-port")]
    web_search_provider: Option<Arc<dyn WebSearchProvider>>,
}

impl ToolRuntimeHandles {
    pub fn new(
        workspace_services: Option<WorkspaceServices>,
        cancellation_token: Option<CancellationToken>,
    ) -> Self {
        Self {
            workspace_services,
            cancellation_token,
            round_injection_preemption_token: None,
            terminal_port: None,
            remote_exec_port: None,
            #[cfg(feature = "web-search-port")]
            web_search_provider: None,
        }
    }

    pub fn with_terminal_port(mut self, terminal_port: Option<Arc<dyn TerminalPort>>) -> Self {
        self.terminal_port = terminal_port;
        self
    }

    pub fn with_round_injection_preemption_token(
        mut self,
        round_injection_preemption_token: Option<CancellationToken>,
    ) -> Self {
        self.round_injection_preemption_token = round_injection_preemption_token;
        self
    }

    pub fn with_remote_exec_port(
        mut self,
        remote_exec_port: Option<Arc<dyn RemoteExecPort>>,
    ) -> Self {
        self.remote_exec_port = remote_exec_port;
        self
    }

    #[cfg(feature = "web-search-port")]
    pub fn with_web_search_provider(
        mut self,
        web_search_provider: Option<Arc<dyn WebSearchProvider>>,
    ) -> Self {
        self.web_search_provider = web_search_provider;
        self
    }

    pub fn workspace_services(&self) -> Option<&WorkspaceServices> {
        self.workspace_services.as_ref()
    }

    pub fn cancellation_token(&self) -> Option<&CancellationToken> {
        self.cancellation_token.as_ref()
    }

    pub fn round_injection_preemption_token(&self) -> Option<&CancellationToken> {
        self.round_injection_preemption_token.as_ref()
    }

    pub fn terminal_port(&self) -> Option<&Arc<dyn TerminalPort>> {
        self.terminal_port.as_ref()
    }

    pub fn remote_exec_port(&self) -> Option<&Arc<dyn RemoteExecPort>> {
        self.remote_exec_port.as_ref()
    }

    #[cfg(feature = "web-search-port")]
    pub fn web_search_provider(&self) -> Option<&Arc<dyn WebSearchProvider>> {
        self.web_search_provider.as_ref()
    }
}

impl std::fmt::Debug for ToolRuntimeHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRuntimeHandles")
            .field(
                "workspace_services",
                &self
                    .workspace_services
                    .as_ref()
                    .map(|_| "<WorkspaceServices>"),
            )
            .field(
                "cancellation_token",
                &self
                    .cancellation_token
                    .as_ref()
                    .map(|_| "<CancellationToken>"),
            )
            .field(
                "round_injection_preemption_token",
                &self
                    .round_injection_preemption_token
                    .as_ref()
                    .map(|_| "<CancellationToken>"),
            )
            .field(
                "terminal_port",
                &self.terminal_port.as_ref().map(|_| "<dyn TerminalPort>"),
            )
            .field(
                "remote_exec_port",
                &self
                    .remote_exec_port
                    .as_ref()
                    .map(|_| "<dyn RemoteExecPort>"),
            )
            .field("web_search_provider", &{
                #[cfg(feature = "web-search-port")]
                {
                    self.web_search_provider
                        .as_ref()
                        .map(|_| "<dyn WebSearchProvider>")
                }
                #[cfg(not(feature = "web-search-port"))]
                {
                    None::<&str>
                }
            })
            .finish()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_ports::tests::fake_workspace_services;

    #[test]
    fn tool_runtime_handles_keep_workspace_services_and_cancellation_contracts() {
        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let round_injection_preemption_token = tokio_util::sync::CancellationToken::new();
        let services = fake_workspace_services();

        let handles =
            ToolRuntimeHandles::new(Some(services.clone()), Some(cancellation_token.clone()))
                .with_round_injection_preemption_token(Some(
                    round_injection_preemption_token.clone(),
                ));

        assert!(handles.cancellation_token().is_some());
        assert!(handles.round_injection_preemption_token().is_some());
        assert!(handles.workspace_services().is_some());
        assert!(std::sync::Arc::ptr_eq(
            &services.fs,
            &handles.workspace_services().expect("workspace services").fs
        ));

        let cloned = handles.clone();
        assert!(cloned.cancellation_token().is_some());
        assert!(cloned.round_injection_preemption_token().is_some());
        round_injection_preemption_token.cancel();
        assert!(cloned
            .round_injection_preemption_token()
            .is_some_and(CancellationToken::is_cancelled));
        assert!(!cancellation_token.is_cancelled());
        assert!(std::sync::Arc::ptr_eq(
            &services.shell,
            &cloned
                .workspace_services()
                .expect("workspace services")
                .shell
        ));
        assert_eq!(
            format!("{:?}", handles),
            "ToolRuntimeHandles { workspace_services: Some(\"<WorkspaceServices>\"), cancellation_token: Some(\"<CancellationToken>\"), round_injection_preemption_token: Some(\"<CancellationToken>\"), terminal_port: None, remote_exec_port: None, web_search_provider: None }"
        );
    }
}
