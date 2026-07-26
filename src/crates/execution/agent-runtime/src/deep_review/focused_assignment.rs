use super::{DeepReviewPolicyViolation, ReviewTargetEvidence};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const ASSIGNMENT_TEXT_LIMIT: usize = 1_000;
const ASSIGNMENT_PATH_LIMIT: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedReviewPathAccess {
    AssignedChange,
    UnassignedChange,
    UnchangedDependency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusedReviewAssignment {
    question_id: String,
    question: String,
    independent_value: String,
    target_fingerprint: String,
    allowed_changed_paths: Vec<String>,
    expected_evidence: String,
    capability_key: String,
    capability_fingerprint: String,
}

impl FocusedReviewAssignment {
    pub fn from_input(
        manifest: &Value,
        input: &Value,
        packet_id: Option<&str>,
    ) -> Result<Self, DeepReviewPolicyViolation> {
        if !is_adaptive_review_manifest(manifest) {
            return Err(violation(
                "focused_review_manifest_required",
                "Focused Review assignments require an adaptive Review manifest",
            ));
        }
        let evidence = ReviewTargetEvidence::from_manifest(manifest)
            .map_err(|error| violation("focused_review_target_invalid", error.to_string()))?
            .ok_or_else(|| {
                violation(
                    "focused_review_target_required",
                    "Focused Review assignments require target evidence",
                )
            })?;
        let object = input.as_object().ok_or_else(|| {
            violation(
                "focused_review_assignment_invalid",
                "focused_assignment must be an object",
            )
        })?;
        let question = bounded_string(object.get("question"), "question")?;
        let independent_value =
            bounded_string(object.get("independent_value"), "independent_value")?;
        let target_fingerprint =
            bounded_string(object.get("target_fingerprint"), "target_fingerprint")?;
        let expected_evidence =
            bounded_string(object.get("expected_evidence"), "expected_evidence")?;
        let capability_key = bounded_string(object.get("capability_key"), "capability_key")?;
        let capability_fingerprint = bounded_string(
            object.get("capability_fingerprint"),
            "capability_fingerprint",
        )?;
        if target_fingerprint != evidence.fingerprint() {
            return Err(violation(
                "focused_review_target_mismatch",
                "focused_assignment target_fingerprint does not match the active review target",
            ));
        }

        let explicit_paths = object.get("allowed_changed_paths");
        if explicit_paths.is_some() == packet_id.is_some() {
            return Err(violation(
                "focused_review_scope_invalid",
                "focused_assignment requires exactly one changed-path scope or packet scope",
            ));
        }
        let raw_paths = if let Some(raw) = explicit_paths {
            parse_path_array(raw)?
        } else {
            packet_paths(manifest, packet_id.unwrap_or_default())?
        };
        let mut seen = HashSet::new();
        let mut allowed_changed_paths = Vec::with_capacity(raw_paths.len());
        for path in raw_paths {
            let Some(canonical) = evidence.canonical_file_path_for_path(&path) else {
                return Err(violation(
                    "focused_review_scope_invalid",
                    format!("focused_assignment path '{path}' is outside the review target"),
                ));
            };
            if seen.insert(canonical.to_string()) {
                allowed_changed_paths.push(canonical.to_string());
            }
        }
        if allowed_changed_paths.is_empty() {
            return Err(violation(
                "focused_review_scope_invalid",
                "focused_assignment must contain at least one changed path",
            ));
        }
        allowed_changed_paths.sort();

        let question_id = derive_question_id(&question, &target_fingerprint);
        Ok(Self {
            question_id,
            question,
            independent_value,
            target_fingerprint,
            allowed_changed_paths,
            expected_evidence,
            capability_key,
            capability_fingerprint,
        })
    }

    pub fn from_manifest(manifest: &Value) -> Result<Option<Self>, DeepReviewPolicyViolation> {
        let Some(raw) = manifest
            .get("focusedAssignment")
            .or_else(|| manifest.get("focused_assignment"))
        else {
            return Ok(None);
        };
        let assignment = serde_json::from_value::<Self>(raw.clone()).map_err(|_| {
            violation(
                "focused_review_assignment_invalid",
                "focusedAssignment is malformed",
            )
        })?;
        let evidence = ReviewTargetEvidence::from_manifest(manifest)
            .map_err(|error| violation("focused_review_target_invalid", error.to_string()))?
            .ok_or_else(|| {
                violation(
                    "focused_review_target_required",
                    "Focused Review assignments require target evidence",
                )
            })?;
        if assignment.target_fingerprint != evidence.fingerprint()
            || assignment.allowed_changed_paths.is_empty()
            || assignment.allowed_changed_paths.len() > ASSIGNMENT_PATH_LIMIT
            || assignment
                .allowed_changed_paths
                .iter()
                .any(|path| evidence.canonical_file_path_for_path(path) != Some(path.as_str()))
        {
            return Err(violation(
                "focused_review_scope_invalid",
                "focusedAssignment no longer matches the active review target",
            ));
        }
        Ok(Some(assignment))
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }

    pub fn question_id(&self) -> &str {
        &self.question_id
    }

    pub fn question(&self) -> &str {
        &self.question
    }

    pub fn capability_key(&self) -> &str {
        &self.capability_key
    }

    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    pub fn allowed_changed_paths(&self) -> &[String] {
        &self.allowed_changed_paths
    }

    pub fn path_access_with_evidence(
        &self,
        evidence: &ReviewTargetEvidence,
        path: &str,
    ) -> FocusedReviewPathAccess {
        classify_path_access(
            &self.allowed_changed_paths,
            evidence.canonical_file_path_for_path(path),
        )
    }

    pub fn path_access_with_local_evidence(
        &self,
        evidence: &ReviewTargetEvidence,
        path: &str,
    ) -> FocusedReviewPathAccess {
        classify_path_access(
            &self.allowed_changed_paths,
            evidence.canonical_file_path_for_local_path(path),
        )
    }
}

fn classify_path_access(
    allowed_changed_paths: &[String],
    canonical: Option<&str>,
) -> FocusedReviewPathAccess {
    match canonical {
        Some(canonical) if allowed_changed_paths.iter().any(|item| item == canonical) => {
            FocusedReviewPathAccess::AssignedChange
        }
        Some(_) => FocusedReviewPathAccess::UnassignedChange,
        None => FocusedReviewPathAccess::UnchangedDependency,
    }
}

pub fn is_adaptive_review_manifest(manifest: &Value) -> bool {
    if manifest.get("reviewMode").and_then(Value::as_str) != Some("deep") {
        return false;
    }
    manifest
        .get("adaptiveReview")
        .or_else(|| manifest.get("adaptive_review"))
        .and_then(Value::as_object)
        .is_some_and(|adaptive| {
            adaptive.get("version").and_then(Value::as_u64) == Some(1)
                && adaptive
                    .get("maxFocusedCalls")
                    .or_else(|| adaptive.get("max_focused_calls"))
                    .and_then(Value::as_u64)
                    .is_some_and(|value| value <= 3)
        })
}

pub fn adaptive_review_max_focused_calls(manifest: &Value) -> Option<usize> {
    if !is_adaptive_review_manifest(manifest) {
        return None;
    }
    let adaptive = manifest
        .get("adaptiveReview")
        .or_else(|| manifest.get("adaptive_review"))?;
    let value = adaptive
        .get("maxFocusedCalls")
        .or_else(|| adaptive.get("max_focused_calls"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())?;
    Some(value)
}

fn bounded_string(
    raw: Option<&Value>,
    field: &'static str,
) -> Result<String, DeepReviewPolicyViolation> {
    raw.and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= ASSIGNMENT_TEXT_LIMIT)
        .map(str::to_string)
        .ok_or_else(|| {
            violation(
                "focused_review_assignment_invalid",
                format!("focused_assignment.{field} must be a bounded non-empty string"),
            )
        })
}

fn parse_path_array(raw: &Value) -> Result<Vec<String>, DeepReviewPolicyViolation> {
    let paths = raw
        .as_array()
        .filter(|paths| !paths.is_empty() && paths.len() <= ASSIGNMENT_PATH_LIMIT)
        .ok_or_else(|| {
            violation(
                "focused_review_scope_invalid",
                "allowed_changed_paths must be a bounded non-empty array",
            )
        })?;
    paths
        .iter()
        .map(|path| {
            path.as_str()
                .filter(|path| !path.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    violation(
                        "focused_review_scope_invalid",
                        "allowed_changed_paths must contain non-empty strings",
                    )
                })
        })
        .collect()
}

fn packet_paths(
    manifest: &Value,
    packet_id: &str,
) -> Result<Vec<String>, DeepReviewPolicyViolation> {
    let packet = manifest
        .get("workPackets")
        .or_else(|| manifest.get("work_packets"))
        .and_then(Value::as_array)
        .and_then(|packets| {
            packets.iter().find(|packet| {
                packet
                    .get("packetId")
                    .or_else(|| packet.get("packet_id"))
                    .and_then(Value::as_str)
                    == Some(packet_id)
            })
        })
        .ok_or_else(|| {
            violation(
                "focused_review_scope_invalid",
                "focused_assignment packet is not active in the review manifest",
            )
        })?;
    parse_path_array(
        packet
            .get("assignedScope")
            .or_else(|| packet.get("assigned_scope"))
            .and_then(|scope| scope.get("files"))
            .ok_or_else(|| {
                violation(
                    "focused_review_scope_invalid",
                    "focused_assignment packet has no file scope",
                )
            })?,
    )
}

fn derive_question_id(question: &str, target: &str) -> String {
    let mut hasher = Sha256::new();
    let normalized_question = question
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    for value in [&normalized_question, target] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    format!("focus-{}", &hex::encode(hasher.finalize())[..16])
}

fn violation(code: &'static str, message: impl Into<String>) -> DeepReviewPolicyViolation {
    DeepReviewPolicyViolation::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest() -> serde_json::Value {
        json!({
            "reviewMode": "deep",
            "adaptiveReview": { "version": 1, "maxFocusedCalls": 2 },
            "evidencePack": {
                "reviewTarget": {
                    "version": 1,
                    "source": "git_range",
                    "fingerprint": "target-12345678",
                    "baseRevision": "1111111111111111111111111111111111111111",
                    "headRevision": "2222222222222222222222222222222222222222",
                    "completeness": "complete",
                    "workspaceBinding": "matching_clean",
                    "files": [
                        { "path": "src/new.rs", "previousPath": "src/old.rs", "status": "renamed", "completeness": "complete" },
                        { "path": "src/other.rs", "status": "modified", "completeness": "complete" }
                    ],
                    "diffRefs": [],
                    "limitations": []
                }
            }
        })
    }

    #[test]
    fn adaptive_marker_requires_a_deep_review_manifest() {
        let marker_only = json!({
            "adaptiveReview": { "version": 1, "maxFocusedCalls": 2 }
        });

        assert!(!is_adaptive_review_manifest(&marker_only));
        assert_eq!(adaptive_review_max_focused_calls(&marker_only), None);
    }

    #[test]
    fn explicit_assignment_canonicalizes_rename_scope_and_classifies_paths() {
        let assignment = FocusedReviewAssignment::from_input(
            &manifest(),
            &json!({
                "question": "Could the rename break module discovery?",
                "independent_value": "The primary review found an unresolved rename boundary.",
                "target_fingerprint": "target-12345678",
                "allowed_changed_paths": ["src/old.rs"],
                "expected_evidence": "A concrete call path or a proof that discovery is unchanged.",
                "capability_key": "builtin::review-worker",
                "capability_fingerprint": "capability-12345678"
            }),
            None,
        )
        .expect("assignment should be valid");

        assert_eq!(assignment.allowed_changed_paths(), &["src/new.rs"]);
        let evidence = ReviewTargetEvidence::from_manifest(&manifest())
            .expect("evidence should parse")
            .expect("manifest should contain evidence");
        assert_eq!(
            assignment.path_access_with_evidence(&evidence, "src/old.rs"),
            FocusedReviewPathAccess::AssignedChange
        );
        assert_eq!(
            assignment.path_access_with_evidence(&evidence, "src/other.rs"),
            FocusedReviewPathAccess::UnassignedChange
        );
        assert_eq!(
            assignment.path_access_with_evidence(&evidence, "src/support.rs"),
            FocusedReviewPathAccess::UnchangedDependency
        );
    }

    #[test]
    fn assignment_rejects_scope_outside_the_review_target() {
        let error = FocusedReviewAssignment::from_input(
            &manifest(),
            &json!({
                "question": "Is this safe?",
                "independent_value": "The primary review needs independent evidence.",
                "target_fingerprint": "target-12345678",
                "allowed_changed_paths": ["src/missing.rs"],
                "expected_evidence": "A concrete path.",
                "capability_key": "builtin::review-worker",
                "capability_fingerprint": "capability-12345678"
            }),
            None,
        )
        .expect_err("scope must be target-bound");

        assert_eq!(error.code, "focused_review_scope_invalid");
    }

    #[test]
    fn assignment_preserves_legal_path_whitespace() {
        let mut manifest = manifest();
        manifest["evidencePack"]["reviewTarget"]["files"] = json!([
            { "path": " src/space.rs ", "status": "modified", "completeness": "complete" }
        ]);

        let assignment = FocusedReviewAssignment::from_input(
            &manifest,
            &json!({
                "question": "Is this path handled exactly?",
                "independent_value": "The path boundary needs independent evidence.",
                "target_fingerprint": "target-12345678",
                "allowed_changed_paths": [" src/space.rs "],
                "expected_evidence": "An exact path match.",
                "capability_key": "builtin::review-worker",
                "capability_fingerprint": "capability-12345678"
            }),
            None,
        )
        .expect("legal path whitespace should remain part of the path");

        assert_eq!(assignment.allowed_changed_paths(), &[" src/space.rs "]);
    }

    #[test]
    fn recreated_rename_source_remains_a_distinct_changed_path() {
        let mut manifest = manifest();
        manifest["evidencePack"]["reviewTarget"]["files"] = json!([
            { "path": "src/new.rs", "previousPath": "src/old.rs", "status": "renamed", "completeness": "complete" },
            { "path": "src/old.rs", "status": "added", "completeness": "complete" }
        ]);
        let assignment = FocusedReviewAssignment::from_input(
            &manifest,
            &json!({
                "question": "Could the renamed implementation break callers?",
                "independent_value": "The renamed implementation needs isolated evidence.",
                "target_fingerprint": "target-12345678",
                "allowed_changed_paths": ["src/new.rs"],
                "expected_evidence": "A concrete call path.",
                "capability_key": "builtin::review-worker",
                "capability_fingerprint": "capability-12345678"
            }),
            None,
        )
        .expect("assignment should be valid");
        let evidence = ReviewTargetEvidence::from_manifest(&manifest)
            .expect("evidence should parse")
            .expect("manifest should contain evidence");

        assert_eq!(
            assignment.path_access_with_evidence(&evidence, "src/old.rs"),
            FocusedReviewPathAccess::UnassignedChange
        );
    }

    #[test]
    fn question_identity_is_stable_across_disjoint_target_scopes() {
        let input_for = |path: &str, capability: &str| {
            json!({
                "question": "Could this contract break callers?",
                "independent_value": "The same question needs evidence from disjoint packets.",
                "target_fingerprint": "target-12345678",
                "allowed_changed_paths": [path],
                "expected_evidence": "A concrete call path.",
                "capability_key": capability,
                "capability_fingerprint": "capability-12345678"
            })
        };
        let first = FocusedReviewAssignment::from_input(
            &manifest(),
            &input_for("src/new.rs", "builtin::review-worker"),
            None,
        )
        .expect("first scope should be valid");
        let second = FocusedReviewAssignment::from_input(
            &manifest(),
            &input_for("src/other.rs", "skill:project::custom::code-review-testing"),
            None,
        )
        .expect("second scope should be valid");

        assert_eq!(first.question_id(), second.question_id());
    }
}
