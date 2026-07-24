//! Unified owner for typed external-source coordinators and discovery lanes.

use crate::refresh::CompletedDeferredDiscovery;
use crate::{
    DeferredDiscovery, DiscoveryBatch, DiscoveryLane, ExternalMcpCoordinator,
    ExternalMcpDiscoveryRequest, ExternalMcpDiscoveryResult, ExternalSourceCoordinator,
    ExternalSourceDiscoveryRequest, ExternalSourceDiscoveryResult, ExternalSubagentCoordinator,
    ExternalSubagentDiscoveryRequest, ExternalSubagentDiscoveryResult, ExternalToolCoordinator,
    ExternalToolDiscoveryRequest, ExternalToolDiscoveryResult,
};
use bitfun_product_domains::external_sources::{
    ExternalMcpRevisionKey, ExternalMcpSourceProvider, ExternalSourceContext,
    ExternalToolSourceProvider, PromptCommandSourceProvider,
};
use bitfun_product_domains::external_subagents::ExternalSubagentSourceProvider;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub struct ExternalSourceControlPlane {
    commands: Mutex<ExternalSourceCoordinator>,
    tools: Mutex<ExternalToolCoordinator>,
    subagents: Mutex<ExternalSubagentCoordinator>,
    mcp: Mutex<ExternalMcpCoordinator>,
    command_lane: DiscoveryLane<ExternalSourceDiscoveryRequest>,
    tool_lane: DiscoveryLane<ExternalToolDiscoveryRequest>,
    subagent_lane: DiscoveryLane<ExternalSubagentDiscoveryRequest>,
    mcp_lane: DiscoveryLane<ExternalMcpDiscoveryRequest>,
}

impl fmt::Debug for ExternalSourceControlPlane {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalSourceControlPlane")
            .field(
                "commands",
                &self.commands(|coordinator| format!("{coordinator:?}")),
            )
            .field(
                "tools",
                &self.tools(|coordinator| format!("{coordinator:?}")),
            )
            .field(
                "subagents",
                &self.subagents(|coordinator| format!("{coordinator:?}")),
            )
            .field("mcp", &self.mcp(|coordinator| format!("{coordinator:?}")))
            .finish_non_exhaustive()
    }
}

impl ExternalSourceControlPlane {
    pub fn new(
        context: ExternalSourceContext,
        mcp_revision_key: ExternalMcpRevisionKey,
        command_providers: Vec<Arc<dyn PromptCommandSourceProvider>>,
        tool_providers: Vec<Arc<dyn ExternalToolSourceProvider>>,
        subagent_providers: Vec<Arc<dyn ExternalSubagentSourceProvider>>,
        mcp_providers: Vec<Arc<dyn ExternalMcpSourceProvider>>,
    ) -> Result<Self, String> {
        Ok(Self {
            commands: Mutex::new(ExternalSourceCoordinator::new(
                context.clone(),
                command_providers,
            )?),
            tools: Mutex::new(ExternalToolCoordinator::new(
                context.clone(),
                tool_providers,
            )?),
            subagents: Mutex::new(ExternalSubagentCoordinator::new(
                context.clone(),
                subagent_providers,
            )?),
            mcp: Mutex::new(ExternalMcpCoordinator::new(
                context,
                mcp_revision_key,
                mcp_providers,
            )?),
            command_lane: DiscoveryLane::new(),
            tool_lane: DiscoveryLane::new(),
            subagent_lane: DiscoveryLane::new(),
            mcp_lane: DiscoveryLane::new(),
        })
    }

    pub fn commands<T>(&self, read: impl FnOnce(&ExternalSourceCoordinator) -> T) -> T {
        read(&lock(&self.commands, "command"))
    }

    /// Returns the command coordinator guard for assembly consumers that need
    /// to perform an atomic multi-step update. Prefer [`Self::commands`] or
    /// [`Self::commands_mut`] for ordinary reads and writes.
    #[doc(hidden)]
    pub fn lock_commands(&self) -> MutexGuard<'_, ExternalSourceCoordinator> {
        lock(&self.commands, "command")
    }

    pub fn commands_mut<T>(&self, update: impl FnOnce(&mut ExternalSourceCoordinator) -> T) -> T {
        update(&mut lock(&self.commands, "command"))
    }

    pub fn tools<T>(&self, read: impl FnOnce(&ExternalToolCoordinator) -> T) -> T {
        read(&lock(&self.tools, "tool"))
    }

    #[doc(hidden)]
    pub fn lock_tools(&self) -> MutexGuard<'_, ExternalToolCoordinator> {
        lock(&self.tools, "tool")
    }

    pub fn tools_mut<T>(&self, update: impl FnOnce(&mut ExternalToolCoordinator) -> T) -> T {
        update(&mut lock(&self.tools, "tool"))
    }

    pub fn subagents<T>(&self, read: impl FnOnce(&ExternalSubagentCoordinator) -> T) -> T {
        read(&lock(&self.subagents, "subagent"))
    }

    #[doc(hidden)]
    pub fn lock_subagents(&self) -> MutexGuard<'_, ExternalSubagentCoordinator> {
        lock(&self.subagents, "subagent")
    }

    pub fn subagents_mut<T>(
        &self,
        update: impl FnOnce(&mut ExternalSubagentCoordinator) -> T,
    ) -> T {
        update(&mut lock(&self.subagents, "subagent"))
    }

    pub fn mcp<T>(&self, read: impl FnOnce(&ExternalMcpCoordinator) -> T) -> T {
        read(&lock(&self.mcp, "MCP"))
    }

    #[doc(hidden)]
    pub fn lock_mcp(&self) -> MutexGuard<'_, ExternalMcpCoordinator> {
        lock(&self.mcp, "MCP")
    }

    pub fn mcp_mut<T>(&self, update: impl FnOnce(&mut ExternalMcpCoordinator) -> T) -> T {
        update(&mut lock(&self.mcp, "MCP"))
    }

    pub fn replace_suppressed_sources(&self, sources: BTreeSet<String>) {
        self.commands_mut(|coordinator| coordinator.replace_suppressed_sources(sources.clone()));
        self.tools_mut(|coordinator| coordinator.replace_suppressed_sources(sources.clone()));
        self.subagents_mut(|coordinator| coordinator.replace_suppressed_sources(sources.clone()));
        self.mcp_mut(|coordinator| coordinator.replace_suppressed_sources(sources));
    }

    pub async fn discover_commands(
        &self,
        requests: Vec<ExternalSourceDiscoveryRequest>,
        timeout: Duration,
    ) -> DiscoveryBatch<ExternalSourceDiscoveryResult> {
        self.command_lane.discover(requests, timeout).await
    }

    pub async fn complete_command(
        &self,
        deferred: DeferredDiscovery<ExternalSourceDiscoveryResult>,
    ) -> Option<(
        CompletedDeferredDiscovery<ExternalSourceDiscoveryResult>,
        Option<DeferredDiscovery<ExternalSourceDiscoveryResult>>,
    )> {
        self.command_lane.complete_deferred(deferred).await
    }

    pub async fn resume_abandoned_command(
        &self,
        deferred: DeferredDiscovery<ExternalSourceDiscoveryResult>,
    ) -> Option<DeferredDiscovery<ExternalSourceDiscoveryResult>> {
        self.command_lane.resume_abandoned(deferred).await
    }

    pub async fn finalize_command(
        &self,
        completed: CompletedDeferredDiscovery<ExternalSourceDiscoveryResult>,
    ) -> Option<ExternalSourceDiscoveryResult> {
        self.command_lane.finalize_deferred(completed).await
    }

    pub async fn discover_tools(
        &self,
        requests: Vec<ExternalToolDiscoveryRequest>,
        timeout: Duration,
    ) -> DiscoveryBatch<ExternalToolDiscoveryResult> {
        self.tool_lane.discover(requests, timeout).await
    }

    pub async fn complete_tool(
        &self,
        deferred: DeferredDiscovery<ExternalToolDiscoveryResult>,
    ) -> Option<(
        CompletedDeferredDiscovery<ExternalToolDiscoveryResult>,
        Option<DeferredDiscovery<ExternalToolDiscoveryResult>>,
    )> {
        self.tool_lane.complete_deferred(deferred).await
    }

    pub async fn resume_abandoned_tool(
        &self,
        deferred: DeferredDiscovery<ExternalToolDiscoveryResult>,
    ) -> Option<DeferredDiscovery<ExternalToolDiscoveryResult>> {
        self.tool_lane.resume_abandoned(deferred).await
    }

    pub async fn finalize_tool(
        &self,
        completed: CompletedDeferredDiscovery<ExternalToolDiscoveryResult>,
    ) -> Option<ExternalToolDiscoveryResult> {
        self.tool_lane.finalize_deferred(completed).await
    }

    pub async fn discover_subagents(
        &self,
        requests: Vec<ExternalSubagentDiscoveryRequest>,
        timeout: Duration,
    ) -> DiscoveryBatch<ExternalSubagentDiscoveryResult> {
        self.subagent_lane.discover(requests, timeout).await
    }

    pub async fn complete_subagent(
        &self,
        deferred: DeferredDiscovery<ExternalSubagentDiscoveryResult>,
    ) -> Option<(
        CompletedDeferredDiscovery<ExternalSubagentDiscoveryResult>,
        Option<DeferredDiscovery<ExternalSubagentDiscoveryResult>>,
    )> {
        self.subagent_lane.complete_deferred(deferred).await
    }

    pub async fn resume_abandoned_subagent(
        &self,
        deferred: DeferredDiscovery<ExternalSubagentDiscoveryResult>,
    ) -> Option<DeferredDiscovery<ExternalSubagentDiscoveryResult>> {
        self.subagent_lane.resume_abandoned(deferred).await
    }

    pub async fn finalize_subagent(
        &self,
        completed: CompletedDeferredDiscovery<ExternalSubagentDiscoveryResult>,
    ) -> Option<ExternalSubagentDiscoveryResult> {
        self.subagent_lane.finalize_deferred(completed).await
    }

    pub async fn discover_mcp(
        &self,
        requests: Vec<ExternalMcpDiscoveryRequest>,
        timeout: Duration,
    ) -> DiscoveryBatch<ExternalMcpDiscoveryResult> {
        self.mcp_lane.discover(requests, timeout).await
    }

    pub async fn complete_mcp(
        &self,
        deferred: DeferredDiscovery<ExternalMcpDiscoveryResult>,
    ) -> Option<(
        CompletedDeferredDiscovery<ExternalMcpDiscoveryResult>,
        Option<DeferredDiscovery<ExternalMcpDiscoveryResult>>,
    )> {
        self.mcp_lane.complete_deferred(deferred).await
    }

    pub async fn resume_abandoned_mcp(
        &self,
        deferred: DeferredDiscovery<ExternalMcpDiscoveryResult>,
    ) -> Option<DeferredDiscovery<ExternalMcpDiscoveryResult>> {
        self.mcp_lane.resume_abandoned(deferred).await
    }

    pub async fn finalize_mcp(
        &self,
        completed: CompletedDeferredDiscovery<ExternalMcpDiscoveryResult>,
    ) -> Option<ExternalMcpDiscoveryResult> {
        self.mcp_lane.finalize_deferred(completed).await
    }
}

fn lock<'a, T>(mutex: &'a Mutex<T>, capability: &str) -> MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::error!(
                "External source coordinator mutex poisoned capability={}",
                capability
            );
            poisoned.into_inner()
        }
    }
}
