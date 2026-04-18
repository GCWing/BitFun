//! Legacy → ControlHub action migration table.
//!
//! When `SelfControlTool` and `ComputerUseTool` were down-registered (so the
//! model only ever sees the unified `ControlHub` tool), every previously
//! exposed action had to be reachable through some `(domain, action)` pair.
//! This file is the source of truth: each entry maps the *old* tool action
//! name to its new ControlHub address.
//!
//! Two consumers use this table:
//!  1. The unit test [`tests::all_legacy_actions_are_reachable`] which asserts
//!     that no legacy action accidentally falls off the matrix after a refactor.
//!  2. ControlHub's own description renderer, which lists "if you used to call
//!     X, call ControlHub Y instead" hints to the model.
//!
//! Adding a new ControlHub action does NOT require touching this table; only
//! when retiring or renaming an action that already shipped to the model.

/// Where the legacy action came from (which previously-registered tool).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LegacyTool {
    /// Operations from the old `SelfControl` tool — controls BitFun's own GUI.
    SelfControl,
    /// Operations from the old `ComputerUse` tool — desktop automation.
    ComputerUse,
    /// `ComputerUseMouseStep` (split mouse tool, never registered globally but
    /// referenced by some prompt fragments).
    ComputerUseMouseStep,
    /// `ComputerUseMouseClick` (split mouse tool, never registered globally).
    ComputerUseMouseClick,
    /// `ComputerUseMousePrecise` (split mouse tool, never registered globally).
    ComputerUseMousePrecise,
}

#[derive(Debug, Clone, Copy)]
pub struct LegacyMapping {
    pub from: LegacyTool,
    pub legacy_action: &'static str,
    pub new_domain: &'static str,
    pub new_action: &'static str,
}

/// The complete mapping. Sorted by (from, legacy_action) for readability.
pub const LEGACY_ACTION_MIGRATIONS: &[LegacyMapping] = &[
    // ── SelfControl → app.* ─────────────────────────────────────────────
    m(LegacyTool::SelfControl, "click", "app", "click"),
    m(LegacyTool::SelfControl, "click_by_text", "app", "click_by_text"),
    m(LegacyTool::SelfControl, "delete_model", "app", "delete_model"),
    m(LegacyTool::SelfControl, "execute_task", "app", "execute_task"),
    m(LegacyTool::SelfControl, "get_config", "app", "get_config"),
    m(LegacyTool::SelfControl, "get_page_state", "app", "get_page_state"),
    m(LegacyTool::SelfControl, "input", "app", "input"),
    m(LegacyTool::SelfControl, "list_models", "app", "list_models"),
    m(LegacyTool::SelfControl, "open_scene", "app", "open_scene"),
    m(LegacyTool::SelfControl, "open_settings_tab", "app", "open_settings_tab"),
    m(LegacyTool::SelfControl, "press_key", "app", "press_key"),
    m(LegacyTool::SelfControl, "read_text", "app", "read_text"),
    m(LegacyTool::SelfControl, "scroll", "app", "scroll"),
    m(LegacyTool::SelfControl, "select_option", "app", "select_option"),
    m(LegacyTool::SelfControl, "set_config", "app", "set_config"),
    m(LegacyTool::SelfControl, "set_default_model", "app", "set_default_model"),
    m(LegacyTool::SelfControl, "wait", "app", "wait"),
    // ── ComputerUse → desktop.* ────────────────────────────────────────
    m(LegacyTool::ComputerUse, "click", "desktop", "click"),
    m(LegacyTool::ComputerUse, "click_element", "desktop", "click_element"),
    m(LegacyTool::ComputerUse, "drag", "desktop", "drag"),
    m(LegacyTool::ComputerUse, "key_chord", "desktop", "key_chord"),
    m(LegacyTool::ComputerUse, "locate", "desktop", "locate"),
    m(LegacyTool::ComputerUse, "mouse_move", "desktop", "mouse_move"),
    m(LegacyTool::ComputerUse, "move_to_text", "desktop", "move_to_text"),
    m(LegacyTool::ComputerUse, "open_app", "system", "open_app"),
    m(LegacyTool::ComputerUse, "pointer_move_rel", "desktop", "pointer_move_rel"),
    m(LegacyTool::ComputerUse, "run_apple_script", "system", "run_script"),
    m(LegacyTool::ComputerUse, "screenshot", "desktop", "screenshot"),
    m(LegacyTool::ComputerUse, "scroll", "desktop", "scroll"),
    m(LegacyTool::ComputerUse, "type_text", "desktop", "type_text"),
    m(LegacyTool::ComputerUse, "wait", "desktop", "wait"),
    // ── Split mouse helper tools → desktop.* ───────────────────────────
    m(LegacyTool::ComputerUseMouseStep, "step", "desktop", "mouse_step"),
    m(LegacyTool::ComputerUseMouseClick, "click", "desktop", "mouse_click"),
    m(LegacyTool::ComputerUseMouseClick, "wheel", "desktop", "scroll"),
    m(LegacyTool::ComputerUseMousePrecise, "absolute", "desktop", "mouse_precise"),
];

const fn m(
    from: LegacyTool,
    legacy_action: &'static str,
    new_domain: &'static str,
    new_action: &'static str,
) -> LegacyMapping {
    LegacyMapping {
        from,
        legacy_action,
        new_domain,
        new_action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every (legacy tool, legacy action) pair must be unique.
    #[test]
    fn legacy_actions_are_unique_per_tool() {
        let mut seen: HashSet<(LegacyTool, &'static str)> = HashSet::new();
        for mapping in LEGACY_ACTION_MIGRATIONS {
            assert!(
                seen.insert((mapping.from, mapping.legacy_action)),
                "duplicate legacy mapping for {:?}::{}",
                mapping.from,
                mapping.legacy_action
            );
        }
    }

    /// Sanity: every legacy SelfControl action listed in self_control_tool's
    /// JSON schema enum must appear here. Keeps the safety net honest when
    /// the schema is extended without updating the migration table.
    #[test]
    fn all_legacy_self_control_actions_are_mapped() {
        let expected = [
            "execute_task",
            "get_page_state",
            "click",
            "click_by_text",
            "input",
            "scroll",
            "open_scene",
            "open_settings_tab",
            "set_config",
            "get_config",
            "list_models",
            "set_default_model",
            "select_option",
            "wait",
            "press_key",
            "read_text",
            "delete_model",
        ];
        for action in expected {
            assert!(
                LEGACY_ACTION_MIGRATIONS
                    .iter()
                    .any(|m| m.from == LegacyTool::SelfControl && m.legacy_action == action),
                "SelfControl action `{}` is not present in LEGACY_ACTION_MIGRATIONS",
                action
            );
        }
    }

    /// Sanity: the public ComputerUse actions advertised in the tool description
    /// must all be reachable through ControlHub.
    #[test]
    fn all_legacy_computer_use_actions_are_mapped() {
        let expected = [
            "screenshot",
            "click",
            "click_element",
            "mouse_move",
            "scroll",
            "drag",
            "key_chord",
            "type_text",
            "wait",
            "locate",
            "open_app",
            "run_apple_script",
            "move_to_text",
            "pointer_move_rel",
        ];
        for action in expected {
            assert!(
                LEGACY_ACTION_MIGRATIONS
                    .iter()
                    .any(|m| m.from == LegacyTool::ComputerUse && m.legacy_action == action),
                "ComputerUse action `{}` is not present in LEGACY_ACTION_MIGRATIONS",
                action
            );
        }
    }
}
