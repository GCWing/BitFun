//! Session-scoped Review target evidence parsing and validation.
//!
//! This module owns platform-neutral workspace and Git-range target facts.
//! Concrete Git access remains in services.

use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

const TARGET_FILE_LIMIT: usize = 500;
const TARGET_LIMITATION_LIMIT: usize = 32;
const TARGET_STRING_LIMIT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewTargetEvidenceSource {
    Workspace,
    GitRange,
    PullRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewTargetEvidenceCompleteness {
    Complete,
    Partial,
    Unknown,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewTargetWorkspaceBinding {
    MatchingClean,
    MatchingDirty,
    Mismatched,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTargetEvidenceFile {
    path: String,
    previous_path: Option<String>,
    status: String,
    completeness: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTargetPullRequestIdentity {
    remote_id: String,
    platform: String,
    host: String,
    project_path: String,
    pull_request_id: String,
    number: u64,
    web_url: String,
}

impl ReviewTargetPullRequestIdentity {
    pub fn remote_id(&self) -> &str {
        &self.remote_id
    }

    pub fn pull_request_id(&self) -> &str {
        &self.pull_request_id
    }

    pub fn platform(&self) -> &str {
        &self.platform
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn project_path(&self) -> &str {
        &self.project_path
    }

    pub fn number(&self) -> u64 {
        self.number
    }

    pub fn web_url(&self) -> &str {
        &self.web_url
    }
}

impl ReviewTargetEvidenceFile {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn previous_path(&self) -> Option<&str> {
        self.previous_path.as_deref()
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn completeness(&self) -> &str {
        &self.completeness
    }

    fn matches_path(&self, path: &str) -> bool {
        let path = normalize_path(path);
        self.path == path || self.previous_path.as_deref() == Some(path.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTargetEvidence {
    fingerprint: String,
    source: ReviewTargetEvidenceSource,
    base_revision: Option<String>,
    head_revision: Option<String>,
    completeness: ReviewTargetEvidenceCompleteness,
    workspace_binding: ReviewTargetWorkspaceBinding,
    pull_request: Option<ReviewTargetPullRequestIdentity>,
    files: Vec<ReviewTargetEvidenceFile>,
    limitations: Vec<String>,
    omitted_file_count: usize,
}

impl ReviewTargetEvidence {
    pub fn from_context_value(
        raw: &Value,
    ) -> Result<Option<Self>, ReviewTargetEvidenceValidationError> {
        if let Some(serialized) = raw.as_str() {
            let manifest = serde_json::from_str::<Value>(serialized).map_err(|_| {
                ReviewTargetEvidenceValidationError::invalid(
                    "deep_review_run_manifest",
                    "expected valid JSON",
                )
            })?;
            return Self::from_manifest(&manifest);
        }
        Self::from_manifest(raw)
    }

    pub fn from_manifest(raw: &Value) -> Result<Option<Self>, ReviewTargetEvidenceValidationError> {
        let evidence = raw
            .get("evidencePack")
            .or_else(|| raw.get("evidence_pack"))
            .and_then(|pack| {
                pack.get("reviewTarget")
                    .or_else(|| pack.get("review_target"))
            })
            .or_else(|| raw.get("reviewTargetEvidence"))
            .or_else(|| raw.get("review_target_evidence"));
        let Some(evidence) = evidence else {
            return Ok(None);
        };
        let object = evidence.as_object().ok_or_else(|| {
            ReviewTargetEvidenceValidationError::invalid("reviewTarget", "expected object")
        })?;

        let version = required_u64(evidence, &["version"], "reviewTarget.version")?;
        if version != 1 {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.version",
                "expected 1",
            ));
        }
        let source = match required_string(evidence, &["source"], "reviewTarget.source")?.as_str() {
            "workspace" => ReviewTargetEvidenceSource::Workspace,
            "git_range" => ReviewTargetEvidenceSource::GitRange,
            "pull_request" => ReviewTargetEvidenceSource::PullRequest,
            _ => {
                return Err(ReviewTargetEvidenceValidationError::invalid(
                    "reviewTarget.source",
                    "unknown source",
                ))
            }
        };
        let fingerprint = required_string(evidence, &["fingerprint"], "reviewTarget.fingerprint")?;
        let base_revision = optional_string(
            evidence,
            &["baseRevision", "base_revision"],
            "reviewTarget.baseRevision",
        )?;
        let head_revision = optional_string(
            evidence,
            &["headRevision", "head_revision"],
            "reviewTarget.headRevision",
        )?;
        let completeness =
            match required_string(evidence, &["completeness"], "reviewTarget.completeness")?
                .as_str()
            {
                "complete" => ReviewTargetEvidenceCompleteness::Complete,
                "partial" => ReviewTargetEvidenceCompleteness::Partial,
                "unknown" => ReviewTargetEvidenceCompleteness::Unknown,
                "stale" => ReviewTargetEvidenceCompleteness::Stale,
                _ => {
                    return Err(ReviewTargetEvidenceValidationError::invalid(
                        "reviewTarget.completeness",
                        "unknown completeness",
                    ))
                }
            };
        let workspace_binding = match required_string(
            evidence,
            &["workspaceBinding", "workspace_binding"],
            "reviewTarget.workspaceBinding",
        )?
        .as_str()
        {
            "matching_clean" => ReviewTargetWorkspaceBinding::MatchingClean,
            "matching_dirty" => ReviewTargetWorkspaceBinding::MatchingDirty,
            "mismatched" => ReviewTargetWorkspaceBinding::Mismatched,
            "unavailable" => ReviewTargetWorkspaceBinding::Unavailable,
            _ => {
                return Err(ReviewTargetEvidenceValidationError::invalid(
                    "reviewTarget.workspaceBinding",
                    "unknown workspace binding",
                ))
            }
        };
        let pull_request = evidence
            .get("pullRequest")
            .or_else(|| evidence.get("pull_request"))
            .map(parse_pull_request_identity)
            .transpose()?;
        if source == ReviewTargetEvidenceSource::PullRequest && pull_request.is_none() {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.pullRequest",
                "pull request targets require provider identity",
            ));
        }
        if source != ReviewTargetEvidenceSource::PullRequest && pull_request.is_some() {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.pullRequest",
                "provider identity is only valid for pull request targets",
            ));
        }

        let file_values = required_array(
            evidence,
            &["files"],
            "reviewTarget.files",
            TARGET_FILE_LIMIT,
        )?;
        let mut files = Vec::with_capacity(file_values.len());
        for file in file_values {
            let path = required_path_string(file, &["path"], "reviewTarget.files[].path")?;
            ensure_relative_target_path(&path, "reviewTarget.files[].path")?;
            let previous_path = optional_path_string(
                file,
                &["previousPath", "previous_path"],
                "reviewTarget.files[].previousPath",
            )?;
            if let Some(previous_path) = previous_path.as_deref() {
                ensure_relative_target_path(previous_path, "reviewTarget.files[].previousPath")?;
            }
            let status = required_string(file, &["status"], "reviewTarget.files[].status")?;
            if !matches!(
                status.as_str(),
                "added" | "modified" | "deleted" | "renamed" | "copied" | "unknown"
            ) {
                return Err(ReviewTargetEvidenceValidationError::invalid(
                    "reviewTarget.files[].status",
                    "unknown file status",
                ));
            }
            let completeness_value =
                required_string(file, &["completeness"], "reviewTarget.files[].completeness")?;
            if !matches!(
                completeness_value.as_str(),
                "complete" | "partial" | "unavailable"
            ) {
                return Err(ReviewTargetEvidenceValidationError::invalid(
                    "reviewTarget.files[].completeness",
                    "unknown file completeness",
                ));
            }
            files.push(ReviewTargetEvidenceFile {
                path: normalize_path(&path),
                previous_path: previous_path.map(|path| normalize_path(&path)),
                status,
                completeness: completeness_value,
            });
        }
        let limitations = required_string_array(
            evidence,
            &["limitations"],
            "reviewTarget.limitations",
            TARGET_LIMITATION_LIMIT,
        )?;
        let omitted_file_count = optional_u64(
            evidence,
            &["omittedFileCount", "omitted_file_count"],
            "reviewTarget.omittedFileCount",
        )?
        .unwrap_or(0) as usize;

        if completeness == ReviewTargetEvidenceCompleteness::Complete
            && source != ReviewTargetEvidenceSource::Workspace
            && (base_revision.is_none() || head_revision.is_none())
        {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.completeness",
                "complete immutable targets require base and head revisions",
            ));
        }
        if completeness == ReviewTargetEvidenceCompleteness::Complete
            && source != ReviewTargetEvidenceSource::Workspace
            && (!base_revision.as_deref().is_some_and(is_full_commit_id)
                || !head_revision.as_deref().is_some_and(is_full_commit_id))
        {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.completeness",
                "complete immutable targets require full commit ids",
            ));
        }
        if completeness == ReviewTargetEvidenceCompleteness::Complete
            && (omitted_file_count > 0 || files.iter().any(|file| file.completeness != "complete"))
        {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.completeness",
                "complete target contains omitted or incomplete files",
            ));
        }
        if workspace_binding == ReviewTargetWorkspaceBinding::MatchingClean
            && (source != ReviewTargetEvidenceSource::GitRange
                || !base_revision.as_deref().is_some_and(is_full_commit_id)
                || !head_revision.as_deref().is_some_and(is_full_commit_id))
        {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.workspaceBinding",
                "matching_clean requires immutable Git range revisions",
            ));
        }
        let _ = object;

        Ok(Some(Self {
            fingerprint,
            source,
            base_revision,
            head_revision,
            completeness,
            workspace_binding,
            pull_request,
            files,
            limitations,
            omitted_file_count,
        }))
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn source(&self) -> ReviewTargetEvidenceSource {
        self.source
    }

    pub fn base_revision(&self) -> Option<&str> {
        self.base_revision.as_deref()
    }

    pub fn head_revision(&self) -> Option<&str> {
        self.head_revision.as_deref()
    }

    pub fn completeness(&self) -> ReviewTargetEvidenceCompleteness {
        self.completeness
    }

    pub fn workspace_binding(&self) -> ReviewTargetWorkspaceBinding {
        self.workspace_binding
    }

    pub fn pull_request(&self) -> Option<&ReviewTargetPullRequestIdentity> {
        self.pull_request.as_ref()
    }

    pub fn files(&self) -> &[ReviewTargetEvidenceFile] {
        &self.files
    }

    pub fn limitations(&self) -> &[String] {
        &self.limitations
    }

    pub fn omitted_file_count(&self) -> usize {
        self.omitted_file_count
    }

    pub fn contains_file(&self, path: &str) -> bool {
        self.files.iter().any(|file| file.matches_path(path))
    }

    pub fn file_status_for_path(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|file| file.matches_path(path))
            .map(ReviewTargetEvidenceFile::status)
    }

    pub fn file_completeness_for_path(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|file| file.matches_path(path))
            .map(ReviewTargetEvidenceFile::completeness)
    }

    pub fn file_page_hint_for_path(&self, path: &str, page_size: usize) -> Option<u32> {
        if page_size == 0 {
            return None;
        }
        let index = self.files.iter().position(|file| file.matches_path(path))?;
        u32::try_from(index / page_size + 1).ok()
    }

    pub fn diff_revisions_for_path(&self, path: &str) -> Option<(&str, &str)> {
        if !self.contains_file(path) {
            return None;
        }
        if self.source != ReviewTargetEvidenceSource::GitRange {
            return None;
        }
        Some((
            self.base_revision.as_deref()?,
            self.head_revision.as_deref()?,
        ))
    }

    pub fn diff_paths_for_path(&self, path: &str) -> Vec<String> {
        let Some(file) = self.files.iter().find(|file| file.matches_path(path)) else {
            return Vec::new();
        };
        let mut paths = Vec::with_capacity(2);
        if let Some(previous_path) = file.previous_path() {
            paths.push(previous_path.to_string());
        }
        if !paths.iter().any(|candidate| candidate == file.path()) {
            paths.push(file.path().to_string());
        }
        paths
    }

    pub fn allows_live_repository_context(&self) -> bool {
        self.source == ReviewTargetEvidenceSource::GitRange
            && self.workspace_binding == ReviewTargetWorkspaceBinding::MatchingClean
            && self.base_revision.as_deref().is_some_and(is_full_commit_id)
            && self.head_revision.as_deref().is_some_and(is_full_commit_id)
    }

    /// Cross-checks the evidence against the manifest scope before any
    /// reviewer can consume target-bound paths. This intentionally validates
    /// only the file boundary: broader manifest policy belongs to the normal
    /// Deep Review gate.
    pub fn validate_manifest_scope(
        &self,
        raw: &Value,
    ) -> Result<(), ReviewTargetEvidenceValidationError> {
        let target_files = raw
            .get("target")
            .and_then(|target| target.get("files"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ReviewTargetEvidenceValidationError::invalid(
                    "target.files",
                    "expected target files when Review target evidence is present",
                )
            })?;

        let mut target_pairs = HashSet::new();
        let mut included_target_pairs = HashSet::new();
        let mut included_paths = HashSet::new();
        for file in target_files {
            let path = required_path_string(
                file,
                &["normalizedPath", "normalized_path", "path"],
                "target.files[].normalizedPath",
            )?;
            ensure_relative_target_path(&path, "target.files[].normalizedPath")?;
            let previous_path = optional_path_string(
                file,
                &[
                    "normalizedOldPath",
                    "normalized_old_path",
                    "oldPath",
                    "old_path",
                ],
                "target.files[].normalizedOldPath",
            )?;
            if let Some(previous_path) = previous_path.as_deref() {
                ensure_relative_target_path(previous_path, "target.files[].normalizedOldPath")?;
            }
            let path = normalize_path(&path);
            let previous_path = previous_path.map(|path| normalize_path(&path));
            let excluded = file
                .get("excluded")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            target_pairs.insert((path.clone(), previous_path.clone()));
            if !excluded {
                included_target_pairs.insert((path.clone(), previous_path));
                included_paths.insert(path);
            }
        }

        if self
            .files
            .iter()
            .any(|file| !target_pairs.contains(&(file.path.clone(), file.previous_path.clone())))
        {
            return Err(ReviewTargetEvidenceValidationError::invalid(
                "reviewTarget.files",
                "evidence path or previous path is outside the classified target",
            ));
        }

        if self.completeness == ReviewTargetEvidenceCompleteness::Complete {
            let evidence_pairs = self
                .files
                .iter()
                .map(|file| (file.path.clone(), file.previous_path.clone()))
                .collect::<HashSet<_>>();
            if evidence_pairs != included_target_pairs {
                return Err(ReviewTargetEvidenceValidationError::invalid(
                    "reviewTarget.files",
                    "complete evidence must cover every included target file",
                ));
            }
        }

        let Some(work_packets) = raw
            .get("workPackets")
            .or_else(|| raw.get("work_packets"))
            .and_then(Value::as_array)
        else {
            return Ok(());
        };
        for packet in work_packets {
            let Some(packet_files) = packet
                .get("assignedScope")
                .or_else(|| packet.get("assigned_scope"))
                .and_then(|scope| scope.get("files"))
                .and_then(Value::as_array)
            else {
                continue;
            };
            for file in packet_files {
                let path = file.as_str().ok_or_else(|| {
                    ReviewTargetEvidenceValidationError::invalid(
                        "workPackets[].assignedScope.files",
                        "expected path strings",
                    )
                })?;
                ensure_relative_target_path(path, "workPackets[].assignedScope.files")?;
                if !included_paths.contains(&normalize_path(path)) {
                    return Err(ReviewTargetEvidenceValidationError::invalid(
                        "workPackets[].assignedScope.files",
                        "packet path is outside the included classified target",
                    ));
                }
                if !self.contains_file(path) {
                    return Err(ReviewTargetEvidenceValidationError::invalid(
                        "workPackets[].assignedScope.files",
                        "packet path is missing from Review target evidence",
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTargetEvidenceValidationError {
    detail: String,
}

impl ReviewTargetEvidenceValidationError {
    fn invalid(field: &'static str, reason: &'static str) -> Self {
        Self {
            detail: format!(
                "invalid Review target evidence field '{}': {}",
                field, reason
            ),
        }
    }
}

impl fmt::Display for ReviewTargetEvidenceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

fn parse_pull_request_identity(
    value: &Value,
) -> Result<ReviewTargetPullRequestIdentity, ReviewTargetEvidenceValidationError> {
    if !value.is_object() {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            "reviewTarget.pullRequest",
            "expected object",
        ));
    }
    let platform = required_string(value, &["platform"], "reviewTarget.pullRequest.platform")?;
    if !matches!(platform.as_str(), "github" | "gitlab" | "gitcode") {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            "reviewTarget.pullRequest.platform",
            "unknown provider",
        ));
    }
    Ok(ReviewTargetPullRequestIdentity {
        remote_id: required_string(
            value,
            &["remoteId", "remote_id"],
            "reviewTarget.pullRequest.remoteId",
        )?,
        platform,
        host: required_string(value, &["host"], "reviewTarget.pullRequest.host")?,
        project_path: required_string(
            value,
            &["projectPath", "project_path"],
            "reviewTarget.pullRequest.projectPath",
        )?,
        pull_request_id: required_string(
            value,
            &["pullRequestId", "pull_request_id"],
            "reviewTarget.pullRequest.pullRequestId",
        )?,
        number: required_u64(value, &["number"], "reviewTarget.pullRequest.number")?,
        web_url: required_string(
            value,
            &["webUrl", "web_url"],
            "reviewTarget.pullRequest.webUrl",
        )?,
    })
}

fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn required_string(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<String, ReviewTargetEvidenceValidationError> {
    optional_string(value, keys, field)?.ok_or_else(|| {
        ReviewTargetEvidenceValidationError::invalid(field, "expected non-empty string")
    })
}

fn optional_string(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<Option<String>, ReviewTargetEvidenceValidationError> {
    let Some(value) = value_for_any_key(value, keys) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ReviewTargetEvidenceValidationError::invalid(field, "expected string"))?;
    if value.len() > TARGET_STRING_LIMIT {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            field,
            "string exceeds supported length",
        ));
    }
    Ok(Some(value.to_string()))
}

fn required_path_string(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<String, ReviewTargetEvidenceValidationError> {
    optional_path_string(value, keys, field)?.ok_or_else(|| {
        ReviewTargetEvidenceValidationError::invalid(field, "expected non-empty path string")
    })
}

fn optional_path_string(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<Option<String>, ReviewTargetEvidenceValidationError> {
    let Some(value) = value_for_any_key(value, keys) else {
        return Ok(None);
    };
    let value = value.as_str().ok_or_else(|| {
        ReviewTargetEvidenceValidationError::invalid(field, "expected path string")
    })?;
    if value.is_empty() {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            field,
            "expected non-empty path string",
        ));
    }
    if value.len() > TARGET_STRING_LIMIT {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            field,
            "path exceeds supported length",
        ));
    }
    if value.contains('\0') {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            field,
            "path contains NUL",
        ));
    }
    Ok(Some(value.to_string()))
}

fn required_u64(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<u64, ReviewTargetEvidenceValidationError> {
    optional_u64(value, keys, field)?.ok_or_else(|| {
        ReviewTargetEvidenceValidationError::invalid(field, "expected unsigned integer")
    })
}

fn optional_u64(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<Option<u64>, ReviewTargetEvidenceValidationError> {
    let Some(value) = value_for_any_key(value, keys) else {
        return Ok(None);
    };
    value.as_u64().map(Some).ok_or_else(|| {
        ReviewTargetEvidenceValidationError::invalid(field, "expected unsigned integer")
    })
}

fn required_array<'a>(
    value: &'a Value,
    keys: &[&str],
    field: &'static str,
    limit: usize,
) -> Result<&'a Vec<Value>, ReviewTargetEvidenceValidationError> {
    let array = value_for_any_key(value, keys)
        .and_then(Value::as_array)
        .ok_or_else(|| ReviewTargetEvidenceValidationError::invalid(field, "expected array"))?;
    if array.len() > limit {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            field,
            "array exceeds supported length",
        ));
    }
    Ok(array)
}

fn required_string_array(
    value: &Value,
    keys: &[&str],
    field: &'static str,
    limit: usize,
) -> Result<Vec<String>, ReviewTargetEvidenceValidationError> {
    required_array(value, keys, field, limit)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|item| !item.is_empty() && item.len() <= TARGET_STRING_LIMIT)
                .map(str::to_string)
                .ok_or_else(|| {
                    ReviewTargetEvidenceValidationError::invalid(
                        field,
                        "expected bounded non-empty string items",
                    )
                })
        })
        .collect()
}

fn normalize_path(path: &str) -> String {
    let path = if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.to_string()
    };
    path.trim_start_matches("./").to_string()
}

fn is_full_commit_id(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ensure_relative_target_path(
    path: &str,
    field: &'static str,
) -> Result<(), ReviewTargetEvidenceValidationError> {
    let path = normalize_path(path);
    let has_drive_prefix = path.len() >= 2 && path.as_bytes()[1] == b':';
    if path.is_empty()
        || path.contains('\0')
        || path.starts_with('/')
        || has_drive_prefix
        || path.split('/').any(|segment| segment == "..")
    {
        return Err(ReviewTargetEvidenceValidationError::invalid(
            field,
            "expected normalized workspace-relative path",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest() -> Value {
        json!({
            "reviewMode": "deep",
            "evidencePack": {
                "reviewTarget": {
                    "version": 1,
                    "source": "git_range",
                    "fingerprint": "abc12345",
                    "baseRevision": "1111111111111111111111111111111111111111",
                    "headRevision": "2222222222222222222222222222222222222222",
                    "completeness": "complete",
                    "workspaceBinding": "matching_clean",
                    "files": [{
                        "path": "src/lib.rs",
                        "status": "modified",
                        "diffRef": "git-range:abc:1",
                        "completeness": "complete"
                    }],
                    "diffRefs": ["git-range:abc:1"],
                    "limitations": []
                }
            }
        })
    }

    fn scoped_manifest() -> Value {
        let mut value = manifest();
        value["target"] = json!({
            "files": [{
                "path": "src/lib.rs",
                "normalizedPath": "src/lib.rs",
                "status": "modified",
                "excluded": false
            }]
        });
        value["workPackets"] = json!([{
            "assignedScope": { "files": ["src/lib.rs"] }
        }]);
        value
    }

    #[test]
    fn parses_complete_git_range_target() {
        let evidence = ReviewTargetEvidence::from_manifest(&manifest())
            .expect("target evidence should validate")
            .expect("target evidence should exist");
        assert!(evidence.allows_live_repository_context());
        assert_eq!(
            evidence.diff_revisions_for_path("src/lib.rs"),
            Some((
                "1111111111111111111111111111111111111111",
                "2222222222222222222222222222222222222222"
            ))
        );
    }

    #[test]
    fn parses_pull_request_identity_without_enabling_live_repository_context() {
        let mut value = manifest();
        value["evidencePack"]["reviewTarget"]["source"] = json!("pull_request");
        value["evidencePack"]["reviewTarget"]["workspaceBinding"] = json!("unavailable");
        value["evidencePack"]["reviewTarget"]["pullRequest"] = json!({
            "remoteId": "origin|https://github.com/example/repo.git",
            "platform": "github",
            "host": "github.com",
            "projectPath": "example/repo",
            "pullRequestId": "42",
            "number": 42,
            "webUrl": "https://github.com/example/repo/pull/42"
        });

        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("pull request evidence should validate")
            .expect("pull request evidence should exist");
        let identity = evidence
            .pull_request()
            .expect("provider identity should be retained");
        assert_eq!(identity.pull_request_id(), "42");
        assert_eq!(identity.project_path(), "example/repo");
        assert!(!evidence.allows_live_repository_context());
        assert_eq!(evidence.diff_revisions_for_path("src/lib.rs"), None);
    }

    #[test]
    fn validates_evidence_and_packet_paths_against_the_target() {
        let value = scoped_manifest();
        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("target evidence should validate")
            .expect("target evidence should exist");

        evidence
            .validate_manifest_scope(&value)
            .expect("aligned target scope should validate");
    }

    #[test]
    fn rejects_rename_previous_path_outside_the_target() {
        let mut value = scoped_manifest();
        value["evidencePack"]["reviewTarget"]["files"][0]["previousPath"] = json!("secret/old.rs");
        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("evidence shape should parse")
            .expect("target evidence should exist");

        let error = evidence
            .validate_manifest_scope(&value)
            .expect_err("an injected previous path must fail closed");
        assert!(error.to_string().contains("outside the classified target"));
    }

    #[test]
    fn rejects_work_packet_path_outside_the_included_target() {
        let mut value = scoped_manifest();
        value["workPackets"][0]["assignedScope"]["files"] = json!(["secret.rs"]);
        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("evidence shape should parse")
            .expect("target evidence should exist");

        let error = evidence
            .validate_manifest_scope(&value)
            .expect_err("an out-of-target work packet must fail closed");
        assert!(error
            .to_string()
            .contains("outside the included classified target"));
    }

    #[test]
    fn rejects_complete_evidence_that_omits_an_included_target_file() {
        let mut value = scoped_manifest();
        value["target"]["files"] = json!([
            {
                "path": "src/lib.rs",
                "normalizedPath": "src/lib.rs",
                "status": "modified",
                "excluded": false
            },
            {
                "path": "src/other.rs",
                "normalizedPath": "src/other.rs",
                "status": "modified",
                "excluded": false
            }
        ]);
        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("evidence shape should parse")
            .expect("target evidence should exist");

        let error = evidence
            .validate_manifest_scope(&value)
            .expect_err("complete evidence cannot omit a target file");
        assert!(error
            .to_string()
            .contains("cover every included target file"));
    }

    #[test]
    fn rejects_a_work_packet_file_missing_from_partial_evidence() {
        let mut value = scoped_manifest();
        value["evidencePack"]["reviewTarget"]["completeness"] = json!("partial");
        value["target"]["files"] = json!([
            {
                "path": "src/lib.rs",
                "normalizedPath": "src/lib.rs",
                "status": "modified",
                "excluded": false
            },
            {
                "path": "src/other.rs",
                "normalizedPath": "src/other.rs",
                "status": "modified",
                "excluded": false
            }
        ]);
        value["workPackets"][0]["assignedScope"]["files"] = json!(["src/other.rs"]);
        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("evidence shape should parse")
            .expect("target evidence should exist");

        let error = evidence
            .validate_manifest_scope(&value)
            .expect_err("a packet cannot request missing evidence");
        assert!(error
            .to_string()
            .contains("missing from Review target evidence"));
    }

    #[test]
    fn accepts_matching_clean_with_partial_evidence_and_keeps_live_context() {
        let mut value = manifest();
        value["evidencePack"]["reviewTarget"]["completeness"] = json!("partial");
        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("partial target should preserve a valid workspace binding")
            .expect("target evidence should exist");
        assert!(evidence.allows_live_repository_context());
        assert!(evidence.diff_revisions_for_path("src/lib.rs").is_some());
    }

    #[test]
    fn rejects_parent_path_escape() {
        let mut value = manifest();
        value["evidencePack"]["reviewTarget"]["files"][0]["path"] = json!("../secret.txt");
        let error = ReviewTargetEvidence::from_manifest(&value)
            .expect_err("target path escape must be rejected");
        assert!(error.to_string().contains("workspace-relative"));
    }

    #[test]
    fn rejects_nul_in_target_path() {
        let mut value = manifest();
        value["evidencePack"]["reviewTarget"]["files"][0]["path"] = json!("src/nul\0path.rs");
        let error = ReviewTargetEvidence::from_manifest(&value)
            .expect_err("NUL target paths must be rejected before process invocation");
        assert!(error.to_string().contains("path contains NUL"));
    }

    #[test]
    fn preserves_legal_leading_and_trailing_path_whitespace() {
        let mut value = manifest();
        value["evidencePack"]["reviewTarget"]["files"][0]["path"] = json!(" leading.rs ");
        let evidence = ReviewTargetEvidence::from_manifest(&value)
            .expect("whitespace is legal in Git paths")
            .expect("target evidence should exist");

        assert!(evidence.contains_file(" leading.rs "));
        assert!(!evidence.contains_file("leading.rs"));
    }
}
