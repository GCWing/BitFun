use super::types::*;
use crate::function_agents::common::AgentResult;
use crate::infrastructure::ai::AIClientFactory;
use crate::product_domain_runtime::CoreProductDomainRuntime;
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
        let git_adapter = CoreProductDomainRuntime::function_agent_git_adapter();
        let ai_adapter = CoreProductDomainRuntime::function_agent_ai_adapter(factory);
        let facade =
            CoreProductDomainRuntime::function_agent_runtime_facade(&git_adapter, &ai_adapter);
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
