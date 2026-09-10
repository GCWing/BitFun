//! Target-owned file previews. Never resolve a target path on the controller.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

const MAX_PREVIEW_BYTES: u64 = 4 * 1024 * 1024;

pub(super) fn read_workspace_file(workspace: &Path, requested: &str) -> Result<(PathBuf, String)> {
    let root = workspace
        .canonicalize()
        .context("Resolve dispatch workspace")?;
    let requested = Path::new(requested);
    let path = if requested.is_absolute() {
        requested.to_owned()
    } else {
        root.join(requested)
    };
    let path = path.canonicalize().context("Resolve dispatch file")?;
    if !path.starts_with(&root) {
        bail!("Dispatch file preview is limited to this job's workspace");
    }
    let file = std::fs::File::open(&path).context("Open dispatch file")?;
    let metadata = file.metadata().context("Inspect dispatch file")?;
    if !metadata.is_file() {
        bail!("Dispatch file preview requires a regular file");
    }
    if metadata.len() > MAX_PREVIEW_BYTES {
        bail!("Dispatch file preview supports text files up to 4 MiB; sync changes to open larger files");
    }
    let mut bytes = Vec::new();
    file.take(MAX_PREVIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("Read dispatch file")?;
    if bytes.len() as u64 > MAX_PREVIEW_BYTES {
        bail!("Dispatch file grew beyond the 4 MiB preview limit; sync changes to open it");
    }
    if bytes.contains(&0) {
        bail!("Dispatch file preview supports UTF-8 text files; sync changes to open binary files");
    }
    let content = String::from_utf8(bytes).context(
        "Dispatch file preview supports UTF-8 text files; sync changes to open this file",
    )?;
    Ok((path, content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::protocol::DispatchQueryRequest;

    #[test]
    fn legacy_usage_query_round_trips_without_new_fields() {
        let old = serde_json::json!({"jobId": "job-1", "kind": "usageReport"});
        let request: DispatchQueryRequest = serde_json::from_value(old.clone()).unwrap();
        assert!(request.file_path.is_none());
        assert_eq!(serde_json::to_value(request).unwrap(), old);
    }

    #[test]
    fn reads_current_target_file_with_relative_and_absolute_paths() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("result.txt");
        std::fs::write(&file, "first").unwrap();
        assert_eq!(
            read_workspace_file(temp.path(), "result.txt").unwrap().1,
            "first"
        );
        std::fs::write(&file, "latest").unwrap();
        assert_eq!(
            read_workspace_file(temp.path(), file.to_str().unwrap())
                .unwrap()
                .1,
            "latest"
        );
    }

    #[test]
    fn rejects_escape_directories_binary_and_oversize_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(temp.path().join("outside.txt"), "private").unwrap();
        assert!(read_workspace_file(&root, "../outside.txt")
            .unwrap_err()
            .to_string()
            .contains("limited"));
        assert!(read_workspace_file(&root, ".").is_err());
        std::fs::write(root.join("binary"), [0, 255]).unwrap();
        assert!(read_workspace_file(&root, "binary").is_err());
        std::fs::File::create(root.join("large"))
            .unwrap()
            .set_len(MAX_PREVIEW_BYTES + 1)
            .unwrap();
        assert!(read_workspace_file(&root, "large")
            .unwrap_err()
            .to_string()
            .contains("4 MiB"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(temp.path().join("outside.txt"), root.join("link")).unwrap();
            assert!(read_workspace_file(&root, "link")
                .unwrap_err()
                .to_string()
                .contains("limited"));
        }
    }
}
