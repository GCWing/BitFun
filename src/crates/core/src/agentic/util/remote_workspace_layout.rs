//! Build a text layout snapshot for remote SSH workspaces (system prompt).

use crate::service::remote_ssh::{RemoteFileService, RemoteTreeNode};
use tokio::time::{timeout, Duration};

fn append_tree_lines(
    node: &RemoteTreeNode,
    prefix: &str,
    count: &mut usize,
    max_lines: usize,
    out: &mut Vec<String>,
) -> bool {
    let Some(children) = &node.children else {
        return false;
    };
    let n = children.len();
    let mut hit = false;
    for (i, child) in children.iter().enumerate() {
        if *count >= max_lines {
            return true;
        }
        *count += 1;
        let is_last = i == n - 1;
        let conn = if is_last { "└── " } else { "├── " };
        let name = if child.is_dir {
            format!("{}/", child.name.trim_end_matches('/'))
        } else {
            child.name.clone()
        };
        out.push(format!("{}{}{}", prefix, conn, name));
        let ext = if is_last { "    " } else { "│   " };
        if append_tree_lines(child, &format!("{}{}", prefix, ext), count, max_lines, out) {
            hit = true;
        }
    }
    hit
}

/// Single SFTP `read_dir` at workspace root, formatted as a shallow tree (no subtree walk).
pub async fn build_remote_workspace_layout_preview(
    file_service: &RemoteFileService,
    connection_id: &str,
    root: &str,
    max_lines: usize,
) -> Result<(bool, String), String> {
    const LAYOUT_PREVIEW_TIMEOUT: Duration = Duration::from_secs(15);
    let tree = timeout(
        LAYOUT_PREVIEW_TIMEOUT,
        file_service.build_shallow_tree_for_layout_preview(connection_id, root),
    )
    .await
    .map_err(|_| "remote layout preview timed out".to_string())?
    .map_err(|e| e.to_string())?;

    let root_line = root.trim_end_matches('/').to_string();
    let mut lines = vec![root_line.clone()];
    let mut count = lines.len();
    let mut hit = count >= max_lines;
    if !hit {
        hit = append_tree_lines(&tree, "", &mut count, max_lines, &mut lines);
    }
    if hit && count >= max_lines {
        lines.push("... (truncated)".to_string());
    }
    Ok((hit, lines.join("\n")))
}
