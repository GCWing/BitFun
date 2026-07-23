use crate::util::errors::*;
use bitfun_services_core::workspace_instructions::WorkspaceInstructionFile;
use std::path::Path;

pub(crate) async fn build_workspace_instruction_files_context(
    workspace_root: &Path,
) -> BitFunResult<Option<String>> {
    let instruction_files =
        bitfun_services_core::workspace_instructions::read_workspace_instruction_files(
            workspace_root,
        )
        .await
        .map_err(BitFunError::service)?;
    Ok(render_workspace_instruction_files_section(
        &instruction_files,
    ))
}

fn render_workspace_instruction_files_section(
    files: &[WorkspaceInstructionFile],
) -> Option<String> {
    if files.is_empty() {
        return None;
    }

    let mut rendered =
        String::from("## Codebase and user instructions\n\nBe sure to adhere to these instructions. IMPORTANT: These instructions OVERRIDE any default behavior and you MUST follow them exactly as written.\n");

    for file in files {
        rendered.push_str(&format!(
            "<document name=\"{}\">\n{}\n</document>\n\n",
            file.name,
            file.content.trim()
        ));
    }

    Some(rendered.trim_end().to_string())
}
