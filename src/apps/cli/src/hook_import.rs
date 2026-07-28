use anyhow::{anyhow, Result};
use bitfun_product_domains::external_hook_import::{
    ExternalHookImportApplyRequestV1, ExternalHookImportDependencyV1,
    ExternalHookImportMutationRequestV1, ExternalHookImportMutationV1, ExternalHookImportPlanV1,
    ExternalHookImportSnapshotV1, EXTERNAL_HOOK_IMPORT_SCHEMA_V1,
};
use bitfun_product_domains::external_sources::{
    ExternalSourceOperationError, ExternalSourceScope, SourceKey,
};
use clap::{Subcommand, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum HookImportOutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum HookImportResetScope {
    User,
    Project,
}

#[derive(Subcommand)]
pub(crate) enum HookAction {
    /// List available and imported Hook sources
    List {
        /// Re-read source files and check imported sources for updates
        #[arg(long)]
        refresh: bool,
        /// Output format for automation
        #[arg(long, value_enum, default_value_t = HookImportOutputFormat::Text)]
        format: HookImportOutputFormat,
    },
    /// Preview or confirm importing one Hook source
    Import {
        /// Stable source key shown by `bitfun hooks list`
        #[arg(long)]
        source: String,
        /// Confirm the exact plan fingerprint shown by the preview
        #[arg(long, value_name = "PLAN_FINGERPRINT")]
        confirm: Option<String>,
        /// Output format for automation
        #[arg(long, value_enum, default_value_t = HookImportOutputFormat::Text)]
        format: HookImportOutputFormat,
    },
    /// Preview or confirm updating one imported Hook source
    Update {
        import_id: String,
        /// Confirm the exact plan fingerprint shown by the preview
        #[arg(long, value_name = "PLAN_FINGERPRINT")]
        confirm: Option<String>,
        /// Output format for automation
        #[arg(long, value_enum, default_value_t = HookImportOutputFormat::Text)]
        format: HookImportOutputFormat,
    },
    /// Enable one imported Hook source
    Enable { import_id: String },
    /// Disable one imported Hook source without deleting it
    Disable { import_id: String },
    /// Remove only BitFun's managed copy of one imported Hook source
    Remove {
        import_id: String,
        /// Confirm removal of the BitFun-managed copy
        #[arg(long, required = true)]
        confirm: bool,
    },
    /// Reset a corrupt BitFun-managed Hook index without changing source files
    Reset {
        #[arg(value_enum)]
        scope: HookImportResetScope,
        /// Confirm deletion of the corrupt managed index
        #[arg(long, required = true)]
        confirm: bool,
    },
}

pub(crate) async fn run(action: Option<HookAction>) -> Result<()> {
    let workspace = std::env::current_dir().ok().map(PathBuf::from);
    match action.unwrap_or(HookAction::List {
        refresh: false,
        format: HookImportOutputFormat::Text,
    }) {
        HookAction::List { refresh, format } => {
            let snapshot = bitfun_core::external_hook_import::external_hook_import_snapshot(
                workspace.as_deref(),
                refresh,
            )
            .await
            .map_err(operation_error)?;
            print_value(format, &snapshot, render_snapshot(&snapshot))
        }
        HookAction::Import {
            source,
            confirm,
            format,
        } => {
            let source = SourceKey::from_stable_key(&source)
                .ok_or_else(|| anyhow!("Invalid Hook source key: {}", escape(&source)))?;
            preview_or_apply(workspace.as_deref(), source, confirm, format).await
        }
        HookAction::Update {
            import_id,
            confirm,
            format,
        } => {
            let snapshot = bitfun_core::external_hook_import::external_hook_import_snapshot(
                workspace.as_deref(),
                false,
            )
            .await
            .map_err(operation_error)?;
            let source = snapshot
                .imports
                .iter()
                .find(|item| item.import_id == import_id)
                .map(|item| item.source.key.clone())
                .ok_or_else(|| anyhow!("Hook import not found: {}", escape(&import_id)))?;
            preview_or_apply(workspace.as_deref(), source, confirm, format).await
        }
        HookAction::Enable { import_id } => {
            mutate(
                workspace.as_deref(),
                ExternalHookImportMutationV1::SetEnabled {
                    import_id: import_id.clone(),
                    enabled: true,
                },
            )
            .await
            .map_err(operation_error)?;
            println!("Enabled imported Hooks: {}", escape(&import_id));
            Ok(())
        }
        HookAction::Disable { import_id } => {
            mutate(
                workspace.as_deref(),
                ExternalHookImportMutationV1::SetEnabled {
                    import_id: import_id.clone(),
                    enabled: false,
                },
            )
            .await
            .map_err(operation_error)?;
            println!("Disabled imported Hooks: {}", escape(&import_id));
            Ok(())
        }
        HookAction::Remove { import_id, .. } => {
            mutate(
                workspace.as_deref(),
                ExternalHookImportMutationV1::Remove {
                    import_id: import_id.clone(),
                },
            )
            .await
            .map_err(operation_error)?;
            println!(
                "Removed BitFun's managed Hook copy; the source was not changed: {}",
                escape(&import_id)
            );
            Ok(())
        }
        HookAction::Reset { scope, .. } => {
            let scope = match scope {
                HookImportResetScope::User => ExternalSourceScope::UserGlobal,
                HookImportResetScope::Project => ExternalSourceScope::Project,
            };
            mutate(
                workspace.as_deref(),
                ExternalHookImportMutationV1::ResetCorruptStore { scope },
            )
            .await
            .map_err(operation_error)?;
            println!(
                "Reset the corrupt {:?} managed Hook index; source files were not changed.",
                scope
            );
            Ok(())
        }
    }
}

async fn preview_or_apply(
    workspace: Option<&std::path::Path>,
    source: SourceKey,
    confirm: Option<String>,
    format: HookImportOutputFormat,
) -> Result<()> {
    let plan =
        bitfun_core::external_hook_import::plan_external_hook_import(workspace, source.clone())
            .await
            .map_err(operation_error)?;
    let Some(plan_fingerprint) = confirm else {
        return print_value(format, &plan, render_plan(&plan));
    };
    let result = bitfun_core::external_hook_import::apply_external_hook_import(
        workspace,
        ExternalHookImportApplyRequestV1 {
            schema_version: EXTERNAL_HOOK_IMPORT_SCHEMA_V1,
            source,
            plan_fingerprint,
        },
    )
    .await
    .map_err(operation_error)?;
    let text = match &result.outcome {
        bitfun_product_domains::external_hook_import::ExternalHookImportApplyOutcomeV1::Applied {
            ..
        } => "Imported Hooks are enabled and will apply on the next matching event.".to_string(),
        bitfun_product_domains::external_hook_import::ExternalHookImportApplyOutcomeV1::Unchanged {
            ..
        } => "The reviewed Hook import is already current and enabled.".to_string(),
        bitfun_product_domains::external_hook_import::ExternalHookImportApplyOutcomeV1::Stale {
            refreshed_plan,
        } => format!(
            "The Hook source changed; nothing was written. Review the refreshed plan:\n{}",
            render_plan(refreshed_plan)
        ),
    };
    print_value(format, &result, text)
}

pub(crate) async fn mutate(
    workspace: Option<&std::path::Path>,
    action: ExternalHookImportMutationV1,
) -> std::result::Result<ExternalHookImportSnapshotV1, ExternalSourceOperationError> {
    let snapshot =
        bitfun_core::external_hook_import::external_hook_import_snapshot(workspace, false).await?;
    bitfun_core::external_hook_import::mutate_external_hook_import(
        workspace,
        ExternalHookImportMutationRequestV1 {
            schema_version: EXTERNAL_HOOK_IMPORT_SCHEMA_V1,
            expected_revision: snapshot.revision,
            action,
        },
    )
    .await
}

fn render_snapshot(snapshot: &ExternalHookImportSnapshotV1) -> String {
    let mut lines = vec![format!(
        "Available Hook sources ({}):",
        snapshot.catalog.sources.len()
    )];
    for source in &snapshot.catalog.sources {
        lines.push(format!(
            "- {} [{}] ({:?}, {:?})",
            escape(&source.display_name),
            escape(&source.key.stable_key()),
            source.scope,
            source.health
        ));
    }
    lines.push(format!(
        "Imported Hook sources ({}):",
        snapshot.imports.len()
    ));
    for imported in &snapshot.imports {
        lines.push(format!(
            "- {} [{}] enabled={} state={:?}",
            escape(&imported.source.display_name),
            escape(&imported.import_id),
            imported.enabled,
            imported.state
        ));
    }
    for diagnostic in &snapshot.diagnostics {
        lines.push(format!(
            "- diagnostic {}: {}",
            escape(&diagnostic.code),
            escape(&diagnostic.message)
        ));
    }
    lines.join("\n")
}

pub(crate) fn render_plan(plan: &ExternalHookImportPlanV1) -> String {
    let mut lines = vec![format!(
        "Hook import {:?}: {} handler(s) from {}",
        plan.disposition,
        plan.handlers.len(),
        escape(&plan.source.display_name)
    )];
    for handler in &plan.handlers {
        lines.push(format!(
            "- {} event={} matcher={} timeout={}s",
            escape(&handler.stable_key),
            escape(&handler.event),
            handler
                .matcher
                .as_deref()
                .map(escape)
                .unwrap_or_else(|| "*".to_string()),
            handler.timeout_seconds.unwrap_or(60)
        ));
        lines.push(format!("  command: {}", escape(&handler.command)));
        if let Some(command) = &handler.command_windows {
            lines.push(format!("  commandWindows: {}", escape(command)));
        }
        for dependency in &handler.dependencies {
            let (kind, value) = match dependency {
                ExternalHookImportDependencyV1::Managed { relative_path } => {
                    ("managed", relative_path)
                }
                ExternalHookImportDependencyV1::External { location } => ("external", location),
            };
            lines.push(format!("  dependency ({kind}): {}", escape(value)));
        }
    }
    for skipped in &plan.skipped {
        lines.push(format!(
            "- skipped {}: {}",
            escape(&skipped.reason_code),
            skipped.count
        ));
    }
    lines.push(format!(
        "Plan fingerprint: {}",
        escape(&plan.plan_fingerprint)
    ));
    lines.push("Preview only until this exact fingerprint is passed with --confirm.".to_string());
    lines.join("\n")
}

fn print_value(format: HookImportOutputFormat, value: &impl Serialize, text: String) -> Result<()> {
    match format {
        HookImportOutputFormat::Text => println!("{text}"),
        HookImportOutputFormat::Json => println!("{}", serde_json::to_string(value)?),
    }
    Ok(())
}

fn operation_error(error: ExternalSourceOperationError) -> anyhow::Error {
    anyhow!("{}: {}", error.code.as_str(), escape(&error.detail))
}

fn escape(value: &str) -> String {
    crate::plugin_diagnostics::escape_terminal_text(value)
}
