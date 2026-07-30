use anyhow::{anyhow, Result};
use bitfun_product_domains::external_sources::{
    ExternalMcpImportApplyOutcomeV1, ExternalMcpImportApplyRequestV1,
    ExternalMcpImportDispositionV1, ExternalMcpImportPlanV1, ExternalMcpImportSelectionV1,
    EXTERNAL_MCP_IMPORT_SCHEMA_V1,
};
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum McpImportOutputFormat {
    Text,
    Json,
}

pub(crate) struct McpImportCommand {
    pub apply: bool,
    pub candidates: Vec<String>,
    pub native_id: Option<String>,
    pub format: McpImportOutputFormat,
}

pub(crate) async fn execute(command: McpImportCommand) -> Result<()> {
    let workspace = std::env::current_dir().ok();
    let plan = bitfun_core::external_mcp_import::plan_external_mcp_import(workspace.clone())
        .await
        .map_err(operation_error)?;
    if !command.apply {
        return print_value(command.format, &plan, render_plan(&plan));
    }
    let selections = selections(&plan, &command.candidates, command.native_id)?;
    if selections.is_empty() {
        return Err(anyhow!(
            "No eligible external MCP servers are available to import"
        ));
    }
    let result = bitfun_core::external_mcp_import::apply_external_mcp_import(
        workspace,
        ExternalMcpImportApplyRequestV1 {
            schema_version: EXTERNAL_MCP_IMPORT_SCHEMA_V1,
            plan_fingerprint: plan.plan_fingerprint.clone(),
            selections,
        },
    )
    .await
    .map_err(operation_error)?;
    let text = match &result.outcome {
        ExternalMcpImportApplyOutcomeV1::Applied { imported } => format!(
            "Imported {} MCP server(s) as disabled native entries. Enable them from MCP settings when ready.",
            imported.len()
        ),
        ExternalMcpImportApplyOutcomeV1::Stale { .. } => {
            "The import plan changed. Review the refreshed plan and run --apply again.".to_string()
        }
    };
    print_value(command.format, &result, text)
}

fn selections(
    plan: &ExternalMcpImportPlanV1,
    requested: &[String],
    native_id: Option<String>,
) -> Result<Vec<ExternalMcpImportSelectionV1>> {
    if native_id.is_some() && requested.len() != 1 {
        return Err(anyhow!("--native-id requires exactly one --candidate"));
    }
    let eligible =
        |item: &&bitfun_product_domains::external_sources::ExternalMcpImportPlanItemV1| {
            matches!(
                item.disposition,
                ExternalMcpImportDispositionV1::Eligible
                    | ExternalMcpImportDispositionV1::AutomaticRename
            )
        };
    if requested.is_empty() {
        return Ok(plan
            .items
            .iter()
            .filter(eligible)
            .map(|item| ExternalMcpImportSelectionV1 {
                candidate_id: item.candidate_id.clone(),
                requested_native_id: None,
            })
            .collect());
    }
    requested
        .iter()
        .map(|candidate_id| {
            let item = plan
                .items
                .iter()
                .find(|item| &item.candidate_id == candidate_id)
                .filter(eligible)
                .ok_or_else(|| {
                    anyhow!("Candidate is not eligible in the current plan: {candidate_id}")
                })?;
            Ok(ExternalMcpImportSelectionV1 {
                candidate_id: item.candidate_id.clone(),
                requested_native_id: native_id.clone(),
            })
        })
        .collect()
}

fn render_plan(plan: &ExternalMcpImportPlanV1) -> String {
    let eligible = plan
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.disposition,
                ExternalMcpImportDispositionV1::Eligible
                    | ExternalMcpImportDispositionV1::AutomaticRename
            )
        })
        .collect::<Vec<_>>();
    let mut lines = vec![format!(
        "{} external MCP server(s) can be imported:",
        eligible.len()
    )];
    for item in eligible {
        lines.push(format!(
            "- {} -> {}",
            crate::plugin_diagnostics::escape_terminal_text(&item.display_name),
            crate::plugin_diagnostics::escape_terminal_text(
                item.proposed_native_id.as_deref().unwrap_or("unavailable")
            )
        ));
    }
    lines.push("Preview only. Use --apply to write disabled native entries.".to_string());
    lines.join("\n")
}

fn print_value(
    value_format: McpImportOutputFormat,
    value: &impl serde::Serialize,
    text: String,
) -> Result<()> {
    match value_format {
        McpImportOutputFormat::Text => println!("{text}"),
        McpImportOutputFormat::Json => println!("{}", serde_json::to_string(value)?),
    }
    Ok(())
}

fn operation_error(
    error: bitfun_product_domains::external_sources::ExternalSourceOperationError,
) -> anyhow::Error {
    anyhow!("{}: {}", error.code.as_str(), error.detail)
}
