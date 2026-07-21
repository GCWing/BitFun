use std::collections::HashSet;
use std::path::PathBuf;

use bitfun_agent_runtime::skills::{
    annotate_shadowed_skills, build_mode_skill_infos, builtin_skill_group_key,
    filter_candidates_for_mode, render_loaded_skill_for_assistant, resolve_builtin_default_enabled,
    resolve_default_hidden_builtin_for_explicit_invocation, resolve_skill_default_enabled_for_mode,
    resolve_skill_state_for_mode, resolve_user_config_skill_root, resolve_visible_skills,
    sort_skills, ExplicitSkillInvocationResolution, ModeSkillStateReason, SkillCandidate,
    SkillData, SkillInfo, SkillLocation, UserModeSkillOverrides, BITFUN_SYSTEM_SKILL_DIR,
    BITFUN_SYSTEM_SKILL_SLOT, BITFUN_USER_SKILL_SLOT, PROJECT_SKILL_KEY_PREFIX,
    PROJECT_SKILL_ROOTS, USER_CONFIG_SKILL_ROOTS, USER_HOME_SKILL_ROOTS, USER_SKILL_KEY_PREFIX,
};

fn builtin_skill(dir_name: &str) -> SkillInfo {
    SkillInfo {
        key: format!("user::bitfun-system::{}", dir_name),
        name: dir_name.to_string(),
        description: String::new(),
        path: format!("/tmp/{}", dir_name),
        level: SkillLocation::User,
        source_slot: "bitfun-system".to_string(),
        source_id: "bitfun".to_string(),
        source_label: "BitFun".to_string(),
        dir_name: dir_name.to_string(),
        is_builtin: true,
        group_key: builtin_skill_group_key(dir_name).map(str::to_string),
        is_shadowed: false,
        shadowed_by_key: None,
    }
}

fn custom_user_skill(dir_name: &str) -> SkillInfo {
    SkillInfo {
        key: format!("user::bitfun::{}", dir_name),
        name: dir_name.to_string(),
        description: String::new(),
        path: format!("/tmp/{}", dir_name),
        level: SkillLocation::User,
        source_slot: "bitfun".to_string(),
        source_id: "bitfun".to_string(),
        source_label: "BitFun".to_string(),
        dir_name: dir_name.to_string(),
        is_builtin: false,
        group_key: None,
        is_shadowed: false,
        shadowed_by_key: None,
    }
}

fn project_skill(dir_name: &str) -> SkillInfo {
    SkillInfo {
        key: format!("project::bitfun::{}", dir_name),
        name: dir_name.to_string(),
        description: String::new(),
        path: format!("/workspace/.bitfun/skills/{}", dir_name),
        level: SkillLocation::Project,
        source_slot: "bitfun".to_string(),
        source_id: "bitfun".to_string(),
        source_label: "BitFun".to_string(),
        dir_name: dir_name.to_string(),
        is_builtin: false,
        group_key: None,
        is_shadowed: false,
        shadowed_by_key: None,
    }
}

#[test]
fn builtin_skill_catalog_and_mode_policy_are_runtime_owned() {
    assert_eq!(builtin_skill_group_key("docx"), Some("office"));
    assert_eq!(builtin_skill_group_key("find-skills"), Some("meta"));
    assert_eq!(builtin_skill_group_key("miniapp-dev"), Some("miniapp"));
    assert_eq!(
        builtin_skill_group_key("agent-browser"),
        Some("computer-use")
    );
    assert_eq!(builtin_skill_group_key("agent-eval-canvas"), Some("canvas"));
    assert_eq!(builtin_skill_group_key("bitfun-canvas"), Some("canvas"));
    assert_eq!(builtin_skill_group_key("pr-review-canvas"), Some("canvas"));
    assert_eq!(builtin_skill_group_key("docs-canvas"), Some("canvas"));
    assert_eq!(builtin_skill_group_key("gstack-review"), Some("gstack"));
    assert_eq!(builtin_skill_group_key("unknown-skill"), None);

    assert_eq!(
        resolve_builtin_default_enabled("ppt-design", "agentic"),
        Some(false)
    );
    assert_eq!(
        resolve_builtin_default_enabled("ppt-design", "Cowork"),
        Some(true)
    );
    assert_eq!(
        resolve_builtin_default_enabled("find-skills", "DeepResearch"),
        Some(true)
    );
    assert_eq!(
        resolve_builtin_default_enabled("miniapp-dev", "agentic"),
        Some(true)
    );
    assert_eq!(
        resolve_builtin_default_enabled("miniapp-dev", "Cowork"),
        Some(false)
    );
    assert_eq!(
        resolve_builtin_default_enabled("miniapp-dev", "DeepResearch"),
        Some(false)
    );
    assert_eq!(
        resolve_builtin_default_enabled("miniapp-dev", "Team"),
        Some(false)
    );
    assert_eq!(
        resolve_builtin_default_enabled("agent-browser", "coding_shared"),
        Some(true)
    );
    assert_eq!(
        resolve_builtin_default_enabled("bitfun-canvas", "agentic"),
        Some(true)
    );
}

#[test]
fn skill_discovery_root_facts_are_runtime_owned() {
    assert_eq!(USER_SKILL_KEY_PREFIX, "user");
    assert_eq!(PROJECT_SKILL_KEY_PREFIX, "project");
    assert_eq!(BITFUN_USER_SKILL_SLOT, "bitfun");
    assert_eq!(BITFUN_SYSTEM_SKILL_SLOT, "bitfun-system");
    assert_eq!(BITFUN_SYSTEM_SKILL_DIR, ".system");

    let project_roots = PROJECT_SKILL_ROOTS
        .iter()
        .map(|root| (root.parent, root.slot, root.source_id, root.source_label))
        .collect::<Vec<_>>();
    assert_eq!(
        project_roots,
        [
            (".bitfun", "bitfun", "bitfun", "BitFun"),
            (".claude", "claude", "claude-code", "Claude Code"),
            (".codex", "codex", "codex", "Codex"),
            (".cursor", "cursor", "cursor", "Cursor"),
            (".opencode", "opencode", "opencode", "OpenCode"),
            (".agents", "agents", "agent-skills", "Agent Skills"),
        ]
    );

    let user_home_roots = USER_HOME_SKILL_ROOTS
        .iter()
        .map(|root| (root.parent, root.slot, root.source_id, root.source_label))
        .collect::<Vec<_>>();
    assert_eq!(
        user_home_roots,
        [
            (".claude", "home.claude", "claude-code", "Claude Code"),
            (".codex", "home.codex", "codex", "Codex"),
            (".cursor", "home.cursor", "cursor", "Cursor"),
            (".opencode", "home.opencode", "opencode", "OpenCode"),
            (".agents", "home.agents", "agent-skills", "Agent Skills"),
        ]
    );
    assert_eq!(
        USER_CONFIG_SKILL_ROOTS
            .iter()
            .map(|root| (root.parent, root.slot, root.source_id, root.source_label))
            .collect::<Vec<_>>(),
        [("opencode", "config.opencode", "opencode", "OpenCode")]
    );
    assert!(!USER_CONFIG_SKILL_ROOTS
        .iter()
        .any(|root| root.parent == "agents"));
}

#[test]
fn skill_source_identity_is_serialized_without_changing_slot_identity() {
    let mut info = project_skill("pdf");
    info.source_id = "bitfun".to_string();
    info.source_label = "BitFun".to_string();

    let value = serde_json::to_value(info).expect("skill info should serialize");
    assert_eq!(value["sourceSlot"], "bitfun");
    assert_eq!(value["sourceId"], "bitfun");
    assert_eq!(value["sourceLabel"], "BitFun");
}

#[test]
fn user_config_skill_root_resolution_matches_platform_contract() {
    let opencode = USER_CONFIG_SKILL_ROOTS
        .iter()
        .find(|root| root.parent == "opencode")
        .expect("opencode config root should exist");

    if cfg!(target_os = "windows") {
        let resolved = resolve_user_config_skill_root(
            opencode,
            std::path::Path::new(r"C:\Users\tester\AppData\Roaming"),
            Some(std::path::Path::new(r"C:\Users\tester")),
        );
        assert_eq!(
            resolved,
            PathBuf::from(r"C:\Users\tester\.config\opencode\skills")
        );
    } else {
        let resolved = resolve_user_config_skill_root(
            opencode,
            std::path::Path::new("/home/tester/.config"),
            Some(std::path::Path::new("/home/tester")),
        );
        assert_eq!(
            resolved,
            PathBuf::from("/home/tester/.config/opencode/skills")
        );
    }
}

#[test]
fn skill_resolution_applies_builtin_and_user_override_rules() {
    let pdf = builtin_skill("pdf");
    let custom = custom_user_skill("my-custom-skill");
    let disabled_project = HashSet::new();

    assert!(!resolve_skill_default_enabled_for_mode(&pdf, "agentic"));
    assert!(resolve_skill_default_enabled_for_mode(&custom, "agentic"));

    let default_state = resolve_skill_state_for_mode(
        &pdf,
        "agentic",
        &UserModeSkillOverrides::default(),
        &disabled_project,
    );
    assert!(!default_state.effective_enabled);
    assert_eq!(
        default_state.reason,
        ModeSkillStateReason::BuiltinPolicyDisabled
    );

    let mut overrides = UserModeSkillOverrides::default();
    overrides.enabled_skills.push(pdf.key.clone());
    let enabled_state =
        resolve_skill_state_for_mode(&pdf, "agentic", &overrides, &disabled_project);
    assert!(enabled_state.effective_enabled);
    assert_eq!(
        enabled_state.reason,
        ModeSkillStateReason::EnabledByUserOverride
    );
}

#[test]
fn user_mode_skill_overrides_share_key_normalization_rules() {
    let overrides = bitfun_agent_runtime::skills::normalize_user_mode_skill_overrides(
        vec![
            " user::bitfun::pdf ".to_string(),
            String::new(),
            "user::bitfun::pdf".to_string(),
        ],
        vec![
            "user::bitfun::pdf".to_string(),
            " user::bitfun::docx ".to_string(),
            "user::bitfun::docx".to_string(),
        ],
    );

    assert_eq!(overrides.disabled_skills, vec!["user::bitfun::pdf"]);
    assert_eq!(overrides.enabled_skills, vec!["user::bitfun::docx"]);
}

#[test]
fn skill_markdown_and_assistant_output_shape_are_runtime_owned() {
    let markdown = r#"---
name: pdf
description: Work with PDF files.
---

Use the pdf workflow.
"#;
    let mut data = SkillData::from_markdown(
        "/workspace/.bitfun/skills/pdf".to_string(),
        markdown,
        SkillLocation::Project,
        true,
    )
    .expect("valid skill markdown should parse");
    data.key = "project::bitfun::pdf".to_string();
    data.source_slot = "bitfun".to_string();

    assert_eq!(data.name, "pdf");
    assert_eq!(data.description, "Work with PDF files.");
    assert_eq!(data.dir_name, "pdf");
    assert_eq!(data.content, "Use the pdf workflow.\n");

    let assistant = render_loaded_skill_for_assistant(&data, false);
    assert!(assistant.contains("Skill 'pdf' loaded successfully."));
    assert!(assistant.contains("relative to /workspace/.bitfun/skills/pdf"));
    assert!(assistant.contains("<skill_content>\nUse the pdf workflow.\n\n</skill_content>"));
    assert!(!assistant.contains("from stable key"));

    let stable_assistant = render_loaded_skill_for_assistant(&data, true);
    assert!(stable_assistant.contains("from stable key 'project::bitfun::pdf'"));
}

#[test]
fn skill_candidate_key_group_and_resolution_are_runtime_owned() {
    let markdown = r#"---
name: pdf
description: Work with PDF files.
---

Use the pdf workflow.
"#;
    let data = SkillData::from_markdown(
        "/tmp/bitfun-system/pdf".to_string(),
        markdown,
        SkillLocation::User,
        false,
    )
    .expect("valid built-in skill markdown should parse");
    let candidate =
        SkillCandidate::from_data(data, "bitfun-system", "bitfun", "BitFun", "user", 10, true);

    assert_eq!(candidate.info.key, "user::bitfun-system::pdf");
    assert_eq!(candidate.info.source_slot, "bitfun-system");
    assert_eq!(candidate.info.group_key.as_deref(), Some("office"));

    let project_pdf = SkillCandidate {
        info: project_skill("pdf"),
        priority: 0,
    };
    let visible = resolve_visible_skills(vec![candidate.clone(), project_pdf.clone()]);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].key, "project::bitfun::pdf");

    let annotated = sort_skills(annotate_shadowed_skills(vec![candidate, project_pdf]));
    let user_pdf = annotated
        .iter()
        .find(|skill| skill.key == "user::bitfun-system::pdf")
        .expect("user built-in skill should be present");
    assert!(user_pdf.is_shadowed);
    assert_eq!(
        user_pdf.shadowed_by_key.as_deref(),
        Some("project::bitfun::pdf")
    );
}

#[test]
fn shadow_annotations_use_the_same_level_tiebreaker_as_runtime_resolution() {
    let mut user_info = custom_user_skill("pdf");
    user_info.key = "user::home.claude::pdf".to_string();
    user_info.source_slot = "home.claude".to_string();
    user_info.source_id = "claude-code".to_string();
    user_info.source_label = "Claude Code".to_string();
    let user_candidate = SkillCandidate {
        info: user_info,
        priority: 0,
    };
    let project_candidate = SkillCandidate {
        info: project_skill("pdf"),
        priority: 0,
    };

    let runtime_winner =
        resolve_visible_skills(vec![user_candidate.clone(), project_candidate.clone()]);
    assert_eq!(runtime_winner[0].key, project_candidate.info.key);

    let annotated =
        annotate_shadowed_skills(vec![user_candidate.clone(), project_candidate.clone()]);
    let user = annotated
        .iter()
        .find(|skill| skill.key == user_candidate.info.key)
        .expect("user candidate should remain visible");
    let project = annotated
        .iter()
        .find(|skill| skill.key == project_candidate.info.key)
        .expect("project candidate should remain visible");

    assert!(user.is_shadowed);
    assert_eq!(
        user.shadowed_by_key.as_deref(),
        Some(project_candidate.info.key.as_str())
    );
    assert!(!project.is_shadowed);
    assert_eq!(project.shadowed_by_key, None);
}

#[test]
fn mode_skill_candidate_filtering_and_info_are_runtime_owned() {
    let project_doc = SkillCandidate {
        info: project_skill("project-doc"),
        priority: 0,
    };
    let custom_user = SkillCandidate {
        info: custom_user_skill("my-custom-skill"),
        priority: 10,
    };
    let mut disabled_project = HashSet::new();
    disabled_project.insert(project_doc.info.key.clone());

    let filtered = filter_candidates_for_mode(
        vec![project_doc.clone(), custom_user.clone()],
        "agentic",
        &UserModeSkillOverrides::default(),
        &disabled_project,
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].info.key, custom_user.info.key);

    let all_skills = sort_skills(annotate_shadowed_skills(vec![
        project_doc,
        custom_user.clone(),
    ]));
    let resolved = resolve_visible_skills(filtered);
    let infos = build_mode_skill_infos(
        all_skills,
        resolved,
        "agentic",
        &UserModeSkillOverrides::default(),
        &disabled_project,
    );

    let project_doc = infos
        .iter()
        .find(|skill| skill.skill.key == "project::bitfun::project-doc")
        .expect("project skill should be listed");
    assert!(!project_doc.effective_enabled);
    assert!(!project_doc.selected_for_runtime);
    assert_eq!(
        project_doc.state_reason,
        ModeSkillStateReason::DisabledByProjectOverride
    );

    let custom = infos
        .iter()
        .find(|skill| skill.skill.key == custom_user.info.key)
        .expect("custom user skill should be listed");
    assert!(custom.effective_enabled);
    assert!(custom.selected_for_runtime);
    assert_eq!(
        custom.state_reason,
        ModeSkillStateReason::CustomUserDefaultEnabled
    );
}

#[test]
fn mode_skill_info_reports_the_actual_runtime_winner_after_filtering() {
    let project_pdf = SkillCandidate {
        info: project_skill("pdf"),
        priority: 0,
    };
    let mut codex_pdf_info = custom_user_skill("pdf");
    codex_pdf_info.key = "user::home.codex::pdf".to_string();
    codex_pdf_info.source_slot = "home.codex".to_string();
    codex_pdf_info.source_id = "codex".to_string();
    codex_pdf_info.source_label = "Codex".to_string();
    let codex_pdf = SkillCandidate {
        info: codex_pdf_info,
        priority: 10,
    };
    let mut opencode_pdf_info = custom_user_skill("pdf");
    opencode_pdf_info.key = "user::home.opencode::pdf".to_string();
    opencode_pdf_info.source_slot = "home.opencode".to_string();
    opencode_pdf_info.source_id = "opencode".to_string();
    opencode_pdf_info.source_label = "OpenCode".to_string();
    let opencode_pdf = SkillCandidate {
        info: opencode_pdf_info,
        priority: 11,
    };
    let candidates = vec![project_pdf.clone(), codex_pdf.clone(), opencode_pdf.clone()];
    let mut disabled_project = HashSet::new();
    disabled_project.insert(project_pdf.info.key.clone());

    let filtered = filter_candidates_for_mode(
        candidates.clone(),
        "agentic",
        &UserModeSkillOverrides::default(),
        &disabled_project,
    );
    let resolved = resolve_visible_skills(filtered);
    let infos = build_mode_skill_infos(
        sort_skills(annotate_shadowed_skills(candidates)),
        resolved,
        "agentic",
        &UserModeSkillOverrides::default(),
        &disabled_project,
    );

    let project = infos
        .iter()
        .find(|skill| skill.skill.key == project_pdf.info.key)
        .expect("disabled project skill should stay visible");
    assert!(!project.effective_enabled);
    assert!(!project.selected_for_runtime);
    assert!(!project.skill.is_shadowed);
    assert_eq!(project.skill.shadowed_by_key, None);

    let codex = infos
        .iter()
        .find(|skill| skill.skill.key == codex_pdf.info.key)
        .expect("selected Codex skill should stay visible");
    assert!(codex.effective_enabled);
    assert!(codex.selected_for_runtime);
    assert!(!codex.skill.is_shadowed);

    let opencode = infos
        .iter()
        .find(|skill| skill.skill.key == opencode_pdf.info.key)
        .expect("covered OpenCode skill should stay visible");
    assert!(opencode.effective_enabled);
    assert!(!opencode.selected_for_runtime);
    assert!(opencode.skill.is_shadowed);
    assert_eq!(
        opencode.skill.shadowed_by_key.as_deref(),
        Some(codex_pdf.info.key.as_str())
    );
}

#[test]
fn explicit_invocation_hidden_builtin_fallback_is_runtime_owned() {
    let candidate = SkillCandidate {
        info: builtin_skill("gstack-review"),
        priority: 10,
    };

    match resolve_default_hidden_builtin_for_explicit_invocation(
        "gstack-review",
        vec![candidate.clone()],
        Some("agentic"),
    ) {
        ExplicitSkillInvocationResolution::Found(skill) => {
            assert_eq!(skill.key, "user::bitfun-system::gstack-review");
        }
        other => panic!("expected hidden gstack fallback, got {other:?}"),
    }

    assert!(matches!(
        resolve_default_hidden_builtin_for_explicit_invocation(
            "missing-skill",
            vec![candidate.clone()],
            Some("agentic")
        ),
        ExplicitSkillInvocationResolution::NotFound
    ));
    assert!(matches!(
        resolve_default_hidden_builtin_for_explicit_invocation(
            "gstack-review",
            vec![candidate],
            None
        ),
        ExplicitSkillInvocationResolution::NotFound
    ));
}
