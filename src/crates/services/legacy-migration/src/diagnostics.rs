use crate::{atomic_write_json, LegacyMigrationError, LegacyMigrationResult, MigrationLayout};
use openbitfun_product_domains::legacy_migration::{
    FindingSeverity, MigrationDiagnostic, MigrationDomainObservation, MigrationDomainState,
    MigrationFailureDiagnosticCode, MigrationFailureDiagnostics, MigrationFailureJournalEntry,
    MigrationJournalEvent, MigrationPhase, MigrationReleaseObservation, MigrationRunReport,
    MigrationRunStatus,
};
use std::fs;
use std::path::{Path, PathBuf};

const FAILURE_DIAGNOSTICS_FORMAT_VERSION: u32 = 1;
const MAX_DIAGNOSTIC_JOURNAL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DIAGNOSTIC_JOURNAL_ENTRIES: usize = 10_000;
const REDACTED_CODE: &str = "redacted_code";

pub(crate) fn persist_release_observation(
    layout: &MigrationLayout,
    report: &MigrationRunReport,
    result_code: &str,
    failure_phase: Option<MigrationPhase>,
    observed_at_ms: i64,
) -> LegacyMigrationResult<()> {
    let observation = release_observation(report, result_code, failure_phase, observed_at_ms);
    atomic_write_json(&layout.release_observation_path(), &observation)
}

pub fn release_observation(
    report: &MigrationRunReport,
    result_code: &str,
    failure_phase: Option<MigrationPhase>,
    observed_at_ms: i64,
) -> MigrationReleaseObservation {
    MigrationReleaseObservation {
        result_code: sanitize_code(result_code),
        domain_states: report
            .domain_results
            .iter()
            .map(|result| MigrationDomainObservation {
                domain: result.domain,
                state: result.state,
            })
            .collect(),
        duration_ms: elapsed_ms(report.started_at_ms, observed_at_ms),
        failure_phase,
    }
}

/// Write a shareable failure-only diagnostic artifact next to the run metadata.
///
/// The export deliberately projects trusted enums and sanitized result codes.
/// It never copies the request, plan, source fingerprint, diagnostic messages,
/// actions, paths, repair identifiers, entity counts, or user-authored content.
pub fn export_failure_diagnostics(
    layout: &MigrationLayout,
    report: &MigrationRunReport,
) -> LegacyMigrationResult<PathBuf> {
    if !matches!(
        report.status,
        MigrationRunStatus::FailedRecoverable | MigrationRunStatus::FailedManualActionRequired
    ) {
        return Err(LegacyMigrationError::InvalidRequest(
            "failure diagnostics require a failed migration report".to_string(),
        ));
    }

    let mut journal = read_sanitized_journal(&layout.journal_path())?;
    let failure = journal
        .entries
        .iter()
        .rev()
        .find(|entry| {
            matches!(
                entry.status,
                MigrationRunStatus::FailedRecoverable
                    | MigrationRunStatus::FailedManualActionRequired
            ) || entry.domain_state == Some(MigrationDomainState::Failed)
        })
        .cloned();
    let result_code = failure
        .as_ref()
        .map(|entry| entry.code.as_str())
        .unwrap_or_else(|| default_result_code(report.status));
    let failure_phase = failure.as_ref().map(|entry| entry.phase);
    let observed_at_ms = journal
        .last_recorded_at_ms
        .or(report.finished_at_ms)
        .unwrap_or(report.started_at_ms);

    let mut diagnostic_codes = report
        .diagnostics
        .iter()
        .chain(
            report
                .domain_results
                .iter()
                .flat_map(|result| result.warnings.iter()),
        )
        .map(sanitize_diagnostic)
        .collect::<Vec<_>>();
    diagnostic_codes.append(&mut journal.supplemental_diagnostics);
    deduplicate_diagnostics(&mut diagnostic_codes);

    let diagnostics = MigrationFailureDiagnostics {
        format_version: FAILURE_DIAGNOSTICS_FORMAT_VERSION,
        observation: release_observation(report, result_code, failure_phase, observed_at_ms),
        diagnostic_codes,
        journal: journal.entries,
    };
    let path = layout.failure_diagnostics_path();
    atomic_write_json(&path, &diagnostics)?;
    Ok(path)
}

struct SanitizedJournal {
    entries: Vec<MigrationFailureJournalEntry>,
    supplemental_diagnostics: Vec<MigrationFailureDiagnosticCode>,
    last_recorded_at_ms: Option<i64>,
}

fn read_sanitized_journal(path: &Path) -> LegacyMigrationResult<SanitizedJournal> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SanitizedJournal {
                entries: Vec::new(),
                supplemental_diagnostics: vec![diagnostic_code(
                    "journal_unavailable",
                    FindingSeverity::Warning,
                )],
                last_recorded_at_ms: None,
            });
        }
        Err(error) => return Err(LegacyMigrationError::io(path, error)),
    };
    if metadata.len() > MAX_DIAGNOSTIC_JOURNAL_BYTES {
        return Ok(SanitizedJournal {
            entries: Vec::new(),
            supplemental_diagnostics: vec![diagnostic_code(
                "journal_size_limit_reached",
                FindingSeverity::Warning,
            )],
            last_recorded_at_ms: None,
        });
    }

    let bytes = fs::read(path).map_err(|error| LegacyMigrationError::io(path, error))?;
    let mut entries = Vec::new();
    let mut supplemental_diagnostics = Vec::new();
    let mut last_recorded_at_ms = None;
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        if entries.len() >= MAX_DIAGNOSTIC_JOURNAL_ENTRIES {
            supplemental_diagnostics.push(diagnostic_code(
                "journal_entry_limit_reached",
                FindingSeverity::Warning,
            ));
            break;
        }
        match serde_json::from_slice::<MigrationJournalEvent>(line) {
            Ok(event) => {
                last_recorded_at_ms = Some(
                    last_recorded_at_ms.map_or(event.recorded_at_ms, |current: i64| {
                        current.max(event.recorded_at_ms)
                    }),
                );
                entries.push(MigrationFailureJournalEntry {
                    sequence: event.sequence,
                    status: event.status,
                    phase: event.phase,
                    domain: event.domain,
                    domain_state: event.domain_state,
                    code: sanitize_code(&event.code),
                });
            }
            Err(_) => supplemental_diagnostics.push(diagnostic_code(
                "journal_entry_invalid",
                FindingSeverity::Warning,
            )),
        }
    }

    Ok(SanitizedJournal {
        entries,
        supplemental_diagnostics,
        last_recorded_at_ms,
    })
}

fn sanitize_diagnostic(diagnostic: &MigrationDiagnostic) -> MigrationFailureDiagnosticCode {
    MigrationFailureDiagnosticCode {
        code: sanitize_code(&diagnostic.code),
        severity: diagnostic.severity,
        domain: diagnostic.domain,
    }
}

fn diagnostic_code(code: &str, severity: FindingSeverity) -> MigrationFailureDiagnosticCode {
    MigrationFailureDiagnosticCode {
        code: code.to_string(),
        severity,
        domain: None,
    }
}

fn deduplicate_diagnostics(diagnostics: &mut Vec<MigrationFailureDiagnosticCode>) {
    let mut unique = Vec::with_capacity(diagnostics.len());
    for diagnostic in diagnostics.drain(..) {
        if !unique.contains(&diagnostic) {
            unique.push(diagnostic);
        }
    }
    *diagnostics = unique;
}

fn sanitize_code(code: &str) -> String {
    if !code.is_empty()
        && code.len() <= 64
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        code.to_string()
    } else {
        REDACTED_CODE.to_string()
    }
}

fn default_result_code(status: MigrationRunStatus) -> &'static str {
    match status {
        MigrationRunStatus::Discovered => "discovered",
        MigrationRunStatus::Scanned => "scanned",
        MigrationRunStatus::Planned => "planned",
        MigrationRunStatus::WaitingForProcesses => "waiting_for_processes",
        MigrationRunStatus::Staging => "staging",
        MigrationRunStatus::ValidatingStage => "validating_stage",
        MigrationRunStatus::Committing => "committing",
        MigrationRunStatus::ValidatingCommit => "validating_commit",
        MigrationRunStatus::Completed => "completed",
        MigrationRunStatus::CompletedWithWarnings => "completed_with_warnings",
        MigrationRunStatus::Cancelled => "cancelled",
        MigrationRunStatus::FailedRecoverable => "failed_recoverable",
        MigrationRunStatus::FailedManualActionRequired => "failed_manual_action_required",
    }
}

fn elapsed_ms(started_at_ms: i64, observed_at_ms: i64) -> u64 {
    if started_at_ms <= 0 || observed_at_ms <= started_at_ms {
        return 0;
    }
    u64::try_from(observed_at_ms.saturating_sub(started_at_ms)).unwrap_or(u64::MAX)
}
