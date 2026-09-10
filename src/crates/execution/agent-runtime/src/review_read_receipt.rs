//! Session-scoped receipts for file ranges returned during code review.

use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileRevision {
    pub modified_ns: u128,
    pub byte_len: u64,
    pub content_sha256: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewReadCoverage {
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewReadReceipt {
    revision: FileRevision,
    ranges: Vec<(usize, usize)>,
    total_lines: usize,
}

#[derive(Default)]
pub struct ReviewReadReceiptStore {
    session_receipts: Arc<DashMap<String, DashMap<String, ReviewReadReceipt>>>,
}

impl ReviewReadReceiptStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&self, session_id: &str) {
        self.session_receipts
            .entry(session_id.to_string())
            .or_default();
    }

    pub fn delete_session(&self, session_id: &str) {
        self.session_receipts.remove(session_id);
    }

    pub fn clear_session(&self, session_id: &str) {
        if let Some(receipts) = self.session_receipts.get(session_id) {
            receipts.clear();
        }
    }

    pub fn record_review_read(
        &self,
        session_id: &str,
        logical_path: &str,
        revision: FileRevision,
        start_line: usize,
        end_line: usize,
        total_lines: usize,
    ) {
        if start_line == 0 || end_line < start_line {
            return;
        }

        let session_receipts = self
            .session_receipts
            .entry(session_id.to_string())
            .or_default();
        let mut receipt = session_receipts
            .entry(logical_path.to_string())
            .or_insert_with(|| ReviewReadReceipt {
                revision,
                ranges: Vec::new(),
                total_lines,
            });
        if receipt.revision != revision {
            receipt.revision = revision;
            receipt.ranges.clear();
        }
        receipt.total_lines = total_lines;
        receipt.ranges.push((start_line, end_line));
        receipt.ranges.sort_unstable_by_key(|range| range.0);

        let mut merged = Vec::<(usize, usize)>::with_capacity(receipt.ranges.len());
        for (start, end) in receipt.ranges.drain(..) {
            if let Some(last) = merged.last_mut() {
                if start <= last.1.saturating_add(1) {
                    last.1 = last.1.max(end);
                    continue;
                }
            }
            merged.push((start, end));
        }
        receipt.ranges = merged;
    }

    pub fn review_read_coverage(
        &self,
        session_id: &str,
        logical_path: &str,
        revision: FileRevision,
        start_line: usize,
        limit: usize,
    ) -> Option<ReviewReadCoverage> {
        if start_line == 0 || limit == 0 {
            return None;
        }
        let session_receipts = self.session_receipts.get(session_id)?;
        let receipt = session_receipts.get(logical_path)?;
        if receipt.revision != revision || start_line > receipt.total_lines {
            return None;
        }
        let end_line = start_line
            .saturating_add(limit.saturating_sub(1))
            .min(receipt.total_lines);
        receipt
            .ranges
            .iter()
            .any(|(covered_start, covered_end)| {
                *covered_start <= start_line && *covered_end >= end_line
            })
            .then_some(ReviewReadCoverage {
                start_line,
                end_line,
                total_lines: receipt.total_lines,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{FileRevision, ReviewReadCoverage, ReviewReadReceiptStore};

    fn revision(hash_byte: u8) -> FileRevision {
        FileRevision {
            modified_ns: 100,
            byte_len: 4096,
            content_sha256: [hash_byte; 32],
        }
    }

    #[test]
    fn review_read_receipt_store_scopes_entries_by_session() {
        let store = ReviewReadReceiptStore::new();
        let revision = revision(1);
        store.create_session("session-a");
        store.create_session("session-b");
        store.record_review_read("session-a", "src/lib.rs", revision, 1, 20, 20);

        assert!(store
            .review_read_coverage("session-a", "src/lib.rs", revision, 1, 20)
            .is_some());
        assert!(store
            .review_read_coverage("session-b", "src/lib.rs", revision, 1, 20)
            .is_none());
    }

    #[test]
    fn review_read_receipt_covers_only_previously_returned_lines() {
        let store = ReviewReadReceiptStore::new();
        let revision = revision(1);
        store.record_review_read("review-session", "src/large.rs", revision, 1, 2000, 3000);

        assert_eq!(
            store.review_read_coverage("review-session", "src/large.rs", revision, 1403, 27),
            Some(ReviewReadCoverage {
                start_line: 1403,
                end_line: 1429,
                total_lines: 3000,
            })
        );
        assert!(store
            .review_read_coverage("review-session", "src/large.rs", revision, 2001, 20)
            .is_none());
    }

    #[test]
    fn review_read_receipt_merges_ranges_and_invalidates_on_revision_change() {
        let store = ReviewReadReceiptStore::new();
        let original = revision(1);
        store.record_review_read("review-session", "src/lib.rs", original, 1, 100, 300);
        store.record_review_read("review-session", "src/lib.rs", original, 101, 200, 300);

        assert!(store
            .review_read_coverage("review-session", "src/lib.rs", original, 50, 151)
            .is_some());
        assert!(store
            .review_read_coverage("review-session", "src/lib.rs", revision(2), 50, 151)
            .is_none());
    }

    #[test]
    fn review_read_receipt_store_clear_and_delete_drop_session_entries() {
        let store = ReviewReadReceiptStore::new();
        let revision = revision(1);
        store.record_review_read("session-a", "src/lib.rs", revision, 1, 20, 20);
        store.record_review_read("session-b", "src/lib.rs", revision, 1, 20, 20);

        store.clear_session("session-a");
        assert!(store
            .review_read_coverage("session-a", "src/lib.rs", revision, 1, 20)
            .is_none());
        assert!(store
            .review_read_coverage("session-b", "src/lib.rs", revision, 1, 20)
            .is_some());

        store.delete_session("session-b");
        assert!(store
            .review_read_coverage("session-b", "src/lib.rs", revision, 1, 20)
            .is_none());
    }
}
