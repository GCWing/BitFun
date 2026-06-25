use std::path::Path;

pub const USER_SKILL_KEY_PREFIX: &str = "user";
pub const PROJECT_SKILL_KEY_PREFIX: &str = "project";
pub const BITFUN_USER_SKILL_SLOT: &str = "bitfun";
pub const BITFUN_SYSTEM_SKILL_SLOT: &str = "bitfun-system";
pub const BITFUN_SYSTEM_SKILL_DIR: &str = ".system";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillRootSpec {
    pub parent: &'static str,
    pub subdir: &'static str,
    pub slot: &'static str,
}

pub const PROJECT_SKILL_ROOTS: &[SkillRootSpec] = &[
    SkillRootSpec {
        parent: ".bitfun",
        subdir: "skills",
        slot: "bitfun",
    },
    SkillRootSpec {
        parent: ".claude",
        subdir: "skills",
        slot: "claude",
    },
    SkillRootSpec {
        parent: ".codex",
        subdir: "skills",
        slot: "codex",
    },
    SkillRootSpec {
        parent: ".cursor",
        subdir: "skills",
        slot: "cursor",
    },
    SkillRootSpec {
        parent: ".opencode",
        subdir: "skills",
        slot: "opencode",
    },
    SkillRootSpec {
        parent: ".agents",
        subdir: "skills",
        slot: "agents",
    },
];

pub const USER_HOME_SKILL_ROOTS: &[SkillRootSpec] = &[
    SkillRootSpec {
        parent: ".claude",
        subdir: "skills",
        slot: "home.claude",
    },
    SkillRootSpec {
        parent: ".codex",
        subdir: "skills",
        slot: "home.codex",
    },
    SkillRootSpec {
        parent: ".cursor",
        subdir: "skills",
        slot: "home.cursor",
    },
    SkillRootSpec {
        parent: ".agents",
        subdir: "skills",
        slot: "home.agents",
    },
];

pub const USER_CONFIG_SKILL_ROOTS: &[SkillRootSpec] = &[
    SkillRootSpec {
        parent: "opencode",
        subdir: "skills",
        slot: "config.opencode",
    },
    SkillRootSpec {
        parent: "agents",
        subdir: "skills",
        slot: "config.agents",
    },
];

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
