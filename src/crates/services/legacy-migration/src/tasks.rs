//! Durable offline tasks. Plans and journals remain compatible with older runs;
//! no expiring launch request, caller process, or Desktop lifecycle is involved.
use crate::{
    atomic_write_json, compute_plan_hash, LegacyMigrationError, LegacyMigrationResult,
    MigrationLayout, MigrationRoots,
};
use openbitfun_product_domains::legacy_migration::{
    MigrationPlan, MigrationRunReport, MigrationRunStatus, CURRENT_MIGRATION_FORMAT_VERSION,
};
use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedMigrationTask {
    pub run_id: String,
    pub status: MigrationRunStatus,
    pub readable: bool,
}

pub fn save_task(roots: &MigrationRoots, plan: &MigrationPlan) -> LegacyMigrationResult<()> {
    validate_plan(plan, &plan.run_id)?;
    let layout = MigrationLayout::new(roots, &plan.run_id);
    if let Some(existing) = layout.read_json::<MigrationPlan>(&layout.plan_path())? {
        if existing != *plan {
            return Err(invalid("a saved task has a different plan"));
        }
    }
    validate_locations(&layout, roots)?;
    layout.initialize()?;
    atomic_write_json(&layout.run_root().join("locations.json"), roots)?;
    atomic_write_json(&layout.plan_path(), plan)
}

pub fn load_task(
    roots: &MigrationRoots,
    run_id: &str,
) -> LegacyMigrationResult<(MigrationPlan, Option<MigrationRunReport>)> {
    validate_id(run_id)?;
    let layout = MigrationLayout::new(roots, run_id);
    let plan = layout
        .read_json::<MigrationPlan>(&layout.plan_path())?
        .ok_or_else(|| invalid("saved plan missing"))?;
    validate_plan(&plan, run_id)?;
    // Older handoff-based runs did not carry this file. Their plan and journal
    // remain readable using the locations explicitly selected by the user.
    validate_locations(&layout, roots)?;
    let report = layout.read_json::<MigrationRunReport>(&layout.report_path())?;
    if let Some(report) = &report {
        if report.run_id != run_id
            || report.plan_hash != plan.plan_hash
            || report.source_fingerprint != plan.source_fingerprint
            || report.format_version != CURRENT_MIGRATION_FORMAT_VERSION
        {
            return Err(invalid("saved report does not match the plan"));
        }
    }
    Ok((plan, report))
}

pub fn list_tasks(roots: &MigrationRoots) -> LegacyMigrationResult<Vec<SavedMigrationTask>> {
    let directory = roots.migration_root().join("runs");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(LegacyMigrationError::io(&directory, error)),
    };
    let mut tasks = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| LegacyMigrationError::io(&directory, error))?;
        if !entry
            .file_type()
            .map_err(|error| LegacyMigrationError::io(entry.path(), error))?
            .is_dir()
        {
            continue;
        }
        let run_id = entry.file_name().to_string_lossy().into_owned();
        if validate_id(&run_id).is_err() || !entry.path().join("plan.json").exists() {
            continue;
        }
        let loaded = load_task(roots, &run_id);
        tasks.push(SavedMigrationTask {
            run_id,
            status: loaded
                .as_ref()
                .ok()
                .and_then(|(_, report)| report.as_ref().map(|report| report.status))
                .unwrap_or(MigrationRunStatus::Planned),
            readable: loaded.is_ok(),
        });
    }
    tasks.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    Ok(tasks)
}

fn validate_id(run_id: &str) -> LegacyMigrationResult<()> {
    if uuid::Uuid::parse_str(run_id).is_err() {
        return Err(invalid("invalid task identifier"));
    }
    Ok(())
}
fn validate_plan(plan: &MigrationPlan, run_id: &str) -> LegacyMigrationResult<()> {
    validate_id(run_id)?;
    if plan.run_id != run_id
        || plan.source_fingerprint.is_empty()
        || plan.format_version != CURRENT_MIGRATION_FORMAT_VERSION
        || compute_plan_hash(plan)? != plan.plan_hash
    {
        return Err(invalid("saved plan identity, version, or hash mismatch"));
    }
    Ok(())
}
fn validate_locations(
    layout: &MigrationLayout,
    roots: &MigrationRoots,
) -> LegacyMigrationResult<()> {
    if let Some(saved) =
        layout.read_json::<MigrationRoots>(&layout.run_root().join("locations.json"))?
    {
        if saved != *roots {
            return Err(invalid(
                "restore the original task locations before resuming",
            ));
        }
    }
    Ok(())
}
fn invalid(message: &str) -> LegacyMigrationError {
    LegacyMigrationError::InvalidPlan(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn roots(root: &Path) -> MigrationRoots {
        MigrationRoots {
            legacy_user_root: root.join("legacy/user"),
            legacy_home_root: root.join("legacy/home"),
            legacy_skills_root: root.join("legacy/skills"),
            legacy_ssh_root: root.join("legacy/ssh"),
            target_user_root: root.join("target/user"),
            target_home_root: root.join("target/home"),
            target_skills_root: root.join("target/skills"),
            target_ssh_root: root.join("target/ssh"),
        }
    }

    fn plan() -> MigrationPlan {
        let mut plan = MigrationPlan {
            format_version: CURRENT_MIGRATION_FORMAT_VERSION,
            run_id: uuid::Uuid::new_v4().to_string(),
            source_fingerprint: "sha256:archived-source".into(),
            ..MigrationPlan::default()
        };
        plan.plan_hash = compute_plan_hash(&plan).unwrap();
        plan
    }

    #[test]
    fn independent_task_reopens_with_report_and_journal_without_a_launch_request() {
        let temp = tempfile::tempdir().unwrap();
        let roots = roots(temp.path());
        let plan = plan();
        save_task(&roots, &plan).unwrap();
        let layout = MigrationLayout::new(&roots, &plan.run_id);
        let journal = b"{\"event\":\"committed\"}\n";
        fs::write(layout.journal_path(), journal).unwrap();
        let report = MigrationRunReport {
            format_version: CURRENT_MIGRATION_FORMAT_VERSION,
            run_id: plan.run_id.clone(),
            plan_hash: plan.plan_hash.clone(),
            source_fingerprint: plan.source_fingerprint.clone(),
            status: MigrationRunStatus::FailedRecoverable,
            ..MigrationRunReport::default()
        };
        atomic_write_json(&layout.report_path(), &report).unwrap();
        assert!(!layout.request_path().exists());
        assert_eq!(
            load_task(&roots, &plan.run_id).unwrap(),
            (plan.clone(), Some(report))
        );
        let tasks = list_tasks(&roots).unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].readable);
        assert_eq!(tasks[0].status, MigrationRunStatus::FailedRecoverable);
        save_task(&roots, &plan).unwrap();
        assert_eq!(fs::read(layout.journal_path()).unwrap(), journal);
    }

    #[test]
    fn legacy_plan_without_locations_ignores_expired_handoff_and_keeps_it_intact() {
        let temp = tempfile::tempdir().unwrap();
        let roots = roots(temp.path());
        let plan = plan();
        let layout = MigrationLayout::new(&roots, &plan.run_id);
        // Old releases persisted plan.json without per-task locations metadata.
        atomic_write_json(&layout.plan_path(), &plan).unwrap();
        let old_request = br#"{"expiresAtMs":1,"requestVersion":1}"#;
        fs::write(layout.request_path(), old_request).unwrap();
        assert_eq!(
            load_task(&roots, &plan.run_id).unwrap(),
            (plan.clone(), None)
        );
        assert!(!layout.run_root().join("locations.json").exists());
        assert_eq!(fs::read(layout.request_path()).unwrap(), old_request);
    }

    #[test]
    fn changed_locations_and_source_cannot_replace_a_saved_task() {
        let temp = tempfile::tempdir().unwrap();
        let mut roots = roots(temp.path());
        let plan = plan();
        save_task(&roots, &plan).unwrap();
        let layout = MigrationLayout::new(&roots, &plan.run_id);
        let metadata = fs::read(layout.run_root().join("locations.json")).unwrap();
        let original_plan = fs::read(layout.plan_path()).unwrap();
        roots.legacy_home_root = temp.path().join("other-source");
        assert!(load_task(&roots, &plan.run_id).is_err());
        assert!(save_task(&roots, &plan).is_err());
        let mut changed = plan.clone();
        changed.source_fingerprint = "sha256:another-source".into();
        changed.plan_hash = compute_plan_hash(&changed).unwrap();
        assert!(save_task(&roots, &changed).is_err());
        assert_eq!(
            fs::read(layout.run_root().join("locations.json")).unwrap(),
            metadata
        );
        assert_eq!(fs::read(layout.plan_path()).unwrap(), original_plan);
    }

    #[test]
    fn unknown_and_corrupt_records_are_listed_as_unreadable_and_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let roots = roots(temp.path());
        for corruption in [
            "json",
            "version",
            "hash",
            "report-identity",
            "report-version",
        ] {
            let plan = plan();
            save_task(&roots, &plan).unwrap();
            let layout = MigrationLayout::new(&roots, &plan.run_id);
            let path = if corruption.starts_with("report") {
                let mut report = MigrationRunReport {
                    format_version: CURRENT_MIGRATION_FORMAT_VERSION,
                    run_id: plan.run_id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                    source_fingerprint: plan.source_fingerprint.clone(),
                    ..MigrationRunReport::default()
                };
                if corruption == "report-version" {
                    report.format_version += 1;
                } else {
                    report.source_fingerprint = "other-source".into();
                }
                atomic_write_json(&layout.report_path(), &report).unwrap();
                layout.report_path()
            } else {
                let mut value = serde_json::to_value(&plan).unwrap();
                if corruption == "version" {
                    value["formatVersion"] = serde_json::json!(999);
                }
                if corruption == "hash" {
                    value["planHash"] = serde_json::json!("changed");
                }
                atomic_write_json(&layout.plan_path(), &value).unwrap();
                if corruption == "json" {
                    fs::write(layout.plan_path(), b"{broken").unwrap();
                }
                layout.plan_path()
            };
            let bytes = fs::read(&path).unwrap();
            assert!(load_task(&roots, &plan.run_id).is_err(), "{corruption}");
            assert!(
                !list_tasks(&roots)
                    .unwrap()
                    .iter()
                    .find(|task| task.run_id == plan.run_id)
                    .unwrap()
                    .readable
            );
            assert_eq!(fs::read(path).unwrap(), bytes, "{corruption}");
        }
        assert!(load_task(&roots, "../outside").is_err());
    }
}
