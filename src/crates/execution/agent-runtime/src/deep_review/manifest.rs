//! Typed Deep Review launch manifest accessors.
//!
//! The frontend builds the launch manifest, but Rust owns defensive parsing and
//! the final trust boundary. Accessors in this module must remain backward
//! compatible with older manifest field spellings and should not silently hide
//! reduced coverage, omitted files, or stale evidence hints.

use super::constants::{
    MANAGED_REVIEW_MAX_BATCHES, MANAGED_REVIEW_MAX_FILES_PER_BATCH,
    MANAGED_REVIEW_MAX_PARALLEL_INSTANCES, MANAGED_REVIEW_MAX_WORKER_TIMEOUT_SECONDS,
    REVIEWER_GENERAL_AGENT_TYPE,
};
use super::execution_policy::DeepReviewPolicyViolation;
use super::target_evidence::ReviewTargetEvidence;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeepReviewScopeProfile {
    review_depth: String,
    risk_focus_tags: Vec<String>,
    max_dependency_hops: Option<String>,
    optional_reviewer_policy: Option<String>,
    allow_broad_tool_exploration: bool,
    coverage_expectation: Option<String>,
}

impl DeepReviewScopeProfile {
    pub(crate) fn from_manifest(raw: &Value) -> Option<Self> {
        let manifest = raw.as_object()?;
        let review_mode = string_for_any_key(raw, &["reviewMode", "review_mode"])?;
        if review_mode != "deep" {
            return None;
        }

        let profile = manifest
            .get("scopeProfile")
            .or_else(|| manifest.get("scope_profile"))?
            .as_object()?;
        let review_depth = profile
            .get("reviewDepth")
            .or_else(|| profile.get("review_depth"))
            .and_then(normalized_non_empty_string)?;
        if !matches!(
            review_depth.as_str(),
            "high_risk_only" | "risk_expanded" | "full_depth"
        ) {
            return None;
        }

        let risk_focus_tags = profile
            .get("riskFocusTags")
            .or_else(|| profile.get("risk_focus_tags"))
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| tag.as_str().map(str::trim))
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(Self {
            review_depth,
            risk_focus_tags,
            max_dependency_hops: profile
                .get("maxDependencyHops")
                .or_else(|| profile.get("max_dependency_hops"))
                .and_then(scope_dependency_hops_to_string),
            optional_reviewer_policy: profile
                .get("optionalReviewerPolicy")
                .or_else(|| profile.get("optional_reviewer_policy"))
                .and_then(normalized_non_empty_string),
            allow_broad_tool_exploration: profile
                .get("allowBroadToolExploration")
                .or_else(|| profile.get("allow_broad_tool_exploration"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            coverage_expectation: profile
                .get("coverageExpectation")
                .or_else(|| profile.get("coverage_expectation"))
                .and_then(normalized_non_empty_string),
        })
    }

    pub(crate) fn coverage_expectation(&self) -> Option<&str> {
        self.coverage_expectation.as_deref()
    }

    pub(crate) fn is_reduced_depth(&self) -> bool {
        self.review_depth != "full_depth"
    }
}

#[cfg(test)]
impl DeepReviewScopeProfile {
    pub(crate) fn review_depth(&self) -> &str {
        &self.review_depth
    }

    pub(crate) fn risk_focus_tags(&self) -> &[String] {
        &self.risk_focus_tags
    }

    pub(crate) fn max_dependency_hops(&self) -> Option<&str> {
        self.max_dependency_hops.as_deref()
    }

    pub(crate) fn optional_reviewer_policy(&self) -> Option<&str> {
        self.optional_reviewer_policy.as_deref()
    }

    pub(crate) fn allow_broad_tool_exploration(&self) -> bool {
        self.allow_broad_tool_exploration
    }
}

fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn normalized_non_empty_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_for_any_key(value: &Value, keys: &[&str]) -> Option<String> {
    value_for_any_key(value, keys).and_then(normalized_non_empty_string)
}

fn scope_dependency_hops_to_string(value: &Value) -> Option<String> {
    if let Some(hops) = value.as_u64() {
        return Some(hops.to_string());
    }
    normalized_non_empty_string(value)
}

const EVIDENCE_PACK_CHANGED_FILE_LIMIT: usize = 80;
const EVIDENCE_PACK_HUNK_HINT_LIMIT: usize = 80;
const EVIDENCE_PACK_CONTRACT_HINT_LIMIT: usize = 40;
const EVIDENCE_PACK_PACKET_ID_LIMIT: usize = 256;
const EVIDENCE_PACK_TAG_LIMIT: usize = 32;
const EVIDENCE_PACK_PRIVACY_EXCLUDES: &[&str] = &[
    "source_text",
    "full_diff",
    "model_output",
    "provider_raw_body",
    "full_file_contents",
];
const EVIDENCE_PACK_FORBIDDEN_KEYS: &[&str] = &[
    "sourceText",
    "source_text",
    "fullDiff",
    "full_diff",
    "modelOutput",
    "model_output",
    "providerRawBody",
    "provider_raw_body",
    "fullFileContents",
    "full_file_contents",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeepReviewEvidencePack {
    version: u64,
    source: String,
    changed_files: Vec<String>,
    packet_ids: Vec<String>,
    hunk_hint_count: usize,
    contract_hint_count: usize,
    content_boundary: String,
}

impl DeepReviewEvidencePack {
    pub(crate) fn from_manifest(
        raw: &Value,
    ) -> Result<Option<Self>, DeepReviewEvidencePackValidationError> {
        if string_for_any_key(raw, &["reviewMode", "review_mode"]).as_deref() != Some("deep") {
            return Ok(None);
        }

        let Some(pack) = value_for_any_key(raw, &["evidencePack", "evidence_pack"]) else {
            return Ok(None);
        };
        ensure_object(pack, "evidencePack")?;
        if let Some(key) = forbidden_evidence_pack_key(pack) {
            return Err(DeepReviewEvidencePackValidationError::new(format!(
                "forbidden evidence pack field '{}'",
                key
            )));
        }

        let version = required_u64_for_any_key(pack, &["version"], "version")?;
        if version != 1 {
            return Err(DeepReviewEvidencePackValidationError::invalid_field(
                "version",
                "expected 1",
            ));
        }

        let source = required_string_for_any_key(pack, &["source"], "source")?;
        if source != "target_manifest" {
            return Err(DeepReviewEvidencePackValidationError::invalid_field(
                "source",
                "expected target_manifest",
            ));
        }

        let changed_files = required_string_array_for_any_key(
            pack,
            &["changedFiles", "changed_files"],
            "changedFiles",
            EVIDENCE_PACK_CHANGED_FILE_LIMIT,
        )?;
        let domain_tags = required_string_array_for_any_key(
            pack,
            &["domainTags", "domain_tags"],
            "domainTags",
            EVIDENCE_PACK_TAG_LIMIT,
        )?;
        let risk_focus_tags = required_string_array_for_any_key(
            pack,
            &["riskFocusTags", "risk_focus_tags"],
            "riskFocusTags",
            EVIDENCE_PACK_TAG_LIMIT,
        )?;
        let packet_ids = required_string_array_for_any_key(
            pack,
            &["packetIds", "packet_ids"],
            "packetIds",
            EVIDENCE_PACK_PACKET_ID_LIMIT,
        )?;

        let diff_stat = required_value_for_any_key(pack, &["diffStat", "diff_stat"], "diffStat")?;
        ensure_object(diff_stat, "diffStat")?;
        required_u64_for_any_key(
            diff_stat,
            &["fileCount", "file_count"],
            "diffStat.fileCount",
        )?;
        required_string_for_any_key(
            diff_stat,
            &["lineCountSource", "line_count_source"],
            "diffStat.lineCountSource",
        )?;

        let hunk_hints = required_array_for_any_key(
            pack,
            &["hunkHints", "hunk_hints"],
            "hunkHints",
            EVIDENCE_PACK_HUNK_HINT_LIMIT,
        )?;
        for hint in hunk_hints {
            ensure_object(hint, "hunkHints[]")?;
            required_string_for_any_key(hint, &["filePath", "file_path"], "hunkHints[].filePath")?;
            required_u64_for_any_key(
                hint,
                &["changedLineCount", "changed_line_count"],
                "hunkHints[].changedLineCount",
            )?;
            required_string_for_any_key(
                hint,
                &["lineCountSource", "line_count_source"],
                "hunkHints[].lineCountSource",
            )?;
        }

        let contract_hints = required_array_for_any_key(
            pack,
            &["contractHints", "contract_hints"],
            "contractHints",
            EVIDENCE_PACK_CONTRACT_HINT_LIMIT,
        )?;
        for hint in contract_hints {
            ensure_object(hint, "contractHints[]")?;
            let kind = required_string_for_any_key(hint, &["kind"], "contractHints[].kind")?;
            if !matches!(
                kind.as_str(),
                "i18n_key" | "tauri_command" | "api_contract" | "config_key"
            ) {
                return Err(DeepReviewEvidencePackValidationError::invalid_field(
                    "contractHints[].kind",
                    "unknown contract hint kind",
                ));
            }
            required_string_for_any_key(
                hint,
                &["filePath", "file_path"],
                "contractHints[].filePath",
            )?;
            let hint_source =
                required_string_for_any_key(hint, &["source"], "contractHints[].source")?;
            if hint_source != "path_classifier" {
                return Err(DeepReviewEvidencePackValidationError::invalid_field(
                    "contractHints[].source",
                    "expected path_classifier",
                ));
            }
        }

        validate_evidence_pack_budget(pack)?;
        let content_boundary = validate_evidence_pack_privacy(pack)?;

        let _ = (domain_tags, risk_focus_tags);

        Ok(Some(Self {
            version,
            source,
            changed_files,
            packet_ids,
            hunk_hint_count: hunk_hints.len(),
            contract_hint_count: contract_hints.len(),
            content_boundary,
        }))
    }
}

#[cfg(test)]
impl DeepReviewEvidencePack {
    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn changed_files(&self) -> &[String] {
        &self.changed_files
    }

    pub(crate) fn packet_ids(&self) -> &[String] {
        &self.packet_ids
    }

    pub(crate) fn hunk_hint_count(&self) -> usize {
        self.hunk_hint_count
    }

    pub(crate) fn contract_hint_count(&self) -> usize {
        self.contract_hint_count
    }

    pub(crate) fn content_boundary(&self) -> &str {
        &self.content_boundary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeepReviewEvidencePackValidationError {
    detail: String,
}

impl DeepReviewEvidencePackValidationError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    fn missing_field(field: &'static str) -> Self {
        Self::new(format!("missing evidence pack field '{}'", field))
    }

    fn invalid_field(field: &'static str, reason: &'static str) -> Self {
        Self::new(format!(
            "invalid evidence pack field '{}': {}",
            field, reason
        ))
    }

    fn too_many_items(field: &'static str, max: usize, actual: usize) -> Self {
        Self::new(format!(
            "too many evidence pack items in '{}': max {}, got {}",
            field, max, actual
        ))
    }
}

impl fmt::Display for DeepReviewEvidencePackValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

fn ensure_object(
    value: &Value,
    field: &'static str,
) -> Result<(), DeepReviewEvidencePackValidationError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(DeepReviewEvidencePackValidationError::invalid_field(
            field,
            "expected object",
        ))
    }
}

fn required_value_for_any_key<'a>(
    value: &'a Value,
    keys: &[&str],
    field: &'static str,
) -> Result<&'a Value, DeepReviewEvidencePackValidationError> {
    value_for_any_key(value, keys)
        .ok_or_else(|| DeepReviewEvidencePackValidationError::missing_field(field))
}

fn required_string_for_any_key(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<String, DeepReviewEvidencePackValidationError> {
    string_for_any_key(value, keys).ok_or_else(|| {
        DeepReviewEvidencePackValidationError::invalid_field(field, "expected non-empty string")
    })
}

fn required_u64_for_any_key(
    value: &Value,
    keys: &[&str],
    field: &'static str,
) -> Result<u64, DeepReviewEvidencePackValidationError> {
    required_value_for_any_key(value, keys, field)?
        .as_u64()
        .ok_or_else(|| {
            DeepReviewEvidencePackValidationError::invalid_field(field, "expected unsigned integer")
        })
}

fn required_array_for_any_key<'a>(
    value: &'a Value,
    keys: &[&str],
    field: &'static str,
    max: usize,
) -> Result<&'a Vec<Value>, DeepReviewEvidencePackValidationError> {
    let array = required_value_for_any_key(value, keys, field)?
        .as_array()
        .ok_or_else(|| {
            DeepReviewEvidencePackValidationError::invalid_field(field, "expected array")
        })?;
    if array.len() > max {
        return Err(DeepReviewEvidencePackValidationError::too_many_items(
            field,
            max,
            array.len(),
        ));
    }
    Ok(array)
}

fn required_string_array_for_any_key(
    value: &Value,
    keys: &[&str],
    field: &'static str,
    max: usize,
) -> Result<Vec<String>, DeepReviewEvidencePackValidationError> {
    required_array_for_any_key(value, keys, field, max)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    DeepReviewEvidencePackValidationError::invalid_field(
                        field,
                        "expected non-empty string items",
                    )
                })
        })
        .collect()
}

fn validate_evidence_pack_budget(
    pack: &Value,
) -> Result<(), DeepReviewEvidencePackValidationError> {
    let budget = required_value_for_any_key(pack, &["budget"], "budget")?;
    ensure_object(budget, "budget")?;
    validate_budget_cap(
        budget,
        &["maxChangedFiles", "max_changed_files"],
        "budget.maxChangedFiles",
        EVIDENCE_PACK_CHANGED_FILE_LIMIT,
    )?;
    validate_budget_cap(
        budget,
        &["maxHunkHints", "max_hunk_hints"],
        "budget.maxHunkHints",
        EVIDENCE_PACK_HUNK_HINT_LIMIT,
    )?;
    validate_budget_cap(
        budget,
        &["maxContractHints", "max_contract_hints"],
        "budget.maxContractHints",
        EVIDENCE_PACK_CONTRACT_HINT_LIMIT,
    )?;
    required_u64_for_any_key(
        budget,
        &["omittedChangedFileCount", "omitted_changed_file_count"],
        "budget.omittedChangedFileCount",
    )?;
    required_u64_for_any_key(
        budget,
        &["omittedHunkHintCount", "omitted_hunk_hint_count"],
        "budget.omittedHunkHintCount",
    )?;
    required_u64_for_any_key(
        budget,
        &["omittedContractHintCount", "omitted_contract_hint_count"],
        "budget.omittedContractHintCount",
    )?;
    Ok(())
}

fn validate_budget_cap(
    budget: &Value,
    keys: &[&str],
    field: &'static str,
    max: usize,
) -> Result<(), DeepReviewEvidencePackValidationError> {
    let cap = required_u64_for_any_key(budget, keys, field)?;
    if cap as usize > max {
        return Err(DeepReviewEvidencePackValidationError::invalid_field(
            field,
            "exceeds supported manifest cap",
        ));
    }
    Ok(())
}

fn validate_evidence_pack_privacy(
    pack: &Value,
) -> Result<String, DeepReviewEvidencePackValidationError> {
    let privacy = required_value_for_any_key(pack, &["privacy"], "privacy")?;
    ensure_object(privacy, "privacy")?;
    let content = required_string_for_any_key(privacy, &["content"], "privacy.content")?;
    if content != "metadata_only" {
        return Err(DeepReviewEvidencePackValidationError::invalid_field(
            "privacy.content",
            "expected metadata_only",
        ));
    }
    let excludes = required_string_array_for_any_key(
        privacy,
        &["excludes"],
        "privacy.excludes",
        EVIDENCE_PACK_PRIVACY_EXCLUDES.len(),
    )?;
    let excludes = excludes.into_iter().collect::<HashSet<_>>();
    for required in EVIDENCE_PACK_PRIVACY_EXCLUDES {
        if !excludes.contains(*required) {
            return Err(DeepReviewEvidencePackValidationError::invalid_field(
                "privacy.excludes",
                "missing required excluded content type",
            ));
        }
    }
    Ok(content)
}

fn forbidden_evidence_pack_key(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if EVIDENCE_PACK_FORBIDDEN_KEYS.contains(&key.as_str()) {
                    return Some(key.clone());
                }
                if let Some(nested) = forbidden_evidence_pack_key(child) {
                    return Some(nested);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(forbidden_evidence_pack_key),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewRunManifestGate {
    active_subagent_ids: HashSet<String>,
    skipped_subagent_reasons: HashMap<String, String>,
    manifest_policy_error: Option<String>,
    manifest_policy_error_code: &'static str,
}

impl DeepReviewRunManifestGate {
    pub fn from_value(raw: &Value) -> Option<Self> {
        let manifest = raw.as_object()?;
        if manifest.get("reviewMode").and_then(Value::as_str) != Some("deep") {
            return None;
        }

        let mut active_subagent_ids = HashSet::new();
        collect_manifest_members(manifest.get("workPackets"), &mut active_subagent_ids);
        collect_manifest_members(manifest.get("coreReviewers"), &mut active_subagent_ids);
        collect_manifest_members(
            manifest.get("enabledExtraReviewers"),
            &mut active_subagent_ids,
        );
        if let Some(id) = manifest
            .get("qualityGateReviewer")
            .and_then(manifest_member_subagent_id)
        {
            active_subagent_ids.insert(id);
        }

        let mut skipped_subagent_reasons = HashMap::new();
        if let Some(skipped) = manifest.get("skippedReviewers").and_then(Value::as_array) {
            for member in skipped {
                let Some(id) = manifest_member_subagent_id(member) else {
                    continue;
                };
                let reason = member
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("skipped")
                    .trim();
                skipped_subagent_reasons.insert(
                    id,
                    if reason.is_empty() {
                        "skipped".to_string()
                    } else {
                        reason.to_string()
                    },
                );
            }
        }
        let target_evidence_error = match ReviewTargetEvidence::from_manifest(raw) {
            Ok(Some(evidence)) => evidence
                .validate_manifest_scope(raw)
                .err()
                .map(|error| error.to_string()),
            Ok(None) => None,
            Err(error) => Some(error.to_string()),
        };
        let quality_decision_error = validate_quality_decision(manifest, &active_subagent_ids);
        let managed_plan_error = validate_managed_review_plan(manifest);
        let (manifest_policy_error_code, manifest_policy_error) =
            if let Some(error) = target_evidence_error {
                ("deep_review_target_evidence_invalid", Some(error))
            } else if let Some(error) = managed_plan_error {
                ("deep_review_managed_plan_invalid", Some(error))
            } else {
                (
                    "deep_review_manifest_quality_decision_mismatch",
                    quality_decision_error,
                )
            };

        if active_subagent_ids.is_empty() && manifest_policy_error.is_none() {
            return None;
        }

        Some(Self {
            active_subagent_ids,
            skipped_subagent_reasons,
            manifest_policy_error,
            manifest_policy_error_code,
        })
    }

    pub fn ensure_active(&self, subagent_type: &str) -> Result<(), DeepReviewPolicyViolation> {
        if let Some(error) = self.manifest_policy_error.as_deref() {
            return Err(DeepReviewPolicyViolation::new(
                self.manifest_policy_error_code,
                error,
            ));
        }
        if self.active_subagent_ids.contains(subagent_type) {
            return Ok(());
        }

        let reason = self
            .skipped_subagent_reasons
            .get(subagent_type)
            .map(String::as_str)
            .unwrap_or("missing_from_manifest");

        Err(DeepReviewPolicyViolation::new(
            "deep_review_subagent_not_active_for_target",
            format!(
                "DeepReview subagent '{}' is not active for this review target (reason: {})",
                subagent_type, reason
            ),
        ))
    }
}

fn validate_managed_review_plan(manifest: &serde_json::Map<String, Value>) -> Option<String> {
    let packets = manifest
        .get("workPackets")
        .or_else(|| manifest.get("work_packets"))
        .and_then(Value::as_array);
    let has_general_packet = packets.is_some_and(|packets| {
        packets.iter().any(|packet| {
            manifest_member_subagent_id(packet).as_deref() == Some(REVIEWER_GENERAL_AGENT_TYPE)
        })
    });
    let Some(plan) = manifest
        .get("managedReviewPlan")
        .or_else(|| manifest.get("managed_review_plan"))
        .and_then(Value::as_object)
    else {
        return has_general_packet
            .then(|| "ReviewGeneral packets require managedReviewPlan runtime bounds".to_string());
    };

    let usize_field = |camel: &str, snake: &str| {
        plan.get(camel)
            .or_else(|| plan.get(snake))
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
    };
    let version = usize_field("version", "version");
    let total_file_count = usize_field("totalFileCount", "total_file_count");
    let planned_file_count = usize_field("plannedFileCount", "planned_file_count");
    let deferred_file_count = usize_field("deferredFileCount", "deferred_file_count");
    let max_files_per_batch = usize_field("maxFilesPerBatch", "max_files_per_batch");
    let max_batches = usize_field("maxBatches", "max_batches");
    let max_parallel_instances = usize_field("maxParallelInstances", "max_parallel_instances");
    let worker_timeout_seconds = plan
        .get("workerTimeoutSeconds")
        .or_else(|| plan.get("worker_timeout_seconds"))
        .and_then(Value::as_u64);

    if version != Some(1)
        || total_file_count.is_none()
        || planned_file_count.is_none()
        || deferred_file_count.is_none()
        || !matches!(
            max_files_per_batch,
            Some(1..=MANAGED_REVIEW_MAX_FILES_PER_BATCH)
        )
        || !matches!(max_batches, Some(1..=MANAGED_REVIEW_MAX_BATCHES))
        || !matches!(
            max_parallel_instances,
            Some(1..=MANAGED_REVIEW_MAX_PARALLEL_INSTANCES)
        )
        || !matches!(
            worker_timeout_seconds,
            Some(1..=MANAGED_REVIEW_MAX_WORKER_TIMEOUT_SECONDS)
        )
    {
        return Some("managedReviewPlan is missing required bounded fields".to_string());
    }

    let total_file_count = total_file_count.unwrap_or_default();
    let planned_file_count = planned_file_count.unwrap_or_default();
    let deferred_file_count = deferred_file_count.unwrap_or_default();
    if total_file_count != planned_file_count.saturating_add(deferred_file_count) {
        return Some("managedReviewPlan file counts are inconsistent".to_string());
    }

    let packets = packets.map(Vec::as_slice).unwrap_or_default();
    if packets.len() > max_batches.unwrap_or_default() {
        return Some("managed Review packet count exceeds maxBatches".to_string());
    }

    let mut packet_ids = HashSet::new();
    let mut assigned_files = HashSet::new();
    let mut launch_batch_counts = HashMap::<u64, usize>::new();
    let mut packet_file_count = 0usize;
    for packet in packets {
        if manifest_member_subagent_id(packet).as_deref() != Some(REVIEWER_GENERAL_AGENT_TYPE)
            || packet.get("phase").and_then(Value::as_str) != Some("reviewer")
        {
            return Some(
                "managed Review packets must use ReviewGeneral reviewer workers".to_string(),
            );
        }
        let packet_id = packet
            .get("packetId")
            .or_else(|| packet.get("packet_id"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());
        if packet_id.is_none() || !packet_ids.insert(packet_id.unwrap_or_default().to_string()) {
            return Some("managed Review packet ids must be non-empty and unique".to_string());
        }
        let files = packet
            .get("assignedScope")
            .or_else(|| packet.get("assigned_scope"))
            .and_then(|scope| scope.get("files"))
            .and_then(Value::as_array);
        let Some(files) = files else {
            return Some("managed Review packet scope is missing files".to_string());
        };
        if files.is_empty()
            || files.len() > max_files_per_batch.unwrap_or_default()
            || files.iter().any(|file| file.as_str().is_none())
        {
            return Some("managed Review packet file scope exceeds its bound".to_string());
        }
        for file in files.iter().filter_map(Value::as_str) {
            if !assigned_files.insert(file.to_string()) {
                return Some(
                    "managed Review packet scopes must not contain duplicate files".to_string(),
                );
            }
        }
        packet_file_count = packet_file_count.saturating_add(files.len());

        let timeout = packet
            .get("timeoutSeconds")
            .or_else(|| packet.get("timeout_seconds"))
            .and_then(Value::as_u64);
        if !matches!(timeout, Some(1..=MANAGED_REVIEW_MAX_WORKER_TIMEOUT_SECONDS))
            || timeout > worker_timeout_seconds
        {
            return Some("managed Review packet timeout exceeds its bound".to_string());
        }
        let launch_batch = packet
            .get("launchBatch")
            .or_else(|| packet.get("launch_batch"))
            .and_then(Value::as_u64)
            .filter(|value| *value > 0);
        let Some(launch_batch) = launch_batch else {
            return Some("managed Review packet is missing launchBatch".to_string());
        };
        let count = launch_batch_counts.entry(launch_batch).or_default();
        *count += 1;
        if *count > max_parallel_instances.unwrap_or_default() {
            return Some("managed Review launch batch exceeds maxParallelInstances".to_string());
        }
    }
    if packet_file_count != planned_file_count {
        return Some("managed Review planned file count does not match packet scopes".to_string());
    }

    let concurrency_max = manifest
        .get("concurrencyPolicy")
        .or_else(|| manifest.get("concurrency_policy"))
        .and_then(Value::as_object)
        .and_then(|policy| {
            policy
                .get("maxParallelInstances")
                .or_else(|| policy.get("max_parallel_instances"))
        })
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    if concurrency_max != max_parallel_instances {
        return Some(
            "managed Review concurrency policy must match maxParallelInstances".to_string(),
        );
    }

    let execution = manifest.get("executionPolicy").and_then(Value::as_object);
    let max_reviewer_calls = execution
        .and_then(|policy| policy.get("maxReviewerCalls"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    if max_reviewer_calls != Some(packets.len().max(1)) {
        return Some("managed Review reviewer-call ceiling must match packet count".to_string());
    }

    None
}

fn validate_quality_decision(
    manifest: &serde_json::Map<String, Value>,
    active_subagent_ids: &HashSet<String>,
) -> Option<String> {
    let decision = manifest
        .get("qualityDecision")
        .or_else(|| manifest.get("quality_decision"))?;
    let level = decision.get("level").and_then(Value::as_str).unwrap_or("");
    let strategy_level = manifest
        .get("strategyLevel")
        .or_else(|| manifest.get("strategy_level"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let quality_gate_id = manifest
        .get("qualityGateReviewer")
        .or_else(|| manifest.get("quality_gate_reviewer"))
        .and_then(manifest_member_subagent_id);

    match level {
        "l2" => {
            if strategy_level != "normal" {
                return Some("L2 Review requires the normal strategy".to_string());
            }
            if quality_gate_id.is_some() || active_subagent_ids.contains("ReviewJudge") {
                return Some("L2 Review must not launch a quality-gate reviewer".to_string());
            }
            if active_subagent_ids.len() > 3 {
                return Some(format!(
                    "L2 Review allows at most 3 active reviewers, found {}",
                    active_subagent_ids.len()
                ));
            }
        }
        "l3" => {
            if strategy_level != "deep" {
                return Some("L3 Review requires the deep strategy".to_string());
            }
            if quality_gate_id.is_some() && quality_gate_id.as_deref() != Some("ReviewJudge") {
                return Some("L3 Review quality gate must use ReviewJudge".to_string());
            }
        }
        _ => return Some(format!("Unsupported Review quality level: {level}")),
    }

    None
}

fn collect_manifest_members(raw: Option<&Value>, output: &mut HashSet<String>) {
    let Some(values) = raw.and_then(Value::as_array) else {
        return;
    };

    for member in values {
        if let Some(id) = manifest_member_subagent_id(member) {
            output.insert(id);
        }
    }
}

fn manifest_member_subagent_id(value: &Value) -> Option<String> {
    let id = value
        .get("subagentId")
        .or_else(|| value.get("subagent_id"))
        .and_then(Value::as_str)?
        .trim();
    (!id.is_empty()).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn quality_manifest(level: &str, strategy: &str, reviewers: &[&str]) -> Value {
        json!({
            "reviewMode": "deep",
            "strategyLevel": strategy,
            "qualityDecision": { "level": level },
            "coreReviewers": reviewers.iter().map(|id| json!({ "subagentId": id })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn l2_manifest_accepts_up_to_three_reviewers_without_judge() {
        let manifest = quality_manifest(
            "l2",
            "normal",
            &[
                "ReviewBusinessLogic",
                "ReviewSecurity",
                "ReviewArchitecture",
            ],
        );
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        assert!(gate.ensure_active("ReviewSecurity").is_ok());
    }

    #[test]
    fn l2_manifest_rejects_more_than_three_reviewers() {
        let manifest = quality_manifest(
            "l2",
            "normal",
            &[
                "ReviewBusinessLogic",
                "ReviewSecurity",
                "ReviewArchitecture",
                "ReviewFrontend",
            ],
        );
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        let error = gate
            .ensure_active("ReviewSecurity")
            .expect_err("L2 must stay bounded");
        assert_eq!(error.code, "deep_review_manifest_quality_decision_mismatch");
    }

    #[test]
    fn l2_manifest_rejects_a_quality_gate_reviewer() {
        let mut manifest = quality_manifest("l2", "normal", &["ReviewBusinessLogic"]);
        manifest["qualityGateReviewer"] = json!({ "subagentId": "ReviewJudge" });
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        assert!(gate.ensure_active("ReviewBusinessLogic").is_err());
    }

    #[test]
    fn l3_manifest_requires_deep_strategy_but_allows_on_demand_delegation() {
        let manifest = quality_manifest("l3", "normal", &["ReviewBusinessLogic"]);
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");
        assert!(gate.ensure_active("ReviewBusinessLogic").is_err());

        let single_specialist = quality_manifest("l3", "deep", &["ReviewSecurity"]);
        let single_specialist_gate =
            DeepReviewRunManifestGate::from_value(&single_specialist).expect("gate should parse");
        assert!(single_specialist_gate
            .ensure_active("ReviewSecurity")
            .is_ok());

        let mut wrong_judge = quality_manifest("l3", "deep", &["ReviewSecurity"]);
        wrong_judge["qualityGateReviewer"] = json!({ "subagentId": "ReviewSecurity" });
        let wrong_judge_gate =
            DeepReviewRunManifestGate::from_value(&wrong_judge).expect("gate should parse");
        assert!(wrong_judge_gate.ensure_active("ReviewSecurity").is_err());

        let mut valid = quality_manifest("l3", "deep", &["ReviewSecurity"]);
        valid["qualityGateReviewer"] = json!({ "subagentId": "ReviewJudge" });
        let valid_gate = DeepReviewRunManifestGate::from_value(&valid).expect("gate should parse");
        assert!(valid_gate.ensure_active("ReviewJudge").is_ok());
    }

    #[test]
    fn legacy_manifest_without_quality_decision_remains_compatible() {
        let manifest = json!({
            "reviewMode": "deep",
            "coreReviewers": [{ "subagentId": "ReviewSecurity" }]
        });
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        assert!(gate.ensure_active("ReviewSecurity").is_ok());
    }

    fn managed_manifest() -> Value {
        json!({
            "reviewMode": "deep",
            "managedReviewPlan": {
                "version": 1,
                "totalFileCount": 2,
                "plannedFileCount": 2,
                "deferredFileCount": 0,
                "maxFilesPerBatch": 40,
                "maxBatches": 8,
                "maxParallelInstances": 2,
                "workerTimeoutSeconds": 120
            },
            "executionPolicy": {
                "reviewerTimeoutSeconds": 120,
                "maxSameRoleInstances": 2,
                "maxReviewerCalls": 2,
                "maxRetriesPerRole": 0
            },
            "concurrencyPolicy": {
                "maxParallelInstances": 2
            },
            "workPackets": [
                {
                    "packetId": "managed-1",
                    "phase": "reviewer",
                    "launchBatch": 1,
                    "subagentId": "ReviewGeneral",
                    "timeoutSeconds": 120,
                    "assignedScope": { "files": ["src/a.rs"] }
                },
                {
                    "packetId": "managed-2",
                    "phase": "reviewer",
                    "launchBatch": 1,
                    "subagentId": "ReviewGeneral",
                    "timeoutSeconds": 120,
                    "assignedScope": { "files": ["src/b.rs"] }
                }
            ]
        })
    }

    #[test]
    fn managed_manifest_admits_only_a_bounded_declared_plan() {
        let manifest = managed_manifest();
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        assert!(gate.ensure_active("ReviewGeneral").is_ok());
    }

    #[test]
    fn review_general_requires_a_managed_plan() {
        let mut manifest = managed_manifest();
        manifest
            .as_object_mut()
            .unwrap()
            .remove("managedReviewPlan");
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        let error = gate
            .ensure_active("ReviewGeneral")
            .expect_err("ReviewGeneral must not enter a strict or legacy plan");
        assert_eq!(error.code, "deep_review_managed_plan_invalid");
    }

    #[test]
    fn managed_manifest_rejects_packet_scope_over_the_declared_bound() {
        let mut manifest = managed_manifest();
        manifest["workPackets"][0]["assignedScope"]["files"] = Value::Array(
            (0..41)
                .map(|index| json!(format!("src/{index}.rs")))
                .collect(),
        );
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        let error = gate
            .ensure_active("ReviewGeneral")
            .expect_err("managed packet must remain bounded");
        assert_eq!(error.code, "deep_review_managed_plan_invalid");
    }

    #[test]
    fn managed_manifest_rejects_duplicate_files_across_packets() {
        let mut manifest = managed_manifest();
        manifest["workPackets"][1]["assignedScope"]["files"] = json!(["src/a.rs"]);
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        let error = gate
            .ensure_active("ReviewGeneral")
            .expect_err("managed packet scopes must be disjoint");
        assert_eq!(error.code, "deep_review_managed_plan_invalid");
    }

    #[test]
    fn managed_manifest_rejects_a_wider_runtime_concurrency_policy() {
        let mut manifest = managed_manifest();
        manifest["concurrencyPolicy"]["maxParallelInstances"] = json!(4);
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("gate should parse");

        let error = gate
            .ensure_active("ReviewGeneral")
            .expect_err("runtime concurrency must match the managed plan");
        assert_eq!(error.code, "deep_review_managed_plan_invalid");
    }

    #[test]
    fn scope_profile_parses_camel_case_manifest() {
        let manifest = json!({
            "reviewMode": "deep",
            "scopeProfile": {
                "reviewDepth": "high_risk_only",
                "riskFocusTags": ["security", "cross_boundary_api_contracts"],
                "maxDependencyHops": 0,
                "optionalReviewerPolicy": "risk_matched_only",
                "allowBroadToolExploration": false,
                "coverageExpectation": "High-risk-only pass."
            }
        });

        let profile =
            DeepReviewScopeProfile::from_manifest(&manifest).expect("scope profile should parse");

        assert_eq!(profile.review_depth(), "high_risk_only");
        assert_eq!(
            profile
                .risk_focus_tags()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["security", "cross_boundary_api_contracts"]
        );
        assert_eq!(profile.max_dependency_hops(), Some("0"));
        assert_eq!(
            profile.optional_reviewer_policy(),
            Some("risk_matched_only")
        );
        assert!(!profile.allow_broad_tool_exploration());
        assert_eq!(profile.coverage_expectation(), Some("High-risk-only pass."));
        assert!(profile.is_reduced_depth());
    }

    #[test]
    fn scope_profile_parses_snake_case_manifest() {
        let manifest = json!({
            "review_mode": "deep",
            "scope_profile": {
                "review_depth": "full_depth",
                "risk_focus_tags": ["security"],
                "max_dependency_hops": "policy_limited",
                "optional_reviewer_policy": "full",
                "allow_broad_tool_exploration": true,
                "coverage_expectation": "Full-depth pass."
            }
        });

        let profile =
            DeepReviewScopeProfile::from_manifest(&manifest).expect("scope profile should parse");

        assert_eq!(profile.review_depth(), "full_depth");
        assert_eq!(profile.max_dependency_hops(), Some("policy_limited"));
        assert!(profile.allow_broad_tool_exploration());
        assert!(!profile.is_reduced_depth());
    }

    #[test]
    fn scope_profile_missing_stays_compatible_with_legacy_manifest() {
        let manifest = json!({
            "reviewMode": "deep",
            "workPackets": []
        });

        assert!(DeepReviewScopeProfile::from_manifest(&manifest).is_none());
    }

    fn valid_evidence_pack_manifest() -> Value {
        json!({
            "reviewMode": "deep",
            "evidencePack": {
                "version": 1,
                "source": "target_manifest",
                "changedFiles": ["src/crates/adapters/api-layer/src/review.rs"],
                "diffStat": {
                    "fileCount": 1,
                    "totalChangedLines": 4,
                    "lineCountSource": "diff_stat"
                },
                "domainTags": ["api_layer"],
                "riskFocusTags": ["cross_boundary_api_contracts"],
                "packetIds": ["reviewer:ReviewArchitecture", "judge:ReviewJudge"],
                "hunkHints": [
                    {
                        "filePath": "src/crates/adapters/api-layer/src/review.rs",
                        "changedLineCount": 4,
                        "lineCountSource": "diff_stat"
                    }
                ],
                "contractHints": [
                    {
                        "kind": "api_contract",
                        "filePath": "src/crates/adapters/api-layer/src/review.rs",
                        "source": "path_classifier"
                    }
                ],
                "budget": {
                    "maxChangedFiles": 80,
                    "maxHunkHints": 80,
                    "maxContractHints": 40,
                    "omittedChangedFileCount": 0,
                    "omittedHunkHintCount": 0,
                    "omittedContractHintCount": 0
                },
                "privacy": {
                    "content": "metadata_only",
                    "excludes": [
                        "source_text",
                        "full_diff",
                        "model_output",
                        "provider_raw_body",
                        "full_file_contents"
                    ]
                }
            }
        })
    }

    #[test]
    fn evidence_pack_parses_metadata_only_manifest() {
        let manifest = valid_evidence_pack_manifest();

        let pack = DeepReviewEvidencePack::from_manifest(&manifest)
            .expect("evidence pack should validate")
            .expect("evidence pack should be present");

        assert_eq!(pack.version(), 1);
        assert_eq!(pack.source(), "target_manifest");
        assert_eq!(pack.content_boundary(), "metadata_only");
        assert_eq!(
            pack.changed_files()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["src/crates/adapters/api-layer/src/review.rs"]
        );
        assert_eq!(
            pack.packet_ids()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["reviewer:ReviewArchitecture", "judge:ReviewJudge"]
        );
        assert_eq!(pack.hunk_hint_count(), 1);
        assert_eq!(pack.contract_hint_count(), 1);
    }

    #[test]
    fn evidence_pack_parses_snake_case_manifest() {
        let manifest = json!({
            "review_mode": "deep",
            "evidence_pack": {
                "version": 1,
                "source": "target_manifest",
                "changed_files": ["src/web-ui/src/locales/en-US/flow-chat.json"],
                "diff_stat": {
                    "file_count": 1,
                    "total_changed_lines": 2,
                    "line_count_source": "diff_stat"
                },
                "domain_tags": ["frontend_i18n"],
                "risk_focus_tags": ["configuration_changes"],
                "packet_ids": ["reviewer:ReviewFrontend"],
                "hunk_hints": [
                    {
                        "file_path": "src/web-ui/src/locales/en-US/flow-chat.json",
                        "changed_line_count": 2,
                        "line_count_source": "diff_stat"
                    }
                ],
                "contract_hints": [
                    {
                        "kind": "i18n_key",
                        "file_path": "src/web-ui/src/locales/en-US/flow-chat.json",
                        "source": "path_classifier"
                    }
                ],
                "budget": {
                    "max_changed_files": 80,
                    "max_hunk_hints": 80,
                    "max_contract_hints": 40,
                    "omitted_changed_file_count": 0,
                    "omitted_hunk_hint_count": 0,
                    "omitted_contract_hint_count": 0
                },
                "privacy": {
                    "content": "metadata_only",
                    "excludes": [
                        "source_text",
                        "full_diff",
                        "model_output",
                        "provider_raw_body",
                        "full_file_contents"
                    ]
                }
            }
        });

        let pack = DeepReviewEvidencePack::from_manifest(&manifest)
            .expect("snake-case evidence pack should validate")
            .expect("evidence pack should be present");

        assert_eq!(
            pack.changed_files()[0],
            "src/web-ui/src/locales/en-US/flow-chat.json"
        );
        assert_eq!(pack.contract_hint_count(), 1);
    }

    #[test]
    fn evidence_pack_missing_stays_compatible_with_legacy_manifest() {
        let manifest = json!({
            "reviewMode": "deep",
            "workPackets": []
        });

        assert_eq!(
            DeepReviewEvidencePack::from_manifest(&manifest).expect("legacy manifest should parse"),
            None
        );
    }

    #[test]
    fn evidence_pack_rejects_forbidden_source_or_diff_payload_keys() {
        let mut manifest = valid_evidence_pack_manifest();
        manifest["evidencePack"]["sourceText"] = json!("fn main() {}");

        let error = DeepReviewEvidencePack::from_manifest(&manifest)
            .expect_err("source text must not be accepted");

        assert!(error.to_string().contains("forbidden evidence pack field"));
        assert!(error.to_string().contains("sourceText"));
    }

    #[test]
    fn evidence_pack_rejects_non_metadata_privacy_boundary() {
        let mut manifest = valid_evidence_pack_manifest();
        manifest["evidencePack"]["privacy"]["content"] = json!("full_diff");

        let error = DeepReviewEvidencePack::from_manifest(&manifest)
            .expect_err("full diff content must not be accepted");

        assert!(error.to_string().contains("privacy.content"));
        assert!(error.to_string().contains("metadata_only"));
    }

    #[test]
    fn evidence_pack_rejects_oversized_hunk_hint_arrays() {
        let mut manifest = valid_evidence_pack_manifest();
        let hunk_hints = (0..=EVIDENCE_PACK_HUNK_HINT_LIMIT)
            .map(|index| {
                json!({
                    "filePath": format!("src/lib_{index}.rs"),
                    "changedLineCount": 1,
                    "lineCountSource": "diff_stat"
                })
            })
            .collect::<Vec<_>>();
        manifest["evidencePack"]["hunkHints"] = json!(hunk_hints);

        let error = DeepReviewEvidencePack::from_manifest(&manifest)
            .expect_err("oversized hunk hints must be rejected");

        assert!(error.to_string().contains("hunkHints"));
        assert!(error.to_string().contains("max 80"));
    }
}
