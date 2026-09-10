//! Stable, platform-agnostic contracts for importing retired product data.
//!
//! This module describes persisted state and the handoff protocol. Filesystem,
//! process, SQLite, credential-vault, and UI behavior belong to service and app
//! owners.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CURRENT_MIGRATION_FORMAT_VERSION: u32 = 1;
pub const CURRENT_MIGRATOR_PROTOCOL_VERSION: u32 = 1;
pub const MIN_MIGRATOR_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationGroupId {
    SettingsAndCredentials,
    AgentsSkillsAndMiniapps,
    WorkspacesSessionsAndTasks,
    Memory,
    RemoteConnectionsAndDevices,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MigrationDomainId {
    #[default]
    Settings,
    Credentials,
    Skills,
    Miniapps,
    Agents,
    WorkspaceSessions,
    AgentCoordination,
    StructuredMemory,
    FileMemory,
    RemoteConnectDevices,
    RemoteSsh,
    CrossReferenceRepair,
}

impl MigrationDomainId {
    pub const fn group(self) -> Option<MigrationGroupId> {
        match self {
            Self::Settings | Self::Credentials => Some(MigrationGroupId::SettingsAndCredentials),
            Self::Skills | Self::Miniapps | Self::Agents => {
                Some(MigrationGroupId::AgentsSkillsAndMiniapps)
            }
            Self::WorkspaceSessions | Self::AgentCoordination => {
                Some(MigrationGroupId::WorkspacesSessionsAndTasks)
            }
            Self::StructuredMemory | Self::FileMemory => Some(MigrationGroupId::Memory),
            Self::RemoteConnectDevices | Self::RemoteSsh => {
                Some(MigrationGroupId::RemoteConnectionsAndDevices)
            }
            Self::CrossReferenceRepair => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationSelection {
    pub groups: BTreeSet<MigrationGroupId>,
}

impl MigrationSelection {
    pub fn all() -> Self {
        Self {
            groups: BTreeSet::from([
                MigrationGroupId::SettingsAndCredentials,
                MigrationGroupId::AgentsSkillsAndMiniapps,
                MigrationGroupId::WorkspacesSessionsAndTasks,
                MigrationGroupId::Memory,
                MigrationGroupId::RemoteConnectionsAndDevices,
            ]),
        }
    }

    /// Expand the five user-visible groups into their fixed dependency order.
    pub fn expanded_domains(&self) -> Vec<MigrationDomainId> {
        let mut domains = Vec::new();
        if self
            .groups
            .contains(&MigrationGroupId::SettingsAndCredentials)
        {
            domains.extend([MigrationDomainId::Settings, MigrationDomainId::Credentials]);
        }
        if self
            .groups
            .contains(&MigrationGroupId::AgentsSkillsAndMiniapps)
        {
            domains.extend([
                MigrationDomainId::Skills,
                MigrationDomainId::Miniapps,
                MigrationDomainId::Agents,
            ]);
        }
        if self
            .groups
            .contains(&MigrationGroupId::WorkspacesSessionsAndTasks)
        {
            domains.extend([
                MigrationDomainId::WorkspaceSessions,
                MigrationDomainId::AgentCoordination,
            ]);
        }
        if self.groups.contains(&MigrationGroupId::Memory) {
            domains.extend([
                MigrationDomainId::StructuredMemory,
                MigrationDomainId::FileMemory,
            ]);
        }
        if self
            .groups
            .contains(&MigrationGroupId::RemoteConnectionsAndDevices)
        {
            domains.extend([
                MigrationDomainId::RemoteConnectDevices,
                MigrationDomainId::RemoteSsh,
            ]);
        }
        if !domains.is_empty() {
            domains.push(MigrationDomainId::CrossReferenceRepair);
        }
        domains
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LegacyRootDescriptor {
    pub kind: LegacyRootKind,
    pub display_path: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyRootKind {
    #[default]
    ProductData,
    ProductHome,
    RemoteSsh,
    ManagedWebview,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LegacySourceDescriptor {
    pub source_id: String,
    pub source_fingerprint: String,
    pub product_id: String,
    pub product_version: String,
    pub platform: String,
    pub roots: Vec<LegacyRootDescriptor>,
    pub readable: bool,
    pub supported: bool,
    pub approximate_bytes: u64,
    pub already_migrated: bool,
    pub diagnostics: Vec<MigrationDiagnostic>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    #[default]
    Info,
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScanFinding {
    pub domain: MigrationDomainId,
    pub code: String,
    pub severity: FindingSeverity,
    pub entity_count: u64,
    pub logical_bytes: u64,
    pub source_schema: Option<String>,
    pub migratable: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    #[default]
    TargetWins,
    SourceImported,
    SourceRemapped,
    DuplicateSkipped,
    ItemSkipped,
    RequiresUserAction,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationConflict {
    pub domain: MigrationDomainId,
    pub code: String,
    pub source_summary: String,
    pub target_summary: String,
    pub resolution: ConflictResolution,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationPlanStep {
    pub sequence: u32,
    pub domain: MigrationDomainId,
    pub estimated_write_bytes: u64,
    pub source_schema: Option<String>,
    pub target_schema: Option<String>,
    pub dependencies: Vec<MigrationDomainId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationPlan {
    pub format_version: u32,
    pub run_id: String,
    pub source_fingerprint: String,
    pub selection: MigrationSelection,
    pub steps: Vec<MigrationPlanStep>,
    pub findings: Vec<ScanFinding>,
    pub conflicts: Vec<MigrationConflict>,
    pub estimated_write_bytes: u64,
    pub plan_hash: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    #[default]
    Discover,
    Scan,
    Plan,
    Acquire,
    Stage,
    ValidateStage,
    Commit,
    ValidateCommit,
    Finalize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationProgressEvent {
    pub run_id: String,
    pub domain: Option<MigrationDomainId>,
    pub phase: MigrationPhase,
    pub processed: u64,
    pub total: u64,
    pub safe_to_cancel: bool,
    pub code: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationRunStatus {
    #[default]
    Discovered,
    Scanned,
    Planned,
    WaitingForProcesses,
    Staging,
    ValidatingStage,
    Committing,
    ValidatingCommit,
    Completed,
    CompletedWithWarnings,
    Cancelled,
    FailedRecoverable,
    FailedManualActionRequired,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationDomainState {
    #[default]
    NotStarted,
    Staged,
    Committed,
    Verified,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationDomainResult {
    pub domain: MigrationDomainId,
    pub state: MigrationDomainState,
    pub imported: u64,
    pub skipped: u64,
    pub conflicts: u64,
    pub warnings: Vec<MigrationDiagnostic>,
    /// Non-sensitive logical identifiers whose credentials must be entered again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_reauthentication: Vec<String>,
    /// Non-sensitive logical identifiers whose filesystem location must be repaired.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_relocation: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationDiagnostic {
    pub code: String,
    pub severity: FindingSeverity,
    pub domain: Option<MigrationDomainId>,
    pub relative_path: Option<String>,
    pub message: String,
    pub action: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationRunReport {
    pub format_version: u32,
    pub run_id: String,
    pub source_fingerprint: String,
    pub plan_hash: String,
    pub status: MigrationRunStatus,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub domain_results: Vec<MigrationDomainResult>,
    pub diagnostics: Vec<MigrationDiagnostic>,
    pub requires_reauthentication: Vec<String>,
    pub requires_relocation: Vec<String>,
}

/// Content-free projection that may be used for migration release observation.
///
/// Keep this shape limited to the result code, per-domain state, elapsed time,
/// and failure phase. In particular, it must not grow run identifiers, source
/// fingerprints, paths, counts, diagnostics, or user-authored content.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationReleaseObservation {
    pub result_code: String,
    pub domain_states: Vec<MigrationDomainObservation>,
    pub duration_ms: u64,
    pub failure_phase: Option<MigrationPhase>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationDomainObservation {
    pub domain: MigrationDomainId,
    pub state: MigrationDomainState,
}

/// A content-free diagnostic code included in an explicit failure export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationFailureDiagnosticCode {
    pub code: String,
    pub severity: FindingSeverity,
    pub domain: Option<MigrationDomainId>,
}

/// Sanitized journal evidence included in an explicit failure export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationFailureJournalEntry {
    pub sequence: u64,
    pub status: MigrationRunStatus,
    pub phase: MigrationPhase,
    pub domain: Option<MigrationDomainId>,
    pub domain_state: Option<MigrationDomainState>,
    pub code: String,
}

/// Shareable failure evidence without source identity, paths, secrets, or text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationFailureDiagnostics {
    pub format_version: u32,
    pub observation: MigrationReleaseObservation,
    pub diagnostic_codes: Vec<MigrationFailureDiagnosticCode>,
    pub journal: Vec<MigrationFailureJournalEntry>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPromptChoice {
    #[default]
    Unset,
    MigrateNow,
    RemindLater,
    DoNotRemind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationOnboardingState {
    pub format_version: u32,
    pub source_fingerprint: String,
    pub detected_at_ms: Option<i64>,
    /// Most recent explicit read-only scan started from a product entry point.
    pub last_scanned_at_ms: Option<i64>,
    pub choice: MigrationPromptChoice,
    pub last_prompted_version: Option<String>,
    pub run_id: Option<String>,
    /// Most recent run whose report can be shown after a later handoff starts.
    pub last_report_run_id: Option<String>,
    pub handled_run_id: Option<String>,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MigratorRequestMode {
    #[default]
    Onboarding,
    Execute,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigratorRequestOrigin {
    #[default]
    FirstLaunch,
    Settings,
    Installer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigratorProtocolCapability {
    ReadOnlyScan,
    OfflineExecute,
    JournalRecovery,
    SafeCancellation,
    TrustedRestart,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigratorProtocolCapabilities {
    pub protocol_version: u32,
    pub minimum_compatible_version: u32,
    pub supported_modes: BTreeSet<MigratorRequestMode>,
    pub capabilities: BTreeSet<MigratorProtocolCapability>,
}

impl MigratorProtocolCapabilities {
    pub fn current() -> Self {
        Self {
            protocol_version: CURRENT_MIGRATOR_PROTOCOL_VERSION,
            minimum_compatible_version: MIN_MIGRATOR_PROTOCOL_VERSION,
            supported_modes: BTreeSet::from([
                MigratorRequestMode::Onboarding,
                MigratorRequestMode::Execute,
            ]),
            capabilities: BTreeSet::from([
                MigratorProtocolCapability::ReadOnlyScan,
                MigratorProtocolCapability::OfflineExecute,
                MigratorProtocolCapability::JournalRecovery,
                MigratorProtocolCapability::SafeCancellation,
                MigratorProtocolCapability::TrustedRestart,
            ]),
        }
    }

    pub fn accepts(&self, version: u32, mode: MigratorRequestMode) -> bool {
        version >= self.minimum_compatible_version
            && version <= self.protocol_version
            && self.supported_modes.contains(&mode)
    }

    pub fn accepts_request(&self, request: &MigratorHandoffRequest) -> bool {
        self.accepts(request.protocol_version, request.mode)
            && request.required_capabilities.is_subset(&self.capabilities)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigratorHandoffRequest {
    pub protocol_version: u32,
    pub mode: MigratorRequestMode,
    pub origin: MigratorRequestOrigin,
    pub run_id: String,
    pub nonce: String,
    pub source_id: Option<String>,
    pub source_fingerprint: Option<String>,
    pub selection: MigrationSelection,
    pub caller_process_id: u32,
    pub product_id: String,
    pub release_channel: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub required_capabilities: BTreeSet<MigratorProtocolCapability>,
}

impl MigratorHandoffRequest {
    pub fn is_expired_at(&self, now_ms: i64) -> bool {
        now_ms > self.expires_at_ms
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationRemoteStance {
    #[default]
    ControllerLocal,
    UnsupportedRemoteControl,
    UnsupportedDetachedDispatch,
}

/// Append-only recovery evidence emitted by the offline migration engine.
///
/// New fields must remain additive and defaultable because a newer migrator can
/// resume a journal created by an older installed build.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MigrationJournalEvent {
    pub format_version: u32,
    pub sequence: u64,
    pub recorded_at_ms: i64,
    pub run_id: String,
    pub status: MigrationRunStatus,
    pub phase: MigrationPhase,
    pub domain: Option<MigrationDomainId>,
    pub domain_state: Option<MigrationDomainState>,
    pub code: String,
}
