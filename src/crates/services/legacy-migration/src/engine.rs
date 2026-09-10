use crate::{
    atomic_write_json, diagnostics::persist_release_observation, probe_legacy_source,
    LegacyMigrationError, LegacyMigrationResult, MigrationLayout, MigrationLock, MigrationRoots,
    ProbeLimits,
};
use openbitfun_product_domains::legacy_migration::{
    FindingSeverity, LegacySourceDescriptor, MigrationConflict, MigrationDiagnostic,
    MigrationDomainId, MigrationDomainResult, MigrationDomainState, MigrationJournalEvent,
    MigrationPhase, MigrationPlan, MigrationPlanStep, MigrationProgressEvent, MigrationRunReport,
    MigrationRunStatus, MigrationSelection, CURRENT_MIGRATION_FORMAT_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainScan {
    pub finding: openbitfun_product_domains::legacy_migration::ScanFinding,
    pub conflicts: Vec<MigrationConflict>,
    pub target_schema: Option<String>,
    pub dependencies: Vec<MigrationDomainId>,
}

impl DomainScan {
    pub fn estimated_write_bytes(&self) -> u64 {
        self.finding.logical_bytes
    }
}

pub struct DomainContext<'a> {
    pub roots: &'a MigrationRoots,
    pub layout: &'a MigrationLayout,
    pub plan: &'a MigrationPlan,
    pub step: &'a MigrationPlanStep,
}

/// Owner-provided legacy reader, converter, writer, and validator.
///
/// `commit` must be idempotent for the same immutable plan. A process can die
/// after the owner writes its target but before the engine records completion,
/// so recovery is allowed to call `commit` again. Implementations must never
/// execute imported content and must keep the legacy source read-only.
pub trait LegacyDomainAdapter: Send + Sync {
    fn domain(&self) -> MigrationDomainId;

    fn scan(&self, roots: &MigrationRoots) -> LegacyMigrationResult<DomainScan>;

    fn stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<MigrationDomainResult>;

    fn validate_stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()>;

    fn commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()>;

    fn validate_commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()>;

    /// Refresh non-sensitive result metadata discovered only while committing.
    ///
    /// Secret-bearing adapters deliberately defer decryption until `commit`.
    /// They can persist a redacted outcome and surface warnings or
    /// reauthentication identifiers here after the owner has validated the
    /// installed data.
    fn finalize_result(
        &self,
        _context: &DomainContext<'_>,
        staged: &MigrationDomainResult,
    ) -> LegacyMigrationResult<MigrationDomainResult> {
        Ok(staged.clone())
    }

    fn rollback_unverified(&self, _context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashPoint {
    AfterPlanPersisted,
    AfterLockAcquired,
    BeforeStage(MigrationDomainId),
    AfterStage(MigrationDomainId),
    AfterStageValidated(MigrationDomainId),
    BeforeCommit(MigrationDomainId),
    AfterCommit(MigrationDomainId),
    AfterCommitValidated(MigrationDomainId),
    BeforeFinalize,
}

pub trait CrashInjector {
    fn should_crash(&self, point: CrashPoint) -> bool;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoCrashInjection;

impl CrashInjector for NoCrashInjection {
    fn should_crash(&self, _point: CrashPoint) -> bool {
        false
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn check(&self) -> LegacyMigrationResult<()> {
        if self.is_cancelled() {
            Err(LegacyMigrationError::Cancelled)
        } else {
            Ok(())
        }
    }
}

pub struct MigrationEngine {
    roots: MigrationRoots,
    adapters: BTreeMap<MigrationDomainId, Box<dyn LegacyDomainAdapter>>,
}

impl MigrationEngine {
    pub fn new(
        roots: MigrationRoots,
        adapters: impl IntoIterator<Item = Box<dyn LegacyDomainAdapter>>,
    ) -> LegacyMigrationResult<Self> {
        roots.validate_distinct()?;
        let mut by_domain = BTreeMap::new();
        for adapter in adapters {
            let domain = adapter.domain();
            if by_domain.insert(domain, adapter).is_some() {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "duplicate migration adapter for {domain:?}"
                )));
            }
        }
        Ok(Self {
            roots,
            adapters: by_domain,
        })
    }

    pub fn scan(
        &self,
        selection: &MigrationSelection,
        cancellation: &CancellationToken,
    ) -> LegacyMigrationResult<Vec<DomainScan>> {
        let mut scans = Vec::new();
        for domain in selection.expanded_domains() {
            cancellation.check()?;
            let adapter = self.adapter(domain)?;
            let scan = match adapter.scan(&self.roots) {
                Ok(scan) => scan,
                Err(LegacyMigrationError::Cancelled) => {
                    return Err(LegacyMigrationError::Cancelled)
                }
                Err(error) => DomainScan {
                    finding: openbitfun_product_domains::legacy_migration::ScanFinding {
                        domain,
                        code: "domain_scan_skipped".to_string(),
                        severity: FindingSeverity::Warning,
                        migratable: false,
                        detail: error.to_string(),
                        ..Default::default()
                    },
                    conflicts: Vec::new(),
                    target_schema: None,
                    dependencies: Vec::new(),
                },
            };
            if scan.finding.domain != domain {
                return Err(LegacyMigrationError::InvalidPlan(format!(
                    "adapter {domain:?} returned a finding for {:?}",
                    scan.finding.domain
                )));
            }
            scans.push(scan);
        }
        // Dependencies may appear later in an adapter's scan results. Propagate
        // unavailable domains to a fixed point before authorizing any writes.
        loop {
            let unavailable = scans
                .iter()
                .filter(|scan| !scan.finding.migratable)
                .map(|scan| scan.finding.domain)
                .collect::<BTreeSet<_>>();
            let mut changed = false;
            for scan in &mut scans {
                if scan.finding.migratable
                    && scan.dependencies.iter().any(|id| unavailable.contains(id))
                {
                    scan.finding.migratable = false;
                    scan.finding.severity = FindingSeverity::Warning;
                    scan.finding.code = "domain_dependency_skipped".to_string();
                    scan.finding.detail =
                        "A required domain could not be scanned; this domain will be skipped."
                            .to_string();
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(scans)
    }

    pub fn plan(
        &self,
        source: &LegacySourceDescriptor,
        selection: MigrationSelection,
        cancellation: &CancellationToken,
    ) -> LegacyMigrationResult<MigrationPlan> {
        self.plan_with_run_id(
            source,
            selection,
            uuid::Uuid::new_v4().to_string(),
            cancellation,
        )
    }

    /// Build an immutable plan for a run id authenticated by the handoff file.
    pub fn plan_with_run_id(
        &self,
        source: &LegacySourceDescriptor,
        selection: MigrationSelection,
        run_id: impl Into<String>,
        cancellation: &CancellationToken,
    ) -> LegacyMigrationResult<MigrationPlan> {
        if !source.readable || !source.supported {
            return Err(LegacyMigrationError::UnsupportedSource(
                "legacy source is not readable and supported".to_string(),
            ));
        }
        let scans = self.scan(&selection, cancellation)?;
        let selected = selection
            .expanded_domains()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut steps = Vec::with_capacity(scans.len());
        let mut findings = Vec::with_capacity(scans.len());
        let mut conflicts = Vec::new();
        let mut estimated_write_bytes = 0u64;

        for (index, scan) in scans.into_iter().enumerate() {
            for dependency in &scan.dependencies {
                if !selected.contains(dependency) {
                    return Err(LegacyMigrationError::InvalidPlan(format!(
                        "{:?} requires unselected domain {dependency:?}",
                        scan.finding.domain
                    )));
                }
            }
            let write_bytes = if scan.finding.migratable {
                scan.estimated_write_bytes()
            } else {
                0
            };
            estimated_write_bytes = estimated_write_bytes.saturating_add(write_bytes);
            steps.push(MigrationPlanStep {
                sequence: u32::try_from(index + 1).unwrap_or(u32::MAX),
                domain: scan.finding.domain,
                estimated_write_bytes: write_bytes,
                source_schema: scan.finding.source_schema.clone(),
                target_schema: scan.target_schema,
                dependencies: scan.dependencies,
            });
            findings.push(scan.finding);
            conflicts.extend(scan.conflicts);
        }

        let run_id = run_id.into();
        if uuid::Uuid::parse_str(&run_id).is_err() {
            return Err(LegacyMigrationError::InvalidRequest(
                "migration run id must be a UUID".to_string(),
            ));
        }
        let mut plan = MigrationPlan {
            format_version: CURRENT_MIGRATION_FORMAT_VERSION,
            run_id,
            source_fingerprint: source.source_fingerprint.clone(),
            selection,
            steps,
            findings,
            conflicts,
            estimated_write_bytes,
            plan_hash: String::new(),
        };
        plan.plan_hash = compute_plan_hash(&plan)?;
        Ok(plan)
    }

    pub fn execute(
        &self,
        plan: &MigrationPlan,
        cancellation: &CancellationToken,
        crash_injector: &dyn CrashInjector,
    ) -> LegacyMigrationResult<MigrationRunReport> {
        self.execute_with_progress(plan, cancellation, crash_injector, |_| {})
    }

    pub fn execute_with_progress(
        &self,
        plan: &MigrationPlan,
        cancellation: &CancellationToken,
        crash_injector: &dyn CrashInjector,
        mut progress: impl FnMut(MigrationProgressEvent),
    ) -> LegacyMigrationResult<MigrationRunReport> {
        self.validate_plan(plan)?;
        let layout = MigrationLayout::new(&self.roots, &plan.run_id);
        layout.initialize()?;
        persist_or_validate_plan(&layout, plan)?;
        inject(crash_injector, CrashPoint::AfterPlanPersisted)?;

        cancellation.check()?;
        let _lock = MigrationLock::acquire(&layout)?;
        inject(crash_injector, CrashPoint::AfterLockAcquired)?;
        let current_source =
            probe_legacy_source(&self.roots, ProbeLimits::default())?.ok_or_else(|| {
                LegacyMigrationError::InvalidPlan(
                    "legacy source disappeared after the plan was created".to_string(),
                )
            })?;
        if current_source.source_fingerprint != plan.source_fingerprint {
            return Err(LegacyMigrationError::InvalidPlan(
                "legacy source changed after the plan was created".to_string(),
            ));
        }

        let mut report = load_or_create_report(&layout, plan)?;
        if matches!(
            report.status,
            MigrationRunStatus::Completed | MigrationRunStatus::CompletedWithWarnings
        ) {
            let result_code = match report.status {
                MigrationRunStatus::CompletedWithWarnings => "migration_completed_with_warnings",
                _ => "migration_completed",
            };
            persist_release_observation(
                &layout,
                &report,
                result_code,
                None,
                report.finished_at_ms.unwrap_or_else(now_ms),
            )?;
            return Ok(report);
        }
        normalize_report(plan, &mut report);
        let mut journal_sequence = journal_sequence(&layout)?;
        transition(
            &layout,
            &mut report,
            &mut journal_sequence,
            MigrationRunStatus::Staging,
            MigrationPhase::Acquire,
            None,
            None,
            "lock_acquired",
        )?;

        let mut last_failure = None;
        for (index, step) in plan.steps.iter().enumerate() {
            let result_index = report
                .domain_results
                .iter()
                .position(|result| result.domain == step.domain)
                .expect("normalized report includes every plan step");
            if report.domain_results[result_index].state == MigrationDomainState::Verified {
                continue;
            }
            let unavailable_dependency = step.dependencies.iter().any(|dependency| {
                !report.domain_results.iter().any(|result| {
                    result.domain == *dependency && result.state == MigrationDomainState::Verified
                })
            });
            let skipped_finding = plan
                .findings
                .iter()
                .find(|finding| finding.domain == step.domain && !finding.migratable);
            if skipped_finding.is_some() || unavailable_dependency {
                let code = skipped_finding
                    .map(|finding| finding.code.as_str())
                    .unwrap_or("domain_dependency_skipped");
                let result = &mut report.domain_results[result_index];
                result.state = MigrationDomainState::Skipped;
                if !result.warnings.iter().any(|warning| warning.code == code) {
                    result.warnings.push(MigrationDiagnostic {
                        code: code.to_string(),
                        severity: FindingSeverity::Warning,
                        domain: Some(step.domain),
                        message: "This domain was skipped; its source data remains available."
                            .to_string(),
                        ..Default::default()
                    });
                }
                persist_report(&layout, &report)?;
                continue;
            }
            // A dependency skipped on a previous attempt may now be available.
            if report.domain_results[result_index].state == MigrationDomainState::Skipped {
                report.domain_results[result_index].state = MigrationDomainState::NotStarted;
            }
            let context = DomainContext {
                roots: &self.roots,
                layout: &layout,
                plan,
                step,
            };
            let adapter = self.adapter(step.domain)?;

            let recovered_state = report.domain_results[result_index].state;
            if matches!(
                recovered_state,
                MigrationDomainState::NotStarted | MigrationDomainState::Failed
            ) {
                cancellation.check().map_err(|error| {
                    let _ = record_cancelled(&layout, &mut report, &mut journal_sequence);
                    error
                })?;
                inject(crash_injector, CrashPoint::BeforeStage(step.domain))?;
                emit_progress(
                    &mut progress,
                    plan,
                    step,
                    MigrationPhase::Stage,
                    index,
                    true,
                    "staging_domain",
                );
                transition(
                    &layout,
                    &mut report,
                    &mut journal_sequence,
                    MigrationRunStatus::Staging,
                    MigrationPhase::Stage,
                    Some(step.domain),
                    Some(MigrationDomainState::NotStarted),
                    "stage_started",
                )?;
                let mut staged = match adapter.stage(&context) {
                    Ok(staged) => staged,
                    Err(error) => {
                        record_domain_failure(
                            &layout,
                            &mut report,
                            &mut journal_sequence,
                            result_index,
                            step.domain,
                            MigrationPhase::Stage,
                            &error,
                        )?;
                        if adapter.rollback_unverified(&context).is_err() {
                            return Err(LegacyMigrationError::Domain {
                                domain: step.domain,
                                message: error.to_string(),
                            });
                        }
                        cancellation.check()?;
                        last_failure = Some((step.domain, MigrationPhase::Stage, error));
                        continue;
                    }
                };
                if staged.domain != step.domain {
                    return Err(LegacyMigrationError::InvalidPlan(format!(
                        "adapter {:?} staged a result for {:?}",
                        step.domain, staged.domain
                    )));
                }
                staged.state = MigrationDomainState::Staged;
                report.domain_results[result_index] = staged;
                persist_report(&layout, &report)?;
                journal(
                    &layout,
                    &report,
                    &mut journal_sequence,
                    MigrationPhase::Stage,
                    Some(step.domain),
                    Some(MigrationDomainState::Staged),
                    "stage_completed",
                )?;
                inject(crash_injector, CrashPoint::AfterStage(step.domain))?;
            }

            if report.domain_results[result_index].state == MigrationDomainState::Staged {
                cancellation.check().map_err(|error| {
                    let _ = record_cancelled(&layout, &mut report, &mut journal_sequence);
                    error
                })?;
                emit_progress(
                    &mut progress,
                    plan,
                    step,
                    MigrationPhase::ValidateStage,
                    index,
                    true,
                    "validating_staged_domain",
                );
                transition(
                    &layout,
                    &mut report,
                    &mut journal_sequence,
                    MigrationRunStatus::ValidatingStage,
                    MigrationPhase::ValidateStage,
                    Some(step.domain),
                    Some(MigrationDomainState::Staged),
                    "stage_validation_started",
                )?;
                if let Err(error) = adapter.validate_stage(&context) {
                    record_domain_failure(
                        &layout,
                        &mut report,
                        &mut journal_sequence,
                        result_index,
                        step.domain,
                        MigrationPhase::ValidateStage,
                        &error,
                    )?;
                    if adapter.rollback_unverified(&context).is_err() {
                        return Err(LegacyMigrationError::Domain {
                            domain: step.domain,
                            message: error.to_string(),
                        });
                    }
                    cancellation.check()?;
                    last_failure = Some((step.domain, MigrationPhase::ValidateStage, error));
                    continue;
                }
                journal(
                    &layout,
                    &report,
                    &mut journal_sequence,
                    MigrationPhase::ValidateStage,
                    Some(step.domain),
                    Some(MigrationDomainState::Staged),
                    "stage_validated",
                )?;
                inject(crash_injector, CrashPoint::AfterStageValidated(step.domain))?;
            }

            if report.domain_results[result_index].state == MigrationDomainState::Staged {
                cancellation.check().map_err(|error| {
                    let _ = record_cancelled(&layout, &mut report, &mut journal_sequence);
                    error
                })?;
                emit_progress(
                    &mut progress,
                    plan,
                    step,
                    MigrationPhase::Commit,
                    index,
                    false,
                    "committing_domain",
                );
                transition(
                    &layout,
                    &mut report,
                    &mut journal_sequence,
                    MigrationRunStatus::Committing,
                    MigrationPhase::Commit,
                    Some(step.domain),
                    Some(MigrationDomainState::Staged),
                    "commit_intent",
                )?;
                inject(crash_injector, CrashPoint::BeforeCommit(step.domain))?;
                if let Err(error) = adapter.commit(&context) {
                    record_domain_failure(
                        &layout,
                        &mut report,
                        &mut journal_sequence,
                        result_index,
                        step.domain,
                        MigrationPhase::Commit,
                        &error,
                    )?;
                    if adapter.rollback_unverified(&context).is_err() {
                        return Err(LegacyMigrationError::Domain {
                            domain: step.domain,
                            message: error.to_string(),
                        });
                    }
                    cancellation.check()?;
                    last_failure = Some((step.domain, MigrationPhase::Commit, error));
                    continue;
                }
                inject(crash_injector, CrashPoint::AfterCommit(step.domain))?;
                report.domain_results[result_index].state = MigrationDomainState::Committed;
                persist_report(&layout, &report)?;
                journal(
                    &layout,
                    &report,
                    &mut journal_sequence,
                    MigrationPhase::Commit,
                    Some(step.domain),
                    Some(MigrationDomainState::Committed),
                    "commit_completed",
                )?;
            }

            if report.domain_results[result_index].state == MigrationDomainState::Committed {
                emit_progress(
                    &mut progress,
                    plan,
                    step,
                    MigrationPhase::ValidateCommit,
                    index,
                    false,
                    "validating_committed_domain",
                );
                transition(
                    &layout,
                    &mut report,
                    &mut journal_sequence,
                    MigrationRunStatus::ValidatingCommit,
                    MigrationPhase::ValidateCommit,
                    Some(step.domain),
                    Some(MigrationDomainState::Committed),
                    "commit_validation_started",
                )?;
                if let Err(error) = adapter.validate_commit(&context) {
                    record_domain_failure(
                        &layout,
                        &mut report,
                        &mut journal_sequence,
                        result_index,
                        step.domain,
                        MigrationPhase::ValidateCommit,
                        &error,
                    )?;
                    if adapter.rollback_unverified(&context).is_err() {
                        return Err(LegacyMigrationError::Domain {
                            domain: step.domain,
                            message: error.to_string(),
                        });
                    }
                    cancellation.check()?;
                    last_failure = Some((step.domain, MigrationPhase::ValidateCommit, error));
                    continue;
                }
                let mut finalized = match adapter
                    .finalize_result(&context, &report.domain_results[result_index])
                {
                    Ok(finalized) => finalized,
                    Err(error) => {
                        record_domain_failure(
                            &layout,
                            &mut report,
                            &mut journal_sequence,
                            result_index,
                            step.domain,
                            MigrationPhase::ValidateCommit,
                            &error,
                        )?;
                        if adapter.rollback_unverified(&context).is_err() {
                            return Err(LegacyMigrationError::Domain {
                                domain: step.domain,
                                message: error.to_string(),
                            });
                        }
                        cancellation.check()?;
                        last_failure = Some((step.domain, MigrationPhase::ValidateCommit, error));
                        continue;
                    }
                };
                if finalized.domain != step.domain {
                    let error = LegacyMigrationError::InvalidPlan(format!(
                        "adapter {:?} finalized a result for {:?}",
                        step.domain, finalized.domain
                    ));
                    record_domain_failure(
                        &layout,
                        &mut report,
                        &mut journal_sequence,
                        result_index,
                        step.domain,
                        MigrationPhase::ValidateCommit,
                        &error,
                    )?;
                    let _ = adapter.rollback_unverified(&context);
                    return Err(error);
                }
                finalized.state = MigrationDomainState::Verified;
                report.domain_results[result_index] = finalized;
                persist_report(&layout, &report)?;
                journal(
                    &layout,
                    &report,
                    &mut journal_sequence,
                    MigrationPhase::ValidateCommit,
                    Some(step.domain),
                    Some(MigrationDomainState::Verified),
                    "commit_verified",
                )?;
                inject(
                    crash_injector,
                    CrashPoint::AfterCommitValidated(step.domain),
                )?;
                emit_progress(
                    &mut progress,
                    plan,
                    step,
                    MigrationPhase::ValidateCommit,
                    index + 1,
                    true,
                    "domain_verified",
                );
            }
        }

        inject(crash_injector, CrashPoint::BeforeFinalize)?;
        report.requires_reauthentication = report
            .domain_results
            .iter()
            .flat_map(|result| result.requires_reauthentication.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        report.requires_relocation = report
            .domain_results
            .iter()
            .flat_map(|result| result.requires_relocation.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        report.finished_at_ms = Some(now_ms());
        let has_warnings = report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.severity,
                FindingSeverity::Warning | FindingSeverity::Blocking
            )
        }) || report.domain_results.iter().any(|result| {
            result.warnings.iter().any(|diagnostic| {
                matches!(
                    diagnostic.severity,
                    FindingSeverity::Warning | FindingSeverity::Blocking
                )
            })
        });
        let has_failures = report
            .domain_results
            .iter()
            .any(|result| result.state == MigrationDomainState::Failed);
        let final_status = if has_failures {
            MigrationRunStatus::FailedRecoverable
        } else if has_warnings {
            MigrationRunStatus::CompletedWithWarnings
        } else {
            MigrationRunStatus::Completed
        };
        let final_code = if has_failures {
            "migration_partially_failed"
        } else if has_warnings {
            "migration_completed_with_warnings"
        } else {
            "migration_completed"
        };
        if let Some((domain, phase, error)) = last_failure {
            transition(
                &layout,
                &mut report,
                &mut journal_sequence,
                MigrationRunStatus::FailedRecoverable,
                phase,
                Some(domain),
                Some(MigrationDomainState::Failed),
                "domain_failed_recoverable",
            )?;
            return Err(LegacyMigrationError::Domain {
                domain,
                message: error.to_string(),
            });
        }
        transition(
            &layout,
            &mut report,
            &mut journal_sequence,
            final_status,
            MigrationPhase::Finalize,
            None,
            None,
            final_code,
        )?;
        emit_progress(
            &mut progress,
            plan,
            plan.steps.last().unwrap_or(&MigrationPlanStep::default()),
            MigrationPhase::Finalize,
            plan.steps.len(),
            true,
            final_code,
        );
        Ok(report)
    }

    fn adapter(
        &self,
        domain: MigrationDomainId,
    ) -> LegacyMigrationResult<&dyn LegacyDomainAdapter> {
        self.adapters.get(&domain).map(Box::as_ref).ok_or_else(|| {
            LegacyMigrationError::InvalidRequest(format!(
                "no migration adapter registered for {domain:?}"
            ))
        })
    }

    fn validate_plan(&self, plan: &MigrationPlan) -> LegacyMigrationResult<()> {
        if plan.format_version != CURRENT_MIGRATION_FORMAT_VERSION {
            return Err(LegacyMigrationError::InvalidPlan(format!(
                "unsupported plan format {}",
                plan.format_version
            )));
        }
        if plan.run_id.is_empty() || plan.source_fingerprint.is_empty() {
            return Err(LegacyMigrationError::InvalidPlan(
                "plan identity is incomplete".to_string(),
            ));
        }
        if compute_plan_hash(plan)? != plan.plan_hash {
            return Err(LegacyMigrationError::InvalidPlan(
                "plan hash does not match the immutable plan".to_string(),
            ));
        }
        let expected = plan.selection.expanded_domains();
        let actual = plan
            .steps
            .iter()
            .map(|step| step.domain)
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(LegacyMigrationError::InvalidPlan(
                "plan steps do not match the selected dependency order".to_string(),
            ));
        }
        for (index, step) in plan.steps.iter().enumerate() {
            if step.sequence != u32::try_from(index + 1).unwrap_or(u32::MAX) {
                return Err(LegacyMigrationError::InvalidPlan(
                    "plan step sequence is not contiguous".to_string(),
                ));
            }
            self.adapter(step.domain)?;
        }
        Ok(())
    }
}

pub fn compute_plan_hash(plan: &MigrationPlan) -> LegacyMigrationResult<String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct HashInput<'a> {
        format_version: u32,
        source_fingerprint: &'a str,
        selection: &'a MigrationSelection,
        steps: &'a [MigrationPlanStep],
        findings: &'a [openbitfun_product_domains::legacy_migration::ScanFinding],
        conflicts: &'a [MigrationConflict],
        estimated_write_bytes: u64,
    }

    let input = HashInput {
        format_version: plan.format_version,
        source_fingerprint: &plan.source_fingerprint,
        selection: &plan.selection,
        steps: &plan.steps,
        findings: &plan.findings,
        conflicts: &plan.conflicts,
        estimated_write_bytes: plan.estimated_write_bytes,
    };
    let bytes = serde_json::to_vec(&input)
        .map_err(|error| LegacyMigrationError::json("<migration-plan-hash>", error))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn persist_or_validate_plan(
    layout: &MigrationLayout,
    plan: &MigrationPlan,
) -> LegacyMigrationResult<()> {
    if let Some(existing) = layout.read_json::<MigrationPlan>(&layout.plan_path())? {
        if existing != *plan {
            return Err(LegacyMigrationError::InvalidPlan(
                "run id already has a different immutable plan".to_string(),
            ));
        }
        return Ok(());
    }
    atomic_write_json(&layout.plan_path(), plan)
}

fn load_or_create_report(
    layout: &MigrationLayout,
    plan: &MigrationPlan,
) -> LegacyMigrationResult<MigrationRunReport> {
    if let Some(report) = layout.read_json::<MigrationRunReport>(&layout.report_path())? {
        if report.run_id != plan.run_id
            || report.source_fingerprint != plan.source_fingerprint
            || report.plan_hash != plan.plan_hash
        {
            return Err(LegacyMigrationError::InvalidPlan(
                "persisted report does not belong to this plan".to_string(),
            ));
        }
        return Ok(report);
    }
    let report = MigrationRunReport {
        format_version: CURRENT_MIGRATION_FORMAT_VERSION,
        run_id: plan.run_id.clone(),
        source_fingerprint: plan.source_fingerprint.clone(),
        plan_hash: plan.plan_hash.clone(),
        status: MigrationRunStatus::Planned,
        started_at_ms: now_ms(),
        domain_results: plan
            .steps
            .iter()
            .map(|step| MigrationDomainResult {
                domain: step.domain,
                ..MigrationDomainResult::default()
            })
            .collect(),
        ..MigrationRunReport::default()
    };
    persist_report(layout, &report)?;
    persist_release_observation(
        layout,
        &report,
        "migration_planned",
        None,
        report.started_at_ms,
    )?;
    Ok(report)
}

fn normalize_report(plan: &MigrationPlan, report: &mut MigrationRunReport) {
    for step in &plan.steps {
        if !report
            .domain_results
            .iter()
            .any(|result| result.domain == step.domain)
        {
            report.domain_results.push(MigrationDomainResult {
                domain: step.domain,
                ..MigrationDomainResult::default()
            });
        }
    }
    report
        .domain_results
        .retain(|result| plan.steps.iter().any(|step| step.domain == result.domain));
    report.domain_results.sort_by_key(|result| {
        plan.steps
            .iter()
            .position(|step| step.domain == result.domain)
            .unwrap_or(usize::MAX)
    });
}

fn transition(
    layout: &MigrationLayout,
    report: &mut MigrationRunReport,
    sequence: &mut u64,
    status: MigrationRunStatus,
    phase: MigrationPhase,
    domain: Option<MigrationDomainId>,
    domain_state: Option<MigrationDomainState>,
    code: &str,
) -> LegacyMigrationResult<()> {
    report.status = status;
    persist_report(layout, report)?;
    journal(layout, report, sequence, phase, domain, domain_state, code)
}

fn journal(
    layout: &MigrationLayout,
    report: &MigrationRunReport,
    sequence: &mut u64,
    phase: MigrationPhase,
    domain: Option<MigrationDomainId>,
    domain_state: Option<MigrationDomainState>,
    code: &str,
) -> LegacyMigrationResult<()> {
    *sequence = sequence.saturating_add(1);
    let event = MigrationJournalEvent {
        format_version: CURRENT_MIGRATION_FORMAT_VERSION,
        sequence: *sequence,
        recorded_at_ms: now_ms(),
        run_id: report.run_id.clone(),
        status: report.status,
        phase,
        domain,
        domain_state,
        code: code.to_string(),
    };
    layout.append_journal(&event)?;
    let failure_phase = matches!(
        report.status,
        MigrationRunStatus::FailedRecoverable | MigrationRunStatus::FailedManualActionRequired
    )
    .then_some(phase);
    persist_release_observation(layout, report, code, failure_phase, event.recorded_at_ms)
}

fn record_domain_failure(
    layout: &MigrationLayout,
    report: &mut MigrationRunReport,
    sequence: &mut u64,
    result_index: usize,
    domain: MigrationDomainId,
    phase: MigrationPhase,
    error: &LegacyMigrationError,
) -> LegacyMigrationResult<()> {
    report.status = MigrationRunStatus::FailedRecoverable;
    report.domain_results[result_index].state = MigrationDomainState::Failed;
    report
        .diagnostics
        .push(domain_failure_diagnostic(domain, error));
    persist_report(layout, report)?;
    journal(
        layout,
        report,
        sequence,
        phase,
        Some(domain),
        Some(MigrationDomainState::Failed),
        "domain_failed_recoverable",
    )
}

fn domain_failure_diagnostic(
    domain: MigrationDomainId,
    error: &LegacyMigrationError,
) -> MigrationDiagnostic {
    let (code, message, action) = match error {
        LegacyMigrationError::UnsupportedSource(_) => (
            "domain_source_unsupported",
            "A migration source record does not match a supported legacy format.",
            "Keep the legacy data unchanged and export diagnostics for review.",
        ),
        LegacyMigrationError::PathEscape(_) | LegacyMigrationError::LinkedPath(_) => (
            "domain_path_unsafe",
            "A migration path failed its safety checks.",
            "Review linked or redirected paths before retrying the migration.",
        ),
        LegacyMigrationError::ResourceLimit(_) => (
            "domain_resource_limit",
            "A migration domain exceeded a configured safety limit.",
            "Export diagnostics and review the size or shape of the legacy data.",
        ),
        LegacyMigrationError::Io { source, .. } => io_failure_diagnostic(source),
        LegacyMigrationError::Json { .. } => (
            "domain_json_invalid",
            "A migration-owned JSON document is unreadable or has an unexpected shape.",
            "Keep the legacy data unchanged and export diagnostics for review.",
        ),
        LegacyMigrationError::Sqlite { .. } => (
            "domain_sqlite_failed",
            "A migration-owned SQLite database could not be copied or validated.",
            "Close processes using the database and retry the migration.",
        ),
        _ => (
            "domain_validation_failed",
            "A migration-owned record failed validation.",
            "Keep the legacy data unchanged and export diagnostics for review.",
        ),
    };
    MigrationDiagnostic {
        code: code.to_string(),
        severity: FindingSeverity::Blocking,
        domain: Some(domain),
        message: message.to_string(),
        action: Some(action.to_string()),
        ..MigrationDiagnostic::default()
    }
}

fn io_failure_diagnostic(error: &std::io::Error) -> (&'static str, &'static str, &'static str) {
    if is_path_too_long(error) {
        return (
            "domain_path_too_long",
            "A migration-owned path exceeds the platform path limit.",
            "Enable long-path support or use a shorter data root before retrying.",
        );
    }
    if is_storage_full(error) {
        return (
            "domain_storage_full",
            "The destination volume does not have enough free space.",
            "Free destination disk space and retry the migration.",
        );
    }
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => (
            "domain_io_permission_denied",
            "A migration-owned file or directory denied access.",
            "Close programs using the data, check permissions, and retry.",
        ),
        std::io::ErrorKind::NotFound => (
            "domain_io_not_found",
            "A migration-owned source or staged file was not found.",
            "Run a new scan and retry after confirming the legacy data is still present.",
        ),
        std::io::ErrorKind::AlreadyExists => (
            "domain_io_conflict",
            "A migration target changed after the plan was created.",
            "Run a new scan and plan before retrying the migration.",
        ),
        std::io::ErrorKind::InvalidData | std::io::ErrorKind::InvalidInput => (
            "domain_io_invalid_data",
            "A migration-owned file could not be processed safely.",
            "Keep the legacy data unchanged and export diagnostics for review.",
        ),
        _ => (
            "domain_io_failed",
            "A migration-owned file or directory could not be read or written.",
            "Export diagnostics, check the destination storage, and retry.",
        ),
    }
}

fn is_path_too_long(error: &std::io::Error) -> bool {
    cfg!(windows) && error.raw_os_error() == Some(206)
}

fn is_storage_full(error: &std::io::Error) -> bool {
    match error.raw_os_error() {
        Some(112) if cfg!(windows) => true,
        Some(28) if cfg!(unix) => true,
        _ => false,
    }
}

fn record_cancelled(
    layout: &MigrationLayout,
    report: &mut MigrationRunReport,
    sequence: &mut u64,
) -> LegacyMigrationResult<()> {
    transition(
        layout,
        report,
        sequence,
        MigrationRunStatus::Cancelled,
        MigrationPhase::Stage,
        None,
        None,
        "migration_cancelled",
    )
}

fn persist_report(
    layout: &MigrationLayout,
    report: &MigrationRunReport,
) -> LegacyMigrationResult<()> {
    atomic_write_json(&layout.report_path(), report)
}

fn journal_sequence(layout: &MigrationLayout) -> LegacyMigrationResult<u64> {
    let bytes = match std::fs::read(layout.journal_path()) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(LegacyMigrationError::io(layout.journal_path(), error)),
    };
    Ok(bytes.iter().filter(|byte| **byte == b'\n').count() as u64)
}

fn emit_progress(
    progress: &mut impl FnMut(MigrationProgressEvent),
    plan: &MigrationPlan,
    step: &MigrationPlanStep,
    phase: MigrationPhase,
    processed: usize,
    safe_to_cancel: bool,
    code: &str,
) {
    progress(MigrationProgressEvent {
        run_id: plan.run_id.clone(),
        domain: if phase == MigrationPhase::Finalize {
            None
        } else {
            Some(step.domain)
        },
        phase,
        processed: processed as u64,
        total: plan.steps.len() as u64,
        safe_to_cancel,
        code: code.to_string(),
    });
}

fn inject(injector: &dyn CrashInjector, point: CrashPoint) -> LegacyMigrationResult<()> {
    if injector.should_crash(point) {
        Err(LegacyMigrationError::InjectedCrash(point))
    } else {
        Ok(())
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
