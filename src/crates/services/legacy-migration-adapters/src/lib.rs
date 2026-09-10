//! Offline legacy BitFun format adapters.
//!
//! The generic service crate owns offline orchestration and filesystem safety;
//! this crate binds readers and converters to shared data contracts and storage
//! services without selecting or starting any product runtime.

mod agent_coordination;
mod common;
mod extensions;
mod memory;
mod remote_connect;
mod remote_ssh;
mod settings;
mod workspace_sessions;
pub use workspace_sessions::{workspace_report_counts, MigrationItemCounts, WorkspaceReportCounts};

use openbitfun_legacy_migration::{
    DomainContext, DomainScan, LegacyDomainAdapter, LegacyMigrationResult, MigrationRoots,
};
use openbitfun_product_domains::legacy_migration::{
    FindingSeverity, MigrationDomainId, MigrationDomainResult, MigrationDomainState,
    MigrationGroupId, MigrationSelection, ScanFinding,
};

pub fn adapters_for_groups(selection: &MigrationSelection) -> Vec<Box<dyn LegacyDomainAdapter>> {
    let selected = selection.expanded_domains();
    let mut adapters: Vec<Box<dyn LegacyDomainAdapter>> = Vec::new();
    if selected.contains(&MigrationDomainId::Settings) {
        adapters.push(Box::new(settings::SettingsAdapter));
    }
    if selected.contains(&MigrationDomainId::Credentials) {
        adapters.push(Box::new(settings::CredentialsAdapter));
    }
    if selected.contains(&MigrationDomainId::Skills) {
        adapters.push(Box::new(extensions::SkillsAdapter));
    }
    if selected.contains(&MigrationDomainId::Miniapps) {
        adapters.push(Box::new(extensions::MiniappsAdapter));
    }
    if selected.contains(&MigrationDomainId::Agents) {
        adapters.push(Box::new(extensions::AgentsAdapter));
    }
    if selected.contains(&MigrationDomainId::WorkspaceSessions) {
        adapters.push(Box::new(workspace_sessions::WorkspaceSessionsAdapter));
    }
    if selected.contains(&MigrationDomainId::AgentCoordination) {
        adapters.push(Box::new(agent_coordination::AgentCoordinationAdapter));
    }
    if selected.contains(&MigrationDomainId::StructuredMemory) {
        adapters.push(Box::new(memory::StructuredMemoryAdapter));
    }
    if selected.contains(&MigrationDomainId::FileMemory) {
        adapters.push(Box::new(memory::FileMemoryAdapter));
    }
    if selected.contains(&MigrationDomainId::RemoteConnectDevices) {
        adapters.push(Box::new(remote_connect::RemoteConnectAdapter));
    }
    if selected.contains(&MigrationDomainId::RemoteSsh) {
        adapters.push(Box::new(remote_ssh::RemoteSshAdapter));
    }
    if selected.contains(&MigrationDomainId::CrossReferenceRepair) {
        adapters.push(Box::new(CrossReferenceAdapter {
            dependencies: selected
                .iter()
                .copied()
                .filter(|domain| *domain == MigrationDomainId::AgentCoordination)
                .collect(),
        }));
    }
    adapters
}

/// Fail closed before any domain writes when the destination belongs to an
/// unknown product/schema. Empty destinations are initialized by the adapters.
pub fn validate_target(roots: &MigrationRoots) -> LegacyMigrationResult<()> {
    use openbitfun_legacy_migration::LegacyMigrationError;
    let path = roots.target_user_root.join("config/app.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(LegacyMigrationError::Io { path, source }),
    };
    let value = serde_json::from_slice(&bytes)
        .map_err(|source| LegacyMigrationError::Json { path, source })?;
    openbitfun_config_contracts::validate_current_config_value(&value, "migration destination")
        .map_err(LegacyMigrationError::UnsupportedTarget)
}

struct CrossReferenceAdapter {
    dependencies: Vec<MigrationDomainId>,
}

impl LegacyDomainAdapter for CrossReferenceAdapter {
    fn domain(&self) -> MigrationDomainId {
        MigrationDomainId::CrossReferenceRepair
    }

    fn scan(&self, _roots: &MigrationRoots) -> LegacyMigrationResult<DomainScan> {
        Ok(DomainScan {
            finding: ScanFinding {
                domain: self.domain(),
                code: "cross_reference_validation_enabled".to_string(),
                severity: FindingSeverity::Info,
                migratable: true,
                detail: "Selected owner adapters will be checked after their atomic commits."
                    .to_string(),
                ..ScanFinding::default()
            },
            conflicts: Vec::new(),
            target_schema: Some("openbitfun.cross-references.current".to_string()),
            dependencies: self.dependencies.clone(),
        })
    }

    fn stage(&self, _context: &DomainContext<'_>) -> LegacyMigrationResult<MigrationDomainResult> {
        Ok(MigrationDomainResult {
            domain: self.domain(),
            state: MigrationDomainState::Staged,
            ..MigrationDomainResult::default()
        })
    }

    fn validate_stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        validate_selected_cross_references(context)
    }

    fn commit(&self, _context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        Ok(())
    }

    fn validate_commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        validate_selected_cross_references(context)
    }
}

fn validate_selected_cross_references(context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
    if context
        .plan
        .selection
        .groups
        .contains(&MigrationGroupId::WorkspacesSessionsAndTasks)
    {
        agent_coordination::validate_committed_coordination_storage(context)?;
    }
    Ok(())
}
