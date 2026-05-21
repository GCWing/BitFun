use super::types::*;
use crate::function_agents::common::AgentResult;
use crate::function_agents::port_adapters::{
    CoreFunctionAgentAiAdapter, CoreFunctionAgentGitAdapter,
};
use crate::infrastructure::ai::AIClientFactory;
use bitfun_product_domains::function_agents::ports::FunctionAgentRuntimeFacade;
use chrono::{Local, Timelike};
/**
 * Work state analyzer
 *
 * Analyzes the user's current work state, including Git status and file changes
 */
use log::info;
use std::path::Path;
use std::sync::Arc;

pub struct WorkStateAnalyzer;

impl WorkStateAnalyzer {
    pub async fn analyze_work_state(
        factory: Arc<AIClientFactory>,
        repo_path: &Path,
        options: WorkStateOptions,
    ) -> AgentResult<WorkStateAnalysis> {
        info!("Analyzing work state: repo_path={:?}", repo_path);

        let now = Local::now();
        let git_adapter = CoreFunctionAgentGitAdapter::default();
        let ai_adapter = CoreFunctionAgentAiAdapter::new(factory);
        let facade = FunctionAgentRuntimeFacade::new(&git_adapter, &ai_adapter);
        // Keep the legacy analyzed_at timing in core: assign it after AI analysis completes.
        let mut analysis = facade
            .analyze_work_state(
                repo_path.to_path_buf(),
                options,
                now.timestamp(),
                now.hour(),
                String::new(),
            )
            .await?;
        analysis.analyzed_at = Local::now().to_rfc3339();
        Ok(analysis)
    }
}
