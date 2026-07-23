//! Provider-neutral skill contracts and runtime decisions.
//!
//! This module owns skill DTOs, built-in catalog facts, mode default policy,
//! override resolution, markdown parsing, and assistant-visible payload
//! rendering. Product hosts still own filesystem/config IO and registry
//! scanning.

mod catalog;
mod keys;
mod policy;
mod resolver;
mod roots;
mod selection;
mod types;

pub use catalog::builtin_skill_group_key;
pub use policy::resolve_builtin_default_enabled;
pub use resolver::{
    normalize_user_mode_skill_overrides, resolve_skill_default_enabled_for_mode,
    resolve_skill_state_for_mode, ModeSkillState, UserModeSkillOverrides,
};
pub use roots::{
    normalize_local_skill_dir_name, normalize_remote_skill_dir_name,
    resolve_user_config_skill_root, SkillRootSpec, BITFUN_SKILL_SOURCE_ID,
    BITFUN_SKILL_SOURCE_LABEL, BITFUN_SYSTEM_SKILL_DIR, BITFUN_SYSTEM_SKILL_SLOT,
    BITFUN_USER_SKILL_SLOT, PROJECT_SKILL_KEY_PREFIX, PROJECT_SKILL_ROOTS, USER_CONFIG_SKILL_ROOTS,
    USER_HOME_SKILL_ROOTS, USER_SKILL_KEY_PREFIX,
};
pub use selection::{
    annotate_shadowed_skills, build_mode_skill_infos, filter_candidates_for_mode,
    normalize_skill_keys, resolve_default_hidden_builtin_for_explicit_invocation,
    resolve_visible_skills, sort_skill_candidates_by_dir, sort_skills,
    ExplicitSkillInvocationResolution, SkillCandidate,
};
pub use types::{
    render_loaded_skill_for_assistant, ModeSkillInfo, ModeSkillStateReason, SkillData, SkillInfo,
    SkillLocation, SkillParseError,
};
