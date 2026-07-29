const MAX_TUI_NATIVE_HOOK_RULES: usize = 100;
const MAX_TUI_NATIVE_HOOK_HANDLERS_PER_RULE: usize = 20;
const MAX_TUI_NATIVE_HOOK_ISSUES: usize = 20;
const MAX_TUI_NATIVE_HOOK_COMMAND_CHARS: usize = 200;

fn native_hook_help_text() -> String {
    [
        "Hooks",
        "",
        "Usage: /hooks [refresh | import <source-number> [--confirm] | update <import-number> [--confirm] | enable <import-number> | disable <import-number> | remove <import-number> --confirm | reset <user|project> --confirm]",
        "",
        "Shows native BitFun Hooks plus compatible Claude Code and Codex command Hooks.",
        "Import and update are preview-only until the exact reviewed plan is confirmed. Source files are never edited.",
        "Compatibility aliases: /hooks_external and /hooks-external.",
        "",
        "Help: /help hooks, /hooks -h, or /hooks --help",
    ]
    .join("\n")
}

fn truncate_hook_command(command: &str) -> String {
    let command = command.trim();
    if command.chars().count() <= MAX_TUI_NATIVE_HOOK_COMMAND_CHARS {
        return command.to_string();
    }
    let kept = command
        .chars()
        .take(MAX_TUI_NATIVE_HOOK_COMMAND_CHARS)
        .collect::<String>();
    format!("{kept}…")
}

fn native_hook_rule_line(rule: &NativeHookRuleView) -> String {
    format!(
        "  matcher: {} [{}; {} handler{}{}]",
        crate::plugin_diagnostics::escape_terminal_text(&rule.matcher),
        rule.scope,
        rule.handlers.len(),
        plural(rule.handlers.len()),
        if rule.matcher_is_valid {
            ""
        } else {
            "; invalid pattern, never matches"
        },
    )
}

fn render_native_hook_overview(overview: &NativeHookOverview) -> String {
    let mut lines = vec![
        "Hooks (BitFun)".to_string(),
        "Commands BitFun runs at agent lifecycle events. Nothing was executed to build this view."
            .to_string(),
        String::new(),
    ];
    lines.push(format!(
        "Hooks: {} (app.hooks.enabled)",
        if overview.enabled {
            "enabled"
        } else {
            "disabled"
        }
    ));
    lines.push(format!(
        "Project hook file: {} (app.hooks.project_hooks_enabled)",
        if overview.project_hooks_enabled {
            "enabled"
        } else {
            "disabled"
        }
    ));

    lines.push(String::new());
    if overview.files.is_empty() {
        lines.push("No hook configuration path is available on this host.".to_string());
    } else {
        lines.push("Configuration:".to_string());
        for file in &overview.files {
            lines.push(format!(
                "  {} [{}; {}]: {}",
                file.scope,
                if file.loaded { "loaded" } else { "not loaded" },
                if file.exists { "present" } else { "missing" },
                crate::plugin_diagnostics::escape_terminal_text(&file.path.to_string_lossy()),
            ));
        }
    }

    lines.push(String::new());
    if !overview.enabled {
        lines.push("All hooks are off; set app.hooks.enabled to run them.".to_string());
    } else if overview.rules.is_empty() {
        lines.push("No hooks are configured.".to_string());
    } else {
        lines.push(format!(
            "{} matcher group{}, {} handler{}:",
            overview.rules.len(),
            plural(overview.rules.len()),
            overview.total_handlers,
            plural(overview.total_handlers),
        ));
        let mut current_event = "";
        for rule in overview.rules.iter().take(MAX_TUI_NATIVE_HOOK_RULES) {
            if rule.event != current_event {
                current_event = rule.event;
                lines.push(String::new());
                lines.push(crate::plugin_diagnostics::escape_terminal_text(rule.event));
            }
            lines.push(native_hook_rule_line(rule));
            for handler in rule
                .handlers
                .iter()
                .take(MAX_TUI_NATIVE_HOOK_HANDLERS_PER_RULE)
            {
                lines.push(format!(
                    "    - {} [timeout {}s{}]",
                    truncate_hook_command(&crate::plugin_diagnostics::escape_terminal_text(
                        &handler.command,
                    )),
                    handler.timeout_seconds,
                    match handler.status_message.as_deref() {
                        Some(message) if !message.trim().is_empty() => format!(
                            "; status: {}",
                            crate::plugin_diagnostics::escape_terminal_text(message.trim())
                        ),
                        _ => String::new(),
                    },
                ));
            }
            let omitted_handlers = rule
                .handlers
                .len()
                .saturating_sub(MAX_TUI_NATIVE_HOOK_HANDLERS_PER_RULE);
            if omitted_handlers > 0 {
                lines.push(format!("    … omitted {omitted_handlers} handler(s)."));
            }
        }
        let omitted_rules = overview
            .rules
            .len()
            .saturating_sub(MAX_TUI_NATIVE_HOOK_RULES);
        if omitted_rules > 0 {
            lines.push(String::new());
            lines.push(format!(
                "… omitted {omitted_rules} matcher group(s); open the hook files for the full configuration."
            ));
        }
    }

    if !overview.issues.is_empty() {
        lines.push(String::new());
        lines.push("Configuration issues:".to_string());
        for issue in overview.issues.iter().take(MAX_TUI_NATIVE_HOOK_ISSUES) {
            lines.push(format!(
                "  ! {}",
                crate::plugin_diagnostics::escape_terminal_text(issue)
            ));
        }
        if overview.issues.len() > MAX_TUI_NATIVE_HOOK_ISSUES {
            lines.push(format!(
                "  … {} additional issue(s) omitted.",
                overview.issues.len() - MAX_TUI_NATIVE_HOOK_ISSUES
            ));
        }
    }

    lines.push(String::new());
    lines.push("Manual Hooks remain editable in hooks.json. Imported Hooks are managed through /hooks. Help: /help hooks, /hooks -h, or /hooks --help".to_string());
    lines.join("\n")
}
