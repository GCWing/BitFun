/**
 * Startchat Function Agent - module entry
 *
 * Provides work state analysis and greeting generation on session start
 */

pub mod types;
pub mod work_state_analyzer;
pub mod ai_service;

pub use types::*;
pub use work_state_analyzer::WorkStateAnalyzer;
pub use ai_service::AIWorkStateService;

use std::path::Path;
use std::sync::Arc;
use crate::infrastructure::ai::AIClientFactory;

/// Combines work state analysis and greeting generation
pub struct StartchatFunctionAgent {
    factory: Arc<AIClientFactory>,
}

impl StartchatFunctionAgent {
    pub fn new(factory: Arc<AIClientFactory>) -> Self {
        Self { factory }
    }
    
    /// Analyze work state and generate greeting
    pub async fn analyze_work_state(
        &self,
        repo_path: &Path,
        options: WorkStateOptions,
    ) -> AgentResult<WorkStateAnalysis> {
        WorkStateAnalyzer::analyze_work_state(self.factory.clone(), repo_path, options).await
    }
    
    /// Quickly analyze work state (use default options with specified language)
    pub async fn quick_analyze(&self, repo_path: &Path, language: Language) -> AgentResult<WorkStateAnalysis> {
        let options = WorkStateOptions {
            language,
            ..WorkStateOptions::default()
        };
        self.analyze_work_state(repo_path, options).await
    }
    
    /// Generate greeting only (do not analyze Git status)
    pub async fn generate_greeting_only(&self, repo_path: &Path) -> AgentResult<WorkStateAnalysis> {
        let options = WorkStateOptions {
            analyze_git: false,
            predict_next_actions: false,
            include_quick_actions: false,
            language: Language::Chinese,
        };
        
        self.analyze_work_state(repo_path, options).await
    }
}

