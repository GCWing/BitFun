//! Build LoopX's `issue_fix_repository_context_input_v0` payload.
//!
//! This is the evidence half of the integration: LoopX holds no code-reading
//! ability and refuses to guess, so the quality of its route decisions depends
//! entirely on what this module reports. Its validator is strict, and a rejected
//! payload costs a whole subprocess round trip — so every constraint LoopX
//! enforces is enforced here too, at construction time.
//!
//! The rule that matters most: LoopX treats an aspect as *grounded* only when a
//! source is `freshness: current`, has `trust` of `authoritative` or `verified`,
//! and is not an external expert. It reports the whole context as grounded only
//! when `change_scope`, `reproduction`, and `validation` are all grounded.
//!
//! Grounding is not by itself the PR gate, though. Testing against the real CLI
//! showed that `--validation-label` — "how will you check this fix" — is what
//! actually permits the `fix_pr` route; a merely partial context still allows it,
//! and a fully grounded one without that label does not. Context grounding shapes
//! LoopX's reason codes and tells a caller what is still worth reading.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "issue_fix_repository_context_input_v0";

/// LoopX rejects a payload with more than this many sources.
pub const MAX_SOURCES: usize = 16;

const MAX_SOURCE_ID_CHARS: usize = 120;
const MAX_REFERENCE_CHARS: usize = 260;
const MAX_SUMMARY_CHARS: usize = 220;

/// Where a piece of evidence came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    RepositoryPolicy,
    ArchitectureDoc,
    MaintainerMap,
    TestSurface,
    SourceCode,
    PriorFix,
    /// LoopX requires `Advisory` trust for this kind.
    MemoryRetrieval,
    /// LoopX requires `Advisory` trust for this kind, and never counts it as
    /// grounding an aspect.
    ExternalExpert,
    KnowledgeBundle,
}

/// How much weight LoopX may place on a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    Authoritative,
    Verified,
    Advisory,
}

/// Whether a source was read at the pinned revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Requires a `repository_revision` on the context.
    Current,
    Stale,
    Unknown,
}

/// Which question a source helps answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportAspect {
    Architecture,
    Ownership,
    ChangeScope,
    Reproduction,
    Validation,
}

impl SupportAspect {
    /// The three aspects LoopX weighs when classifying a context's grounding.
    pub const REQUIRED_FOR_FIX: [Self; 3] =
        [Self::ChangeScope, Self::Reproduction, Self::Validation];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryContextError {
    EmptyField {
        field: &'static str,
    },
    TooLong {
        field: &'static str,
        limit: usize,
        actual: usize,
    },
    InvalidSourceId {
        source_id: String,
    },
    AbsoluteReference {
        reference: String,
    },
    TraversingReference {
        reference: String,
    },
    InvalidReferenceUrl {
        reference: String,
        reason: &'static str,
    },
    NoSupportedAspects {
        source_id: String,
    },
    TrustMustBeAdvisory {
        source_id: String,
    },
    CurrentFreshnessNeedsRevision {
        source_id: String,
    },
    DuplicateSourceId {
        source_id: String,
    },
    TooManySources {
        limit: usize,
        actual: usize,
    },
    NoSources,
}

impl fmt::Display for RepositoryContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => {
                write!(f, "{field} must not be empty")
            }
            Self::TooLong {
                field,
                limit,
                actual,
            } => write!(
                f,
                "{field} is {actual} characters, exceeding LoopX's limit of {limit}"
            ),
            Self::InvalidSourceId { source_id } => write!(
                f,
                "source id {source_id:?} must start alphanumeric and use only letters, digits, '_', '.', ':', or '-'"
            ),
            Self::AbsoluteReference { reference } => write!(
                f,
                "reference {reference:?} must be repository-relative; LoopX rejects absolute and home-relative paths as unsafe to publish"
            ),
            Self::TraversingReference { reference } => write!(
                f,
                "reference {reference:?} must not traverse outside the repository"
            ),
            Self::InvalidReferenceUrl { reference, reason } => {
                write!(f, "reference URL {reference:?} {reason}")
            }
            Self::NoSupportedAspects { source_id } => write!(
                f,
                "source {source_id:?} must support at least one aspect"
            ),
            Self::TrustMustBeAdvisory { source_id } => write!(
                f,
                "source {source_id:?} is a memory retrieval or external expert, which LoopX requires to be advisory"
            ),
            Self::CurrentFreshnessNeedsRevision { source_id } => write!(
                f,
                "source {source_id:?} claims current freshness, which requires a repository revision"
            ),
            Self::DuplicateSourceId { source_id } => {
                write!(f, "source id {source_id:?} appears more than once")
            }
            Self::TooManySources { limit, actual } => {
                write!(f, "{actual} sources exceeds LoopX's limit of {limit}")
            }
            Self::NoSources => write!(f, "a repository context needs at least one source"),
        }
    }
}

impl std::error::Error for RepositoryContextError {}

/// One validated piece of evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryContextSource {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub reference: String,
    pub trust: Trust,
    pub freshness: Freshness,
    pub supports: Vec<SupportAspect>,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_state: Option<String>,
}

/// A validated context payload, ready to serialize for LoopX.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryContext {
    pub schema_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_revision: Option<String>,
    pub sources: Vec<RepositoryContextSource>,
}

/// How LoopX will classify one aspect's coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AspectStatus {
    /// A current, trusted, non-expert source covers it.
    Grounded,
    /// Only weaker sources cover it.
    Advisory,
    Missing,
}

/// What LoopX will report for the context as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextStatus {
    /// All three fix-required aspects are grounded.
    Grounded,
    Partial,
    Ungrounded,
}

/// Accumulates sources and validates each as it is added.
///
/// Validating on `push` rather than at build time means a caller learns which
/// source is wrong, instead of getting one failure for the whole payload.
#[derive(Debug, Clone, Default)]
pub struct RepositoryContextBuilder {
    repository_revision: Option<String>,
    sources: Vec<RepositoryContextSource>,
}

impl RepositoryContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pin the revision the sources were read at.
    ///
    /// Required before any source may claim `Freshness::Current`, which in turn
    /// is required for that source to ground an aspect.
    pub fn repository_revision(mut self, revision: impl Into<String>) -> Self {
        let revision = revision.into();
        self.repository_revision = (!revision.trim().is_empty()).then_some(revision);
        self
    }

    pub fn has_revision(&self) -> bool {
        self.repository_revision.is_some()
    }

    /// Validate and append one source.
    pub fn push(
        &mut self,
        source: RepositoryContextSource,
    ) -> Result<&mut Self, RepositoryContextError> {
        let source = self.validate(source)?;
        self.sources.push(source);
        Ok(self)
    }

    fn validate(
        &self,
        mut source: RepositoryContextSource,
    ) -> Result<RepositoryContextSource, RepositoryContextError> {
        source.source_id = validate_source_id(&source.source_id)?;
        source.reference = validate_reference(&source.reference)?;
        source.summary = validate_text(&source.summary, "summary", MAX_SUMMARY_CHARS)?;

        if source.supports.is_empty() {
            return Err(RepositoryContextError::NoSupportedAspects {
                source_id: source.source_id,
            });
        }
        // LoopX sorts and dedupes these; matching here keeps the payload stable.
        source.supports = source
            .supports
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        if matches!(
            source.source_kind,
            SourceKind::MemoryRetrieval | SourceKind::ExternalExpert
        ) && source.trust != Trust::Advisory
        {
            return Err(RepositoryContextError::TrustMustBeAdvisory {
                source_id: source.source_id,
            });
        }

        if source.freshness == Freshness::Current && !self.has_revision() {
            return Err(RepositoryContextError::CurrentFreshnessNeedsRevision {
                source_id: source.source_id,
            });
        }

        if self
            .sources
            .iter()
            .any(|existing| existing.source_id == source.source_id)
        {
            return Err(RepositoryContextError::DuplicateSourceId {
                source_id: source.source_id,
            });
        }

        if self.sources.len() + 1 > MAX_SOURCES {
            return Err(RepositoryContextError::TooManySources {
                limit: MAX_SOURCES,
                actual: self.sources.len() + 1,
            });
        }

        Ok(source)
    }

    /// Classify one aspect exactly as LoopX will.
    pub fn aspect_status(&self, aspect: SupportAspect) -> AspectStatus {
        let matching = self
            .sources
            .iter()
            .filter(|source| source.supports.contains(&aspect));
        let mut any_match = false;
        for source in matching {
            any_match = true;
            if source.freshness == Freshness::Current
                && matches!(source.trust, Trust::Authoritative | Trust::Verified)
                && source.source_kind != SourceKind::ExternalExpert
            {
                return AspectStatus::Grounded;
            }
        }
        if any_match {
            AspectStatus::Advisory
        } else {
            AspectStatus::Missing
        }
    }

    /// Predict LoopX's overall verdict without spending a subprocess call.
    pub fn context_status(&self) -> ContextStatus {
        let statuses = SupportAspect::REQUIRED_FOR_FIX.map(|aspect| self.aspect_status(aspect));
        if statuses.iter().all(|s| *s == AspectStatus::Grounded) {
            ContextStatus::Grounded
        } else if statuses.contains(&AspectStatus::Grounded) {
            ContextStatus::Partial
        } else {
            ContextStatus::Ungrounded
        }
    }

    /// Which fix-required aspects are not yet grounded.
    ///
    /// A caller uses this to decide what else to read before asking LoopX. Gaps
    /// here weaken the context rather than block a fix outright, so treat this as
    /// a reading list, not a hard gate.
    pub fn ungrounded_required_aspects(&self) -> Vec<SupportAspect> {
        SupportAspect::REQUIRED_FOR_FIX
            .into_iter()
            .filter(|aspect| self.aspect_status(*aspect) != AspectStatus::Grounded)
            .collect()
    }

    pub fn build(self) -> Result<RepositoryContext, RepositoryContextError> {
        if self.sources.is_empty() {
            return Err(RepositoryContextError::NoSources);
        }
        Ok(RepositoryContext {
            schema_version: SCHEMA_VERSION.to_string(),
            repository_revision: self.repository_revision,
            sources: self.sources,
        })
    }
}

fn validate_text(
    value: &str,
    field: &'static str,
    limit: usize,
) -> Result<String, RepositoryContextError> {
    // LoopX collapses whitespace before measuring, so do the same or a payload
    // that looks short enough here could still be rejected there.
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return Err(RepositoryContextError::EmptyField { field });
    }
    let actual = compact.chars().count();
    if actual > limit {
        return Err(RepositoryContextError::TooLong {
            field,
            limit,
            actual,
        });
    }
    Ok(compact)
}

fn validate_source_id(value: &str) -> Result<String, RepositoryContextError> {
    let id = validate_text(value, "source_id", MAX_SOURCE_ID_CHARS)?;
    let mut chars = id.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'));
    if !valid {
        return Err(RepositoryContextError::InvalidSourceId { source_id: id });
    }
    Ok(id)
}

/// Enforce LoopX's publish-safety rules on a reference.
///
/// A local absolute path would leak the operator's filesystem layout into a
/// payload that may reach a public issue thread, so LoopX rejects it — and so
/// does this, before the round trip.
fn validate_reference(value: &str) -> Result<String, RepositoryContextError> {
    let reference = validate_text(value, "reference", MAX_REFERENCE_CHARS)?;

    if reference.contains("://") {
        return validate_reference_url(reference);
    }

    if reference.starts_with('/') || reference.starts_with('~') || is_windows_absolute(&reference) {
        return Err(RepositoryContextError::AbsoluteReference { reference });
    }
    if reference.split(['/', '\\']).any(|segment| segment == "..") {
        return Err(RepositoryContextError::TraversingReference { reference });
    }
    // LoopX parses references as POSIX paths, so normalize separators rather than
    // sending a Windows-style path it would treat as one long segment.
    Ok(reference.replace('\\', "/"))
}

fn validate_reference_url(reference: String) -> Result<String, RepositoryContextError> {
    let Some((scheme, rest)) = reference.split_once("://") else {
        return Err(RepositoryContextError::InvalidReferenceUrl {
            reference,
            reason: "must be a well-formed URL",
        });
    };
    if scheme != "https" {
        return Err(RepositoryContextError::InvalidReferenceUrl {
            reference,
            reason: "must use https",
        });
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() {
        return Err(RepositoryContextError::InvalidReferenceUrl {
            reference,
            reason: "must name a host",
        });
    }
    if authority.contains('@') {
        return Err(RepositoryContextError::InvalidReferenceUrl {
            reference,
            reason: "must not embed user info",
        });
    }
    if rest.contains('?') {
        return Err(RepositoryContextError::InvalidReferenceUrl {
            reference,
            reason: "must not contain query parameters",
        });
    }
    Ok(reference)
}

fn is_windows_absolute(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(
        id: &str,
        kind: SourceKind,
        trust: Trust,
        freshness: Freshness,
        supports: &[SupportAspect],
    ) -> RepositoryContextSource {
        RepositoryContextSource {
            source_id: id.to_string(),
            source_kind: kind,
            reference: "src/lib.rs".to_string(),
            trust,
            freshness,
            supports: supports.to_vec(),
            summary: "a compact public-safe summary".to_string(),
            consultation_state: None,
        }
    }

    fn grounded_builder() -> RepositoryContextBuilder {
        let mut builder = RepositoryContextBuilder::new().repository_revision("abc123");
        builder
            .push(source(
                "scope",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Current,
                &[SupportAspect::ChangeScope, SupportAspect::Reproduction],
            ))
            .expect("scope source is valid");
        builder
            .push(source(
                "validation",
                SourceKind::TestSurface,
                Trust::Verified,
                Freshness::Current,
                &[SupportAspect::Validation],
            ))
            .expect("validation source is valid");
        builder
    }

    #[test]
    fn a_context_covering_all_required_aspects_is_grounded() {
        let builder = grounded_builder();
        assert_eq!(builder.context_status(), ContextStatus::Grounded);
        assert!(builder.ungrounded_required_aspects().is_empty());
    }

    #[test]
    fn a_missing_validation_source_leaves_the_context_partial() {
        // Verified against the real CLI: LoopX reports this exact shape as
        // `context_status: partial` with validation as the sole unresolved aspect.
        let mut builder = RepositoryContextBuilder::new().repository_revision("abc123");
        builder
            .push(source(
                "scope",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Current,
                &[SupportAspect::ChangeScope, SupportAspect::Reproduction],
            ))
            .expect("scope source is valid");

        assert_eq!(builder.context_status(), ContextStatus::Partial);
        assert_eq!(
            builder.ungrounded_required_aspects(),
            vec![SupportAspect::Validation]
        );
    }

    #[test]
    fn stale_sources_ground_nothing() {
        let mut builder = RepositoryContextBuilder::new().repository_revision("abc123");
        builder
            .push(source(
                "stale",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Stale,
                &SupportAspect::REQUIRED_FOR_FIX,
            ))
            .expect("stale source is still valid");

        assert_eq!(builder.context_status(), ContextStatus::Ungrounded);
        assert_eq!(
            builder.aspect_status(SupportAspect::ChangeScope),
            AspectStatus::Advisory
        );
    }

    #[test]
    fn advisory_trust_grounds_nothing() {
        let mut builder = RepositoryContextBuilder::new().repository_revision("abc123");
        builder
            .push(source(
                "memory",
                SourceKind::MemoryRetrieval,
                Trust::Advisory,
                Freshness::Current,
                &SupportAspect::REQUIRED_FOR_FIX,
            ))
            .expect("advisory memory source is valid");

        assert_eq!(builder.context_status(), ContextStatus::Ungrounded);
    }

    #[test]
    fn an_external_expert_never_grounds_an_aspect() {
        // LoopX excludes experts from grounding even when everything else lines
        // up, because their answers still need local verification.
        let mut builder = RepositoryContextBuilder::new().repository_revision("abc123");
        builder
            .push(source(
                "expert",
                SourceKind::ExternalExpert,
                Trust::Advisory,
                Freshness::Current,
                &SupportAspect::REQUIRED_FOR_FIX,
            ))
            .expect("expert source is valid");

        assert_eq!(builder.context_status(), ContextStatus::Ungrounded);
        assert_eq!(
            builder.aspect_status(SupportAspect::Validation),
            AspectStatus::Advisory
        );
    }

    #[test]
    fn an_unmatched_aspect_is_missing_not_advisory() {
        let builder = RepositoryContextBuilder::new().repository_revision("abc123");
        assert_eq!(
            builder.aspect_status(SupportAspect::Ownership),
            AspectStatus::Missing
        );
    }

    #[test]
    fn memory_retrieval_must_be_advisory() {
        let mut builder = RepositoryContextBuilder::new().repository_revision("abc123");
        let error = builder
            .push(source(
                "memory",
                SourceKind::MemoryRetrieval,
                Trust::Verified,
                Freshness::Current,
                &[SupportAspect::Architecture],
            ))
            .expect_err("verified memory retrieval is rejected");
        assert!(matches!(
            error,
            RepositoryContextError::TrustMustBeAdvisory { .. }
        ));
    }

    #[test]
    fn current_freshness_requires_a_revision() {
        let mut builder = RepositoryContextBuilder::new();
        let error = builder
            .push(source(
                "scope",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Current,
                &[SupportAspect::ChangeScope],
            ))
            .expect_err("current freshness without a revision is rejected");
        assert!(matches!(
            error,
            RepositoryContextError::CurrentFreshnessNeedsRevision { .. }
        ));
    }

    #[test]
    fn a_blank_revision_does_not_count() {
        let builder = RepositoryContextBuilder::new().repository_revision("   ");
        assert!(!builder.has_revision());
    }

    #[test]
    fn absolute_references_are_rejected() {
        // The whole point: a local path would leak the operator's filesystem into
        // a payload that can reach a public thread.
        for path in [
            "/home/user/repo/src/lib.rs",
            "~/repo/src/lib.rs",
            "C:/codeagent/BitFun/src/lib.rs",
            "C:\\codeagent\\BitFun\\src\\lib.rs",
        ] {
            let error = validate_reference(path).expect_err("absolute paths are rejected");
            assert!(
                matches!(error, RepositoryContextError::AbsoluteReference { .. }),
                "{path} produced {error:?}"
            );
        }
    }

    #[test]
    fn traversing_references_are_rejected() {
        for path in ["../secrets.txt", "src/../../etc/passwd", "src\\..\\out.txt"] {
            let error = validate_reference(path).expect_err("traversal is rejected");
            assert!(
                matches!(error, RepositoryContextError::TraversingReference { .. }),
                "{path} produced {error:?}"
            );
        }
    }

    #[test]
    fn windows_separators_are_normalized_to_posix() {
        // LoopX parses references as POSIX paths, so a backslash path would look
        // like one long segment to it.
        let reference =
            validate_reference("src\\web-ui\\src\\app.tsx").expect("relative path is accepted");
        assert_eq!(reference, "src/web-ui/src/app.tsx");
    }

    #[test]
    fn a_bare_drive_letter_is_not_treated_as_absolute() {
        // "C:" without a separator is a valid relative name, not a drive root.
        assert!(validate_reference("C:file.rs").is_ok());
    }

    #[test]
    fn https_urls_are_accepted_without_query_parameters() {
        let reference = validate_reference("https://github.com/example/repo/blob/main/README.md")
            .expect("plain https URL is accepted");
        assert!(reference.starts_with("https://"));
    }

    #[test]
    fn unsafe_urls_are_rejected() {
        for (url, expected) in [
            ("http://example.com/a", "must use https"),
            ("https://user:pw@example.com/a", "must not embed user info"),
            (
                "https://example.com/a?token=secret",
                "must not contain query parameters",
            ),
            ("https:///no-host", "must name a host"),
        ] {
            let error = validate_reference(url).expect_err("unsafe URL is rejected");
            match error {
                RepositoryContextError::InvalidReferenceUrl { reason, .. } => {
                    assert_eq!(reason, expected, "for {url}");
                }
                other => panic!("expected a URL error for {url}, got {other:?}"),
            }
        }
    }

    #[test]
    fn source_ids_must_match_loopx_shape() {
        let mut builder = RepositoryContextBuilder::new();
        for bad in ["_leading", "-leading", "has space", "has/slash"] {
            let mut candidate = source(
                bad,
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Unknown,
                &[SupportAspect::ChangeScope],
            );
            candidate.source_id = bad.to_string();
            let error = builder.push(candidate).expect_err("invalid id is rejected");
            assert!(
                matches!(error, RepositoryContextError::InvalidSourceId { .. }),
                "{bad} produced {error:?}"
            );
        }
        // The permitted punctuation still works.
        assert!(builder
            .push(source(
                "bitfun.workspace:icon-branch_1",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Unknown,
                &[SupportAspect::ChangeScope],
            ))
            .is_ok());
    }

    #[test]
    fn duplicate_source_ids_are_rejected() {
        let mut builder = grounded_builder();
        let error = builder
            .push(source(
                "scope",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Current,
                &[SupportAspect::Architecture],
            ))
            .expect_err("a repeated id is rejected");
        assert!(matches!(
            error,
            RepositoryContextError::DuplicateSourceId { .. }
        ));
    }

    #[test]
    fn the_source_limit_is_enforced() {
        let mut builder = RepositoryContextBuilder::new();
        for index in 0..MAX_SOURCES {
            builder
                .push(source(
                    &format!("source{index}"),
                    SourceKind::SourceCode,
                    Trust::Verified,
                    Freshness::Unknown,
                    &[SupportAspect::ChangeScope],
                ))
                .expect("sources within the limit are accepted");
        }
        let error = builder
            .push(source(
                "overflow",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Unknown,
                &[SupportAspect::ChangeScope],
            ))
            .expect_err("one source past the limit is rejected");
        assert!(matches!(
            error,
            RepositoryContextError::TooManySources {
                limit: MAX_SOURCES,
                actual: 17
            }
        ));
    }

    #[test]
    fn summaries_are_bounded_and_whitespace_collapsed() {
        let mut builder = RepositoryContextBuilder::new();
        let mut candidate = source(
            "long",
            SourceKind::SourceCode,
            Trust::Verified,
            Freshness::Unknown,
            &[SupportAspect::ChangeScope],
        );
        candidate.summary = "s".repeat(MAX_SUMMARY_CHARS + 1);
        let error = builder
            .push(candidate)
            .expect_err("an oversized summary is rejected");
        assert!(matches!(
            error,
            RepositoryContextError::TooLong {
                field: "summary",
                ..
            }
        ));

        let mut spaced = source(
            "spaced",
            SourceKind::SourceCode,
            Trust::Verified,
            Freshness::Unknown,
            &[SupportAspect::ChangeScope],
        );
        spaced.summary = "  collapse   these\n\nspaces  ".to_string();
        builder.push(spaced).expect("whitespace is collapsed");
        assert_eq!(builder.sources[0].summary, "collapse these spaces");
    }

    #[test]
    fn a_source_needs_at_least_one_aspect() {
        let mut builder = RepositoryContextBuilder::new();
        let error = builder
            .push(source(
                "empty",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Unknown,
                &[],
            ))
            .expect_err("a source with no aspects is rejected");
        assert!(matches!(
            error,
            RepositoryContextError::NoSupportedAspects { .. }
        ));
    }

    #[test]
    fn supports_are_sorted_and_deduplicated() {
        let mut builder = RepositoryContextBuilder::new();
        builder
            .push(source(
                "dupes",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Unknown,
                &[
                    SupportAspect::Validation,
                    SupportAspect::Architecture,
                    SupportAspect::Validation,
                ],
            ))
            .expect("duplicate aspects are tolerated");
        assert_eq!(
            builder.sources[0].supports,
            vec![SupportAspect::Architecture, SupportAspect::Validation]
        );
    }

    #[test]
    fn an_empty_context_cannot_be_built() {
        let error = RepositoryContextBuilder::new()
            .build()
            .expect_err("an empty context is rejected");
        assert_eq!(error, RepositoryContextError::NoSources);
    }

    #[test]
    fn the_payload_serializes_to_loopx_field_names() {
        let context = grounded_builder().build().expect("context builds");
        let json = serde_json::to_value(&context).expect("context serializes");

        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert_eq!(json["repository_revision"], "abc123");
        assert_eq!(json["sources"][0]["source_kind"], "source_code");
        assert_eq!(json["sources"][0]["trust"], "verified");
        assert_eq!(json["sources"][0]["freshness"], "current");
        assert_eq!(json["sources"][0]["supports"][0], "change_scope");
        assert_eq!(json["sources"][1]["source_kind"], "test_surface");
        // LoopX rejects unknown fields, so an absent consultation state must be
        // omitted rather than serialized as null.
        assert!(json["sources"][0].get("consultation_state").is_none());
    }

    #[test]
    fn a_revisionless_payload_omits_the_revision_field() {
        let mut builder = RepositoryContextBuilder::new();
        builder
            .push(source(
                "unknown",
                SourceKind::SourceCode,
                Trust::Verified,
                Freshness::Unknown,
                &[SupportAspect::ChangeScope],
            ))
            .expect("source is valid");
        let json = serde_json::to_value(builder.build().expect("context builds"))
            .expect("context serializes");
        assert!(json.get("repository_revision").is_none());
    }
}
