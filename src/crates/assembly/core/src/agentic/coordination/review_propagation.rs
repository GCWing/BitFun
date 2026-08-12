//! Review propagation along the conversation tree - basic version
//!
//! When a leaf agent completes, review results propagate upward along the parent_session_id chain.

use log::{debug, info};

pub struct ReviewPropagationManager;

/// Review propagation action
pub enum ReviewPropagationAction {
    /// No action needed
    None,
    /// Suggest triggering a review of the parent session
    ReviewNeeded {
        parent_session_id: String,
        child_session_id: String,
    },
}

impl ReviewPropagationManager {
    /// Triggered when a leaf agent completes - checks the parent session and decides whether to propagate a review
    pub fn on_leaf_completed(
        session_id: &str,
        agent_type: &str,
        response_text: &str,
        parent_session_id: Option<&str>,
    ) -> ReviewPropagationAction {
        info!(
            "ReviewPropagation: leaf agent completed session={} agent_type={} text_len={} parent={:?}",
            session_id,
            agent_type,
            response_text.len(),
            parent_session_id,
        );

        match parent_session_id {
            Some(parent_id) if !parent_id.is_empty() => {
                debug!(
                    "ReviewPropagation: review may be needed for parent session={} (child={} completed)",
                    parent_id, session_id
                );
                ReviewPropagationAction::ReviewNeeded {
                    parent_session_id: parent_id.to_string(),
                    child_session_id: session_id.to_string(),
                }
            }
            _ => ReviewPropagationAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_leaf_completed_with_parent_suggests_review() {
        let action = ReviewPropagationManager::on_leaf_completed(
            "child-1",
            "GeneralPurpose",
            "done",
            Some("parent-1"),
        );
        match action {
            ReviewPropagationAction::ReviewNeeded {
                parent_session_id,
                child_session_id,
            } => {
                assert_eq!(parent_session_id, "parent-1");
                assert_eq!(child_session_id, "child-1");
            }
            ReviewPropagationAction::None => panic!("expected ReviewNeeded"),
        }
    }

    #[test]
    fn on_leaf_completed_without_parent_returns_none() {
        let action = ReviewPropagationManager::on_leaf_completed(
            "child-1",
            "GeneralPurpose",
            "done",
            None,
        );
        assert!(matches!(action, ReviewPropagationAction::None));

        let empty_parent = ReviewPropagationManager::on_leaf_completed(
            "child-1",
            "GeneralPurpose",
            "done",
            Some(""),
        );
        assert!(matches!(empty_parent, ReviewPropagationAction::None));
    }
}
