/*!
 * Function Agents Common Module
 *
 * Shared types, errors, and utilities for function agents
 */

use serde::{Deserialize, Serialize};
use std::fmt;

// ==================== Shared Types ====================

/// Language selection for agent outputs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Language {
    Chinese,
    English,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Chinese => "Chinese",
            Language::English => "English",
        }
    }
}

// ==================== Shared Error Types ====================

/// Error types for function agents
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentErrorType {
    GitError,
    AnalysisError,
    InvalidInput,
    InternalError,
}

/// Error struct for function agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentError {
    pub message: String,
    pub error_type: AgentErrorType,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.error_type, self.message)
    }
}

impl std::error::Error for AgentError {}

impl AgentError {
    pub fn git_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::GitError,
        }
    }

    pub fn analysis_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::AnalysisError,
        }
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::InvalidInput,
        }
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::InternalError,
        }
    }
}

/// Result type for function agents
pub type AgentResult<T> = Result<T, AgentError>;
