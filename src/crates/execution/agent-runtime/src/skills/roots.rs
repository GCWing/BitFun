use std::path::{Path, PathBuf};

use openbitfun_core_types::product_identity::hidden_data_directory;

use super::types::SkillSourceDialect;

pub const USER_SKILL_KEY_PREFIX: &str = "user";
pub const PROJECT_SKILL_KEY_PREFIX: &str = "project";
pub const OPENBITFUN_USER_SKILL_SLOT: &str = "openbitfun";
pub const OPENBITFUN_SYSTEM_SKILL_SLOT: &str = "openbitfun-system";
pub const OPENBITFUN_SYSTEM_SKILL_DIR: &str = ".system";
pub const OPENBITFUN_SKILL_SOURCE_ID: &str = "openbitfun";
pub const OPENBITFUN_SKILL_SOURCE_LABEL: &str = "OpenBitFun";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillRootSpec {
    pub parent: &'static str,
    pub subdir: &'static str,
    pub slot: &'static str,
    /// Provider-neutral ecosystem identity. It is presentation metadata only
    /// and must never participate in skill precedence.
    pub source_id: &'static str,
    pub source_label: &'static str,
}

pub const PROJECT_SKILL_ROOTS: &[SkillRootSpec] = &[
    SkillRootSpec {
        parent: hidden_data_directory(),
        subdir: "skills",
        slot: "openbitfun",
        source_id: OPENBITFUN_SKILL_SOURCE_ID,
        source_label: OPENBITFUN_SKILL_SOURCE_LABEL,
    },
    SkillRootSpec {
        parent: ".claude",
        subdir: "skills",
        slot: "claude",
        source_id: "claude-code",
        source_label: "Claude Code",
    },
    SkillRootSpec {
        parent: ".codex",
        subdir: "skills",
        slot: "codex",
        source_id: "codex",
        source_label: "Codex",
    },
    SkillRootSpec {
        parent: ".cursor",
        subdir: "skills",
        slot: "cursor",
        source_id: "cursor",
        source_label: "Cursor",
    },
    SkillRootSpec {
        parent: ".opencode",
        subdir: "skills",
        slot: "opencode",
        source_id: "opencode",
        source_label: "OpenCode",
    },
    SkillRootSpec {
        parent: ".agents",
        subdir: "skills",
        slot: "agents",
        source_id: "agent-skills",
        source_label: "Agent Skills",
    },
];

pub const USER_HOME_SKILL_ROOTS: &[SkillRootSpec] = &[
    SkillRootSpec {
        parent: ".claude",
        subdir: "skills",
        slot: "home.claude",
        source_id: "claude-code",
        source_label: "Claude Code",
    },
    SkillRootSpec {
        parent: ".codex",
        subdir: "skills",
        slot: "home.codex",
        source_id: "codex",
        source_label: "Codex",
    },
    SkillRootSpec {
        parent: ".cursor",
        subdir: "skills",
        slot: "home.cursor",
        source_id: "cursor",
        source_label: "Cursor",
    },
    SkillRootSpec {
        parent: ".opencode",
        subdir: "skills",
        slot: "home.opencode",
        source_id: "opencode",
        source_label: "OpenCode",
    },
    SkillRootSpec {
        parent: ".agents",
        subdir: "skills",
        slot: "home.agents",
        source_id: "agent-skills",
        source_label: "Agent Skills",
    },
];

pub const USER_CONFIG_SKILL_ROOTS: &[SkillRootSpec] = &[SkillRootSpec {
    parent: "opencode",
    subdir: "skills",
    slot: "config.opencode",
    source_id: "opencode",
    source_label: "OpenCode",
}];

pub(crate) fn skill_source_dialect(source_slot: &str) -> SkillSourceDialect {
    match source_slot {
        "claude" | "home.claude" => SkillSourceDialect::ClaudeCode,
        "codex" | "home.codex" => SkillSourceDialect::Codex,
        _ => SkillSourceDialect::AgentSkills,
    }
}

pub fn resolve_user_config_skill_root(
    spec: &SkillRootSpec,
    config_dir: &Path,
    home_dir: Option<&Path>,
) -> PathBuf {
    if cfg!(target_os = "windows") && spec.parent == "opencode" {
        if let Some(home_dir) = home_dir {
            return home_dir.join(".config").join(spec.parent).join(spec.subdir);
        }
    }

    config_dir.join(spec.parent).join(spec.subdir)
}

pub fn normalize_local_skill_dir_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn normalize_remote_skill_dir_name(path: &str) -> Option<String> {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}
