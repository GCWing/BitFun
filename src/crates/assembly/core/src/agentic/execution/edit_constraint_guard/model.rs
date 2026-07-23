use super::EDIT_CONSTRAINT_SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintSource {
    Deterministic,
    Model,
    #[default]
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedConstraint {
    #[serde(default)]
    pub id: String,
    /// Human-readable paraphrase shown to the agent in the rejection message.
    pub description: String,
    #[serde(default)]
    pub operation_scope: ConstraintOperationScope,
    pub matcher: ConstraintMatcher,
    #[serde(default)]
    pub source: ConstraintSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintOperationScope {
    #[default]
    All,
    DeleteOnly,
}

impl ConstraintOperationScope {
    pub(super) fn applies_to(self, operation: &str) -> bool {
        match self {
            Self::All => true,
            Self::DeleteOnly => matches!(operation, "delete" | "recursive_delete"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintMatcher {
    TestFiles,
    PathContains {
        substrings: Vec<String>,
    },
    PathUnderDir {
        dirs: Vec<String>,
    },
    Extension {
        exts: Vec<String>,
    },
    /// Recorded for analysis but never enforced.
    Unmatched,
}

impl ConstraintMatcher {
    pub fn matches(&self, file_path: &str) -> bool {
        let normalized = file_path.replace('\\', "/");
        match self {
            ConstraintMatcher::TestFiles => is_test_file(&normalized),
            ConstraintMatcher::PathContains { substrings } => substrings
                .iter()
                .any(|value| !value.is_empty() && normalized.contains(value.as_str())),
            ConstraintMatcher::PathUnderDir { dirs } => dirs.iter().any(|dir| {
                let dir = dir.trim_matches('/');
                !dir.is_empty()
                    && (normalized == dir
                        || normalized.starts_with(&format!("{dir}/"))
                        || normalized.contains(&format!("/{dir}/")))
            }),
            ConstraintMatcher::Extension { exts } => exts
                .iter()
                .any(|extension| !extension.is_empty() && normalized.ends_with(extension)),
            ConstraintMatcher::Unmatched => false,
        }
    }

    pub(super) fn enforceable(&self) -> bool {
        !matches!(self, ConstraintMatcher::Unmatched)
    }
}

fn is_test_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let lowercase = normalized.to_lowercase();
    let name = Path::new(&lowercase)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    stem.starts_with("test_")
        || stem.starts_with("test-")
        || stem.ends_with("_test")
        || stem.ends_with("-test")
        || stem.ends_with("_tests")
        || stem.ends_with("_spec")
        || stem.ends_with("-spec")
        || name.contains(".test.")
        || name.contains(".spec.")
        || lowercase
            .split('/')
            .any(|segment| matches!(segment, "tests" | "test" | "__tests__" | "spec" | "specs"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Extracted,
    NoConstraints,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelExtractionStatus {
    #[default]
    NotRun,
    Parsed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRevocation {
    pub constraint_id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionFailure {
    pub stage: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintExtractionRecord {
    pub message_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialog_turn_id: Option<String>,
    pub status: ExtractionStatus,
    pub constraints: Vec<ExtractedConstraint>,
    pub deterministic_constraint_count: usize,
    pub model_attempts: usize,
    /// Snapshot of the active ids shown to fast for this extraction.
    #[serde(default)]
    pub active_constraint_ids: Vec<String>,
    /// Whether this turn originated at a real user submission and can therefore
    /// relax an existing user-authored edit constraint.
    #[serde(default)]
    pub revocation_authorized: bool,
    #[serde(default)]
    pub model_status: ModelExtractionStatus,
    /// Exact additions parsed from the fast model, before deterministic
    /// additions are merged in.
    #[serde(default)]
    pub model_constraints: Vec<ExtractedConstraint>,
    /// Exact revocation requests parsed from the fast model. Invalid ids remain
    /// here for telemetry but are never applied.
    #[serde(default)]
    pub model_revocations: Vec<ConstraintRevocation>,
    /// Revocations validated against the active constraint ids supplied to the
    /// model. Only these ids are applied to session state.
    #[serde(default)]
    pub revoked_constraint_ids: Vec<String>,
    #[serde(default)]
    pub unmatched_revocation_ids: Vec<String>,
    pub input_chars: usize,
    pub prompt_chars: usize,
    pub input_truncated: bool,
    pub latency_ms: u64,
    pub extracted_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<ExtractionFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditConstraintState {
    pub schema_version: u32,
    /// Active constraints inherited at fork time. Child rollbacks always begin
    /// from this baseline before replaying surviving child turns.
    #[serde(default)]
    pub inherited_constraints: Vec<ExtractedConstraint>,
    #[serde(default)]
    pub constraints: Vec<ExtractedConstraint>,
    #[serde(default)]
    pub extractions: Vec<ConstraintExtractionRecord>,
    /// Paths first created through direct agent file tools in this session.
    /// They remain distinct from repository files across session restoration.
    #[serde(default)]
    pub agent_created_paths: Vec<String>,
    /// Agent-created paths inherited at fork time. They are part of the child
    /// baseline rather than parent turn-scoped history.
    #[serde(default)]
    pub inherited_agent_created_paths: Vec<String>,
    /// Turn-scoped provenance used to rewind helper-file permissions when a
    /// session is rolled back. `agent_created_paths` remains for backwards
    /// compatibility and is rebuilt from these records when possible.
    #[serde(default)]
    pub agent_created_path_records: Vec<AgentCreatedPathRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCreatedPathRecord {
    pub path: String,
    pub dialog_turn_id: String,
}

impl Default for EditConstraintState {
    fn default() -> Self {
        Self {
            schema_version: EDIT_CONSTRAINT_SCHEMA_VERSION,
            inherited_constraints: Vec::new(),
            constraints: Vec::new(),
            extractions: Vec::new(),
            agent_created_paths: Vec::new(),
            inherited_agent_created_paths: Vec::new(),
            agent_created_path_records: Vec::new(),
        }
    }
}

impl EditConstraintState {
    pub fn mark_current_state_as_fork_baseline(&mut self) {
        self.schema_version = EDIT_CONSTRAINT_SCHEMA_VERSION;
        self.inherited_constraints = self.constraints.clone();
        self.inherited_agent_created_paths = self.agent_created_paths.clone();
    }

    pub fn merge_extraction(&mut self, extraction: ConstraintExtractionRecord) {
        self.schema_version = EDIT_CONSTRAINT_SCHEMA_VERSION;
        self.constraints.retain(|constraint| {
            !extraction
                .revoked_constraint_ids
                .iter()
                .any(|constraint_id| constraint_id == &constraint.id)
        });
        for constraint in &extraction.constraints {
            if !self.constraints.iter().any(|existing| {
                existing.matcher == constraint.matcher
                    && existing.operation_scope == constraint.operation_scope
            }) {
                self.constraints.push(constraint.clone());
            }
        }
        self.extractions.push(extraction);
    }

    pub fn message_processed(&self, dialog_turn_id: &str, message_sha256: &str) -> bool {
        self.extractions.iter().any(|record| {
            record.dialog_turn_id.as_deref() == Some(dialog_turn_id)
                && record.message_sha256 == message_sha256
                && record.status != ExtractionStatus::Failed
        })
    }

    pub fn latest_status(&self) -> Option<ExtractionStatus> {
        self.extractions.last().map(|record| record.status)
    }

    pub fn has_enforceable_constraints(&self) -> bool {
        self.constraints
            .iter()
            .any(|constraint| constraint.matcher.enforceable())
    }

    pub(super) fn tracks_agent_created_test_paths(&self) -> bool {
        self.constraints.iter().any(|constraint| {
            matches!(constraint.matcher, ConstraintMatcher::TestFiles)
                && constraint.operation_scope == ConstraintOperationScope::All
        })
    }

    pub(super) fn has_agent_created_paths(&self) -> bool {
        !self.agent_created_paths.is_empty()
    }

    pub fn remember_agent_created_paths(
        &mut self,
        paths: impl IntoIterator<Item = String>,
        dialog_turn_id: &str,
    ) {
        for path in paths {
            let normalized = path.replace('\\', "/");
            if !normalized.is_empty() && !self.agent_created_paths.contains(&normalized) {
                self.agent_created_paths.push(normalized.clone());
            }
            if !normalized.is_empty()
                && !dialog_turn_id.is_empty()
                && !self.agent_created_path_records.iter().any(|record| {
                    record.path == normalized && record.dialog_turn_id == dialog_turn_id
                })
            {
                self.agent_created_path_records
                    .push(AgentCreatedPathRecord {
                        path: normalized,
                        dialog_turn_id: dialog_turn_id.to_string(),
                    });
            }
        }
    }

    pub fn forget_agent_created_paths_under(&mut self, paths: &[String]) {
        self.agent_created_paths.retain(|created| {
            !paths.iter().any(|path| {
                let path = path.trim_end_matches('/');
                created == path || created.starts_with(&format!("{path}/"))
            })
        });
        self.agent_created_path_records.retain(|record| {
            !paths.iter().any(|path| {
                let path = path.trim_end_matches('/');
                record.path == path || record.path.starts_with(&format!("{path}/"))
            })
        });
    }

    pub(super) fn is_agent_created_path(&self, paths: &[String]) -> bool {
        paths
            .iter()
            .any(|path| self.agent_created_paths.contains(path))
    }

    /// Rebuilds the state from events belonging to turns that survive a
    /// session rollback. Older provenance entries did not carry a turn id, so
    /// they are deliberately discarded rather than granting a stale exemption
    /// to a file created only in a rolled-back future turn.
    pub fn rollback_to_surviving_turns(&mut self, surviving_turn_ids: &HashSet<String>) {
        let retained_extractions = self
            .extractions
            .iter()
            .filter(|record| {
                record
                    .dialog_turn_id
                    .as_ref()
                    .is_some_and(|turn_id| surviving_turn_ids.contains(turn_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        let retained_path_records = self
            .agent_created_path_records
            .iter()
            .filter(|record| surviving_turn_ids.contains(&record.dialog_turn_id))
            .cloned()
            .collect::<Vec<_>>();

        self.schema_version = EDIT_CONSTRAINT_SCHEMA_VERSION;
        self.constraints = self.inherited_constraints.clone();
        self.extractions.clear();
        for extraction in retained_extractions {
            self.merge_extraction(extraction);
        }
        self.agent_created_paths = self.inherited_agent_created_paths.clone();
        self.agent_created_path_records = retained_path_records;
        for record in &self.agent_created_path_records {
            if !self.agent_created_paths.contains(&record.path) {
                self.agent_created_paths.push(record.path.clone());
            }
        }
    }
}
