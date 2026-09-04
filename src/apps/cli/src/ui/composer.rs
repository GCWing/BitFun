use std::fmt;
use std::sync::Arc;

use base64::Engine as _;
use openbitfun_agent_runtime::sdk::{AgentInputAttachment, AgentWorkspaceReference};

pub(crate) const MAX_COMPOSER_IMAGES: usize = 5;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ComposerImage {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) mime_type: String,
    bytes: Arc<[u8]>,
}

impl ComposerImage {
    pub(crate) fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        mime_type: impl Into<String>,
        bytes: Arc<[u8]>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            mime_type: mime_type.into(),
            bytes,
        }
    }

    fn to_runtime_attachment(&self) -> AgentInputAttachment {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&self.bytes);
        AgentInputAttachment::remote_image(
            self.id.clone(),
            self.name.clone(),
            format!("data:{};base64,{encoded}", self.mime_type),
        )
    }

    #[cfg(test)]
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for ComposerImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComposerImage")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("mime_type", &self.mime_type)
            .field("byte_len", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposerSourceRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposerImageAttachment {
    pub(crate) image: ComposerImage,
    pub(crate) source: ComposerSourceRange,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ComposerDraft {
    pub(crate) text: String,
    pub(crate) workspace_references: Vec<AgentWorkspaceReference>,
    pub(crate) image_attachments: Vec<ComposerImageAttachment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SourceReconcileOutcome {
    pub(crate) retained: usize,
    pub(crate) dropped: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ExternalDraftReconcileOutcome {
    pub(crate) workspace_references: SourceReconcileOutcome,
    pub(crate) images: SourceReconcileOutcome,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum ComposerImageInsertError {
    #[error("A message can contain at most {MAX_COMPOSER_IMAGES} images")]
    TooManyImages,
    #[error("Images are unavailable in Shell mode")]
    ShellModeUnsupported,
    #[error(
        "Local draft image memory is full; remove or send an image from another session first"
    )]
    LocalDraftBudgetExceeded,
}

impl ComposerDraft {
    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    pub(crate) fn replace_text_from_external_editor(
        &mut self,
        text: String,
    ) -> ExternalDraftReconcileOutcome {
        let workspace_references =
            reconcile_external_sources(&mut self.workspace_references, &text);
        let images = reconcile_external_sources(&mut self.image_attachments, &text);
        self.text = text;
        self.relabel_images();
        ExternalDraftReconcileOutcome {
            workspace_references,
            images,
        }
    }

    pub(crate) fn reconcile_edit(
        &mut self,
        edit_start: usize,
        removed_chars: usize,
        inserted_chars: usize,
    ) {
        reconcile_source_edit(
            &mut self.workspace_references,
            edit_start,
            removed_chars,
            inserted_chars,
        );
        reconcile_source_edit(
            &mut self.image_attachments,
            edit_start,
            removed_chars,
            inserted_chars,
        );
    }

    pub(crate) fn retain_valid_sources(&mut self) {
        let chars = self.text.chars().collect::<Vec<_>>();
        retain_valid_sources(&mut self.workspace_references, &chars);
        retain_valid_sources(&mut self.image_attachments, &chars);
        self.relabel_images();
    }

    pub(crate) fn insert_image(
        &mut self,
        cursor: usize,
        image: ComposerImage,
    ) -> Result<usize, ComposerImageInsertError> {
        if self.image_attachments.len() >= MAX_COMPOSER_IMAGES {
            return Err(ComposerImageInsertError::TooManyImages);
        }
        let cursor = cursor.min(self.text.chars().count());
        let label = image_label(self.image_attachments.len() + 1);
        let inserted = format!("{label} ");
        let inserted_chars = inserted.chars().count();
        self.reconcile_edit(cursor, 0, inserted_chars);
        replace_char_range(&mut self.text, cursor, cursor, &inserted);
        self.image_attachments.push(ComposerImageAttachment {
            image,
            source: ComposerSourceRange {
                start: cursor,
                end: cursor + label.chars().count(),
                value: label,
            },
        });
        self.relabel_images();
        Ok(cursor + inserted_chars)
    }

    pub(crate) fn remove_image_overlapping_edit(
        &mut self,
        edit_start: usize,
        removed_chars: usize,
    ) -> Option<usize> {
        if removed_chars == 0 {
            return None;
        }
        let edit_end = edit_start.saturating_add(removed_chars);
        let source = self
            .image_attachments
            .iter()
            .find(|attachment| {
                edit_start < attachment.source.end && edit_end > attachment.source.start
            })?
            .source
            .clone();
        let chars = self.text.chars().collect::<Vec<_>>();
        let remove_end = if chars.get(source.end) == Some(&' ') {
            source.end + 1
        } else {
            source.end
        };
        self.reconcile_edit(source.start, remove_end - source.start, 0);
        replace_char_range(&mut self.text, source.start, remove_end, "");
        self.relabel_images();
        Some(source.start)
    }

    pub(crate) fn safe_insertion_cursor(&self, cursor: usize) -> usize {
        self.image_attachments
            .iter()
            .find(|attachment| attachment.source.start < cursor && cursor < attachment.source.end)
            .map_or(cursor, |attachment| attachment.source.end)
    }

    pub(crate) fn cursor_left(&self, cursor: usize) -> usize {
        let candidate = cursor.saturating_sub(1);
        self.image_attachments
            .iter()
            .find(|attachment| {
                attachment.source.start < candidate && candidate < attachment.source.end
            })
            .map_or(candidate, |attachment| attachment.source.start)
    }

    pub(crate) fn cursor_right(&self, cursor: usize) -> usize {
        let candidate = cursor.saturating_add(1).min(self.text.chars().count());
        self.image_attachments
            .iter()
            .find(|attachment| {
                attachment.source.start < candidate && candidate < attachment.source.end
            })
            .map_or(candidate, |attachment| attachment.source.end)
    }

    pub(crate) fn runtime_attachments(&self) -> Vec<AgentInputAttachment> {
        self.image_attachments
            .iter()
            .map(|attachment| attachment.image.to_runtime_attachment())
            .collect()
    }

    pub(crate) fn has_images(&self) -> bool {
        !self.image_attachments.is_empty()
    }

    pub(crate) fn image_byte_len(&self) -> usize {
        self.image_attachments
            .iter()
            .fold(0usize, |total, attachment| {
                total.saturating_add(attachment.image.bytes.len())
            })
    }

    pub(crate) fn drop_image_metadata(&mut self) {
        self.image_attachments.clear();
    }

    fn relabel_images(&mut self) {
        self.image_attachments
            .sort_by_key(|attachment| attachment.source.start);
        for (index, attachment) in self.image_attachments.iter_mut().enumerate() {
            let label = image_label(index + 1);
            debug_assert_eq!(
                label.chars().count(),
                attachment.source.value.chars().count(),
                "the five-image composer limit keeps labels the same width"
            );
            replace_char_range(
                &mut self.text,
                attachment.source.start,
                attachment.source.end,
                &label,
            );
            attachment.source.end = attachment.source.start + label.chars().count();
            attachment.source.value = label;
        }
    }
}

fn image_label(index: usize) -> String {
    format!("[Image {index}]")
}

trait TrackedComposerSource {
    fn start(&self) -> usize;
    fn end(&self) -> usize;
    fn value(&self) -> &str;
    fn set_range(&mut self, start: usize, end: usize);
    fn boundary_character(character: char) -> bool;
}

impl TrackedComposerSource for AgentWorkspaceReference {
    fn start(&self) -> usize {
        self.source.start
    }

    fn end(&self) -> usize {
        self.source.end
    }

    fn value(&self) -> &str {
        &self.source.value
    }

    fn set_range(&mut self, start: usize, end: usize) {
        self.source.start = start;
        self.source.end = end;
    }

    fn boundary_character(character: char) -> bool {
        is_workspace_reference_token_char(character)
    }
}

impl TrackedComposerSource for ComposerImageAttachment {
    fn start(&self) -> usize {
        self.source.start
    }

    fn end(&self) -> usize {
        self.source.end
    }

    fn value(&self) -> &str {
        &self.source.value
    }

    fn set_range(&mut self, start: usize, end: usize) {
        self.source.start = start;
        self.source.end = end;
    }

    fn boundary_character(_character: char) -> bool {
        false
    }
}

fn reconcile_source_edit<T: TrackedComposerSource>(
    sources: &mut Vec<T>,
    edit_start: usize,
    removed_chars: usize,
    inserted_chars: usize,
) {
    let edit_end = edit_start.saturating_add(removed_chars);
    let delta = inserted_chars as isize - removed_chars as isize;
    sources.retain_mut(|source| {
        if edit_end <= source.start() {
            source.set_range(
                source.start().saturating_add_signed(delta),
                source.end().saturating_add_signed(delta),
            );
            true
        } else if edit_start >= source.end() {
            true
        } else {
            false
        }
    });
}

fn retain_valid_sources<T: TrackedComposerSource>(sources: &mut Vec<T>, chars: &[char]) {
    sources.retain(|source| {
        let start = source.start();
        let end = source.end();
        start < end
            && end <= chars.len()
            && (start == 0 || !T::boundary_character(chars[start - 1]))
            && (end == chars.len() || !T::boundary_character(chars[end]))
            && chars[start..end].iter().collect::<String>() == source.value()
    });
}

fn reconcile_external_sources<T: TrackedComposerSource>(
    sources: &mut Vec<T>,
    text: &str,
) -> SourceReconcileOutcome {
    let mut groups = std::collections::BTreeMap::<String, Vec<usize>>::new();
    for (index, source) in sources.iter().enumerate() {
        groups
            .entry(source.value().to_string())
            .or_default()
            .push(index);
    }
    let mut assignments = vec![None; sources.len()];
    for (value, indices) in groups {
        let occurrences = source_occurrences::<T>(text, &value);
        if occurrences.len() == indices.len() {
            for (index, start) in indices.into_iter().zip(occurrences) {
                assignments[index] = Some(start);
            }
        }
    }

    let original_count = sources.len();
    let mut source_index = 0usize;
    sources.retain_mut(|source| {
        let assignment = assignments[source_index];
        source_index += 1;
        if let Some(start) = assignment {
            source.set_range(start, start + source.value().chars().count());
            true
        } else {
            false
        }
    });
    SourceReconcileOutcome {
        retained: sources.len(),
        dropped: original_count.saturating_sub(sources.len()),
    }
}

fn is_workspace_reference_token_char(character: char) -> bool {
    character.is_alphanumeric()
        || matches!(character, '_' | '-' | '.' | '/' | '\\' | ':' | '#' | '@')
}

fn source_occurrences<T: TrackedComposerSource>(text: &str, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }
    let text = text.chars().collect::<Vec<_>>();
    let needle = needle.chars().collect::<Vec<_>>();
    if needle.len() > text.len() {
        return Vec::new();
    }
    (0..=text.len() - needle.len())
        .filter(|start| {
            let end = start + needle.len();
            text[*start..end] == needle
                && (*start == 0 || !T::boundary_character(text[*start - 1]))
                && (end == text.len() || !T::boundary_character(text[end]))
        })
        .collect()
}

fn replace_char_range(text: &mut String, start: usize, end: usize, replacement: &str) {
    let char_count = text.chars().count();
    let start = start.min(char_count);
    let end = end.clamp(start, char_count);
    let start_byte = text
        .char_indices()
        .nth(start)
        .map_or(text.len(), |(offset, _)| offset);
    let end_byte = text
        .char_indices()
        .nth(end)
        .map_or(text.len(), |(offset, _)| offset);
    text.replace_range(start_byte..end_byte, replacement);
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_agent_runtime::sdk::{
        AgentWorkspaceReferenceKind, AgentWorkspaceReferenceSourceRange,
    };
    use std::sync::Arc;

    fn reference(start: usize) -> AgentWorkspaceReference {
        AgentWorkspaceReference {
            path: "src/lib.rs".to_string(),
            kind: AgentWorkspaceReferenceKind::File,
            start_line: None,
            end_line: None,
            source: AgentWorkspaceReferenceSourceRange {
                start,
                end: start + 11,
                value: "@src/lib.rs".to_string(),
            },
        }
    }

    fn image(id: &str) -> ComposerImage {
        ComposerImage::new(
            id,
            format!("{id}.png"),
            "image/png",
            Arc::<[u8]>::from([1, 2, 3]),
        )
    }

    #[test]
    fn inserting_an_image_shifts_workspace_references_through_one_reconciler() {
        let mut draft = ComposerDraft {
            text: "@src/lib.rs".to_string(),
            workspace_references: vec![reference(0)],
            ..ComposerDraft::default()
        };

        let cursor = draft.insert_image(0, image("first")).unwrap();

        assert_eq!(cursor, 10);
        assert_eq!(draft.text, "[Image 1] @src/lib.rs");
        assert_eq!(draft.image_attachments[0].source.start, 0);
        assert_eq!(draft.image_attachments[0].source.end, 9);
        assert_eq!(draft.workspace_references[0].source.start, 10);
    }

    #[test]
    fn deleting_any_part_of_an_image_token_removes_it_and_relabels_the_rest() {
        let mut draft = ComposerDraft::default();
        let cursor = draft.insert_image(0, image("first")).unwrap();
        draft.insert_image(cursor, image("second")).unwrap();

        let cursor = draft.remove_image_overlapping_edit(1, 1).unwrap();

        assert_eq!(cursor, 0);
        assert_eq!(draft.text, "[Image 1] ");
        assert_eq!(draft.image_attachments.len(), 1);
        assert_eq!(draft.image_attachments[0].image.id, "second");
        assert_eq!(draft.image_attachments[0].source.value, "[Image 1]");
    }

    #[test]
    fn external_editor_drops_missing_images_and_relabels_source_order() {
        let mut draft = ComposerDraft::default();
        let cursor = draft.insert_image(0, image("first")).unwrap();
        draft.insert_image(cursor, image("second")).unwrap();

        let outcome = draft.replace_text_from_external_editor("[Image 2] only".to_string());

        assert_eq!(outcome.images.retained, 1);
        assert_eq!(outcome.images.dropped, 1);
        assert_eq!(draft.text, "[Image 1] only");
        assert_eq!(draft.image_attachments[0].image.id, "second");
        assert_eq!(draft.image_attachments[0].source.start, 0);
    }

    #[test]
    fn edits_before_sources_shift_images_and_workspace_references_together() {
        let mut draft = ComposerDraft {
            text: "[Image 1] @src/lib.rs".to_string(),
            workspace_references: vec![reference(10)],
            image_attachments: vec![ComposerImageAttachment {
                image: image("first"),
                source: ComposerSourceRange {
                    start: 0,
                    end: 9,
                    value: "[Image 1]".to_string(),
                },
            }],
        };

        draft.reconcile_edit(0, 0, 3);

        assert_eq!(draft.image_attachments[0].source.start, 3);
        assert_eq!(draft.workspace_references[0].source.start, 13);
    }

    #[test]
    fn runtime_projection_uses_the_existing_remote_image_contract() {
        let mut draft = ComposerDraft::default();
        draft.insert_image(0, image("first")).unwrap();

        let attachments = draft.runtime_attachments();

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].kind, "remote_image");
        assert_eq!(attachments[0].id, "first");
        assert_eq!(attachments[0].metadata["name"], "first.png");
        assert_eq!(
            attachments[0].metadata["dataUrl"],
            "data:image/png;base64,AQID"
        );
        assert!(!attachments[0].metadata.contains_key("imagePath"));
    }

    #[test]
    fn sixth_image_is_rejected_without_mutating_the_draft() {
        let mut draft = ComposerDraft::default();
        let mut cursor = 0;
        for index in 0..MAX_COMPOSER_IMAGES {
            cursor = draft
                .insert_image(cursor, image(&format!("image-{index}")))
                .unwrap();
        }
        let before = draft.clone();

        let error = draft.insert_image(cursor, image("overflow")).unwrap_err();

        assert_eq!(error, ComposerImageInsertError::TooManyImages);
        assert_eq!(draft, before);
    }

    #[test]
    fn edits_before_mentions_shift_ranges_and_overlaps_invalidate_them() {
        let mut draft = ComposerDraft {
            text: "see @src/lib.rs".to_string(),
            workspace_references: vec![reference(4)],
            ..ComposerDraft::default()
        };
        draft.reconcile_edit(0, 0, 2);
        assert_eq!(draft.workspace_references[0].source.start, 6);
        draft.reconcile_edit(8, 1, 0);
        assert!(draft.workspace_references.is_empty());
    }

    #[test]
    fn token_boundary_edits_invalidate_structured_references() {
        let reference = reference(4);
        let mut right = ComposerDraft {
            text: "see @src/lib.rsx".to_string(),
            workspace_references: vec![reference.clone()],
            ..ComposerDraft::default()
        };
        right.retain_valid_sources();
        assert!(right.workspace_references.is_empty());

        let mut left = ComposerDraft {
            text: "x@src/lib.rs".to_string(),
            workspace_references: vec![AgentWorkspaceReference {
                source: AgentWorkspaceReferenceSourceRange {
                    start: 1,
                    end: 12,
                    value: "@src/lib.rs".to_string(),
                },
                ..reference
            }],
            ..ComposerDraft::default()
        };
        left.retain_valid_sources();
        assert!(left.workspace_references.is_empty());
    }

    #[test]
    fn external_edit_repositions_a_unique_workspace_reference() {
        let mut draft = ComposerDraft {
            text: "Review @src/lib.rs before editing".to_string(),
            workspace_references: vec![reference(7)],
            ..ComposerDraft::default()
        };

        let outcome = draft.replace_text_from_external_editor(
            "Please carefully review @src/lib.rs before editing".to_string(),
        );

        assert_eq!(outcome.workspace_references.retained, 1);
        assert_eq!(outcome.workspace_references.dropped, 0);
        assert_eq!(draft.workspace_references[0].source.start, 24);
        assert_eq!(draft.workspace_references[0].source.end, 35);
        assert_eq!(
            draft.text,
            "Please carefully review @src/lib.rs before editing"
        );
    }

    #[test]
    fn external_edit_drops_an_ambiguous_workspace_reference() {
        let mut draft = ComposerDraft {
            text: "Review @src/lib.rs".to_string(),
            workspace_references: vec![reference(7)],
            ..ComposerDraft::default()
        };

        let outcome = draft
            .replace_text_from_external_editor("Compare @src/lib.rs with @src/lib.rs".to_string());

        assert_eq!(outcome.workspace_references.retained, 0);
        assert_eq!(outcome.workspace_references.dropped, 1);
        assert!(draft.workspace_references.is_empty());
        assert_eq!(draft.text, "Compare @src/lib.rs with @src/lib.rs");
    }

    #[test]
    fn external_edit_repositions_equal_reference_groups_in_source_order() {
        let mut draft = ComposerDraft {
            text: "@src/lib.rs and @src/lib.rs".to_string(),
            workspace_references: vec![reference(0), reference(16)],
            ..ComposerDraft::default()
        };

        let outcome = draft
            .replace_text_from_external_editor("Compare @src/lib.rs with @src/lib.rs".to_string());

        assert_eq!(outcome.workspace_references.retained, 2);
        assert_eq!(outcome.workspace_references.dropped, 0);
        assert_eq!(draft.workspace_references[0].source.start, 8);
        assert_eq!(draft.workspace_references[1].source.start, 25);
    }

    #[test]
    fn external_edit_preserves_a_reference_next_to_punctuation() {
        let mut draft = ComposerDraft {
            text: "Review @src/lib.rs".to_string(),
            workspace_references: vec![reference(7)],
            ..ComposerDraft::default()
        };

        let outcome = draft
            .replace_text_from_external_editor("Review (@src/lib.rs), then continue.".to_string());

        assert_eq!(outcome.workspace_references.retained, 1);
        assert_eq!(outcome.workspace_references.dropped, 0);
        assert_eq!(draft.workspace_references[0].source.start, 8);
    }

    #[test]
    fn external_edit_does_not_match_a_longer_reference_token() {
        let mut draft = ComposerDraft {
            text: "Review @src/lib.rs".to_string(),
            workspace_references: vec![reference(7)],
            ..ComposerDraft::default()
        };

        let outcome = draft.replace_text_from_external_editor("Review @src/lib.rs.bak".to_string());

        assert_eq!(outcome.workspace_references.retained, 0);
        assert_eq!(outcome.workspace_references.dropped, 1);
    }
}
