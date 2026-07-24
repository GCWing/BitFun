const MAX_TUI_HOOK_SOURCES_PER_PROVIDER: usize = 10;
const MAX_TUI_HOOK_ENTRIES_PER_PROVIDER: usize = 100;
const MAX_TUI_HOOK_DIAGNOSTICS_PER_PROVIDER: usize = 20;
const MAX_TUI_HOOK_CATALOG_DIAGNOSTICS: usize = 20;

fn external_hook_help_text() -> String {
    [
        "Hooks",
        "",
        "Usage: /hooks",
        "",
        "Shows a read-only static catalog of Hooks configured for OpenCode, Claude Code, and Codex.",
        "BitFun does not load or run handlers from this view. Coverage mapped means BitFun recognizes an equivalent reviewed Hook point; it does not mean the native handler is active.",
        "",
        "Help: /help hooks, /hooks -h, or /hooks --help",
    ]
    .join("\n")
}

fn extension_command_help_request(command_name: &str, arguments: &str) -> Option<String> {
    let arguments = arguments.trim();
    let requested = if command_name.eq_ignore_ascii_case("help") {
        arguments
    } else if matches!(arguments, "-h" | "--help" | "help") {
        command_name
    } else {
        return None;
    };
    match requested.to_ascii_lowercase().as_str() {
        "hooks" => Some(external_hook_help_text()),
        "extensions" => Some([
            "External integrations",
            "",
            "Usage: /extensions [status | refresh | safe-mode on | safe-mode off | source enable <source-key> | source disable <source-key>]",
            "",
            "Shows external AI application sources and controls BitFun Safe Mode. Source files remain owned by their native application.",
            "",
            "Help: /help extensions, /extensions -h, or /extensions --help",
        ].join("\n")),
        "tools" => Some([
            "Tools",
            "",
            "Usage: /tools [refresh | enable <number> | disable <number> | choose <conflict-number> <choice-number>]",
            "",
            "Shows BitFun, MCP, and compatible external tool sources. Activation and conflicts remain guarded by BitFun policy.",
            "",
            "Help: /help tools, /tools -h, or /tools --help",
        ].join("\n")),
        "agent" | "agents" => Some([
            "Agents",
            "",
            "Usage: /agent [refresh | enable <number> | disable <number> | choose <conflict-number> <choice-number>]",
            "Alias: /agents",
            "",
            "Without arguments, opens the agent selector. Management arguments review compatible external subagents.",
            "",
            "Help: /help agent, /agent -h, or /agent --help",
        ].join("\n")),
        "mcp" | "mcps" => Some([
            "MCP servers",
            "",
            "Usage: /mcp",
            "Alias: /mcps",
            "",
            "Opens the shared MCP server manager, including compatible external MCP definitions after approval.",
            "",
            "Help: /help mcp, /mcp -h, or /mcp --help",
        ].join("\n")),
        _ => None,
    }
}

fn render_external_hook_catalog(snapshot: &ExternalHookCatalogSnapshotV1) -> String {
    let mut lines = vec![
        "Hooks (read-only)".to_string(),
        "Static configuration only; no handler was loaded or executed.".to_string(),
        String::new(),
    ];
    if snapshot.discovery_pending {
        lines.push("Hook discovery is still pending. Run /hooks again.".to_string());
        return lines.join("\n");
    }
    if snapshot.sources.is_empty()
        && snapshot.failed_provider_ids.is_empty()
        && snapshot.stale_provider_ids.is_empty()
    {
        lines.push("No supported Hook configuration was found.".to_string());
    }
    if !snapshot.providers.is_empty() {
        let source_by_key = snapshot
            .sources
            .iter()
            .map(|source| (&source.key, source))
            .collect::<BTreeMap<_, _>>();
        for provider in &snapshot.providers {
            let provider_sources = snapshot
                .sources
                .iter()
                .filter(|source| source.key.provider_id == provider.provider_id)
                .collect::<Vec<_>>();
            let provider_entry_count = snapshot
                .entries
                .iter()
                .filter(|entry| entry.source.provider_id == provider.provider_id)
                .count();
            let stale = snapshot
                .stale_provider_ids
                .iter()
                .any(|provider_id| provider_id == &provider.provider_id);
            let failed = snapshot
                .failed_provider_ids
                .iter()
                .any(|provider_id| provider_id == &provider.provider_id);
            lines.push(format!(
                "{}: {} Hook{}, {} source{}{}",
                provider.display_name,
                provider_entry_count,
                plural(provider_entry_count),
                provider_sources.len(),
                plural(provider_sources.len()),
                if failed {
                    " (discovery failed)"
                } else if stale {
                    " (stale)"
                } else {
                    ""
                },
            ));
            if provider_sources.is_empty() {
                lines.push(if failed {
                    "  No valid catalog is available because static discovery failed.".to_string()
                } else if stale {
                    "  The last valid catalog is empty; the latest refresh failed.".to_string()
                } else {
                    "  No supported static Hook source was found.".to_string()
                });
                continue;
            }
            let mut rendered_entries = 0;
            let mut rendered_diagnostics = 0;
            for source in provider_sources
                .iter()
                .take(MAX_TUI_HOOK_SOURCES_PER_PROVIDER)
            {
                lines.push(format!(
                    "  {} [{}; {}; {}]",
                    source.display_name,
                    source_scope_label(source.scope),
                    source_health_label(source.health),
                    source.location_hint,
                ));
                for entry in snapshot
                    .entries
                    .iter()
                    .filter(|entry| entry.source == source.key)
                    .take(MAX_TUI_HOOK_ENTRIES_PER_PROVIDER - rendered_entries)
                {
                    lines.push(format!(
                        "    - {} [{}; {}; {}; matcher: {}]",
                        entry.native_event,
                        hook_handler_label(entry.handler_kind),
                        projection_label(entry),
                        native_activation_label(entry.native_activation),
                        matcher_label(&entry.matcher),
                    ));
                    rendered_entries += 1;
                }
                for diagnostic in source
                    .diagnostics
                    .iter()
                    .take(MAX_TUI_HOOK_DIAGNOSTICS_PER_PROVIDER - rendered_diagnostics)
                {
                    lines.push(format!("    ! {}: {}", diagnostic.code, diagnostic.message));
                    rendered_diagnostics += 1;
                }
            }
            let omitted_sources = provider_sources
                .len()
                .saturating_sub(MAX_TUI_HOOK_SOURCES_PER_PROVIDER);
            let omitted_entries = provider_entry_count.saturating_sub(rendered_entries);
            let provider_diagnostic_count = provider_sources
                .iter()
                .map(|source| source.diagnostics.len())
                .sum::<usize>();
            let omitted_diagnostics =
                provider_diagnostic_count.saturating_sub(rendered_diagnostics);
            if omitted_sources + omitted_entries + omitted_diagnostics > 0 {
                lines.push(format!(
                    "  … omitted {omitted_sources} source(s), {omitted_entries} Hook(s), and {omitted_diagnostics} diagnostic(s); use Desktop settings for the full catalog."
                ));
            }
        }
        for entry in snapshot
            .entries
            .iter()
            .filter(|entry| !source_by_key.contains_key(&entry.source))
            .take(MAX_TUI_HOOK_ENTRIES_PER_PROVIDER)
        {
            lines.push(format!("External: {}", entry.native_event));
        }
    }
    let catalog_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.source.is_none())
        .collect::<Vec<_>>();
    if !catalog_diagnostics.is_empty() {
        lines.push(String::new());
        lines.push("Catalog diagnostics:".to_string());
        for diagnostic in catalog_diagnostics
            .iter()
            .take(MAX_TUI_HOOK_CATALOG_DIAGNOSTICS)
        {
            lines.push(format!("  - {}: {}", diagnostic.code, diagnostic.message));
        }
        if catalog_diagnostics.len() > MAX_TUI_HOOK_CATALOG_DIAGNOSTICS {
            lines.push(format!(
                "  … {} additional catalog diagnostic(s) omitted.",
                catalog_diagnostics.len() - MAX_TUI_HOOK_CATALOG_DIAGNOSTICS
            ));
        }
    }
    lines.push(String::new());
    lines.push(
        "Edit Hooks in the source application's configuration. Help: /help hooks, /hooks -h, or /hooks --help"
            .to_string(),
    );
    lines.join("\n")
}

fn matcher_label(matcher: &ExternalHookMatcherSummary) -> String {
    match matcher {
        ExternalHookMatcherSummary::Any => "all".to_string(),
        ExternalHookMatcherSummary::Pattern { display } => display.clone(),
        ExternalHookMatcherSummary::Dynamic => "dynamic".to_string(),
        ExternalHookMatcherSummary::Unavailable => "unavailable".to_string(),
        _ => "unknown".to_string(),
    }
}

fn projection_label(entry: &bitfun_core::external_hooks::ExternalHookCatalogEntry) -> &'static str {
    match entry.projection_status {
        ExternalHookProjectionStatus::Mapped => match entry
            .mapping
            .as_ref()
            .map(|mapping| mapping.hook_point)
        {
            Some(
                bitfun_product_domains::external_hook_contributions::ExternalHookPoint::ToolBefore,
            ) => "coverage mapped: BitFun tool before",
            Some(
                bitfun_product_domains::external_hook_contributions::ExternalHookPoint::ToolAfter,
            ) => "coverage mapped: BitFun tool after",
            None => "invalid mapping",
        },
        ExternalHookProjectionStatus::NativeOnly => "native only",
        ExternalHookProjectionStatus::Opaque => "opaque static registration",
        _ => "unknown projection",
    }
}

fn native_activation_label(activation: ExternalHookNativeActivation) -> &'static str {
    match activation {
        ExternalHookNativeActivation::Disabled => "native disabled",
        ExternalHookNativeActivation::Unsupported => "unsupported by native runtime",
        ExternalHookNativeActivation::Unknown => "native activation unknown",
        _ => "native activation unknown",
    }
}

fn source_scope_label(scope: ExternalSourceScope) -> &'static str {
    match scope {
        ExternalSourceScope::UserGlobal => "user",
        ExternalSourceScope::WorkspaceLocal => "workspace",
        ExternalSourceScope::Project => "project",
        _ => "external",
    }
}

fn source_health_label(health: ExternalSourceHealth) -> &'static str {
    match health {
        ExternalSourceHealth::Available => "available",
        ExternalSourceHealth::Partial => "partial",
        ExternalSourceHealth::Degraded => "degraded",
        ExternalSourceHealth::Unavailable => "unavailable",
        _ => "unknown",
    }
}

fn hook_handler_label(kind: bitfun_core::external_hooks::ExternalHookHandlerKind) -> &'static str {
    use bitfun_core::external_hooks::ExternalHookHandlerKind;
    match kind {
        ExternalHookHandlerKind::Function => "function",
        ExternalHookHandlerKind::Command => "command",
        ExternalHookHandlerKind::Http => "http",
        ExternalHookHandlerKind::McpTool => "mcp_tool",
        ExternalHookHandlerKind::Prompt => "prompt",
        ExternalHookHandlerKind::Agent => "agent",
        _ => "unknown",
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

impl ChatMode {
    fn handle_external_hooks(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if self.external_hook_catalog_rx.is_some() {
            chat_view.set_status(Some("Hook catalog refresh already in progress".to_string()));
            return;
        }
        let workspace_root = self.workspace_path_for_sync(chat_state);
        let (sender, receiver) = mpsc::channel();
        rt_handle.spawn(async move {
            let mut force_refresh = true;
            let result = loop {
                let result = local_external_hook_catalog_snapshot(
                    Some(workspace_root.as_path()),
                    force_refresh,
                )
                .await;
                if !matches!(&result, Ok(snapshot) if snapshot.discovery_pending) {
                    break result;
                }
                force_refresh = false;
                tokio::time::sleep(Duration::from_millis(250)).await;
            };
            let _ = sender.send(result);
        });
        self.external_hook_catalog_rx = Some(receiver);
        chat_view.set_status(Some("Refreshing Hook catalog...".to_string()));
    }

    fn poll_external_hook_catalog(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
    ) -> bool {
        let result = match self
            .external_hook_catalog_rx
            .as_ref()
            .map(Receiver::try_recv)
        {
            Some(Ok(result)) => result,
            Some(Err(MpscTryRecvError::Empty)) | None => return false,
            Some(Err(MpscTryRecvError::Disconnected)) => {
                self.external_hook_catalog_rx = None;
                chat_view.set_status(Some("Hook catalog refresh failed".to_string()));
                chat_state.add_system_message(
                    "Hooks are unavailable because the background refresh ended unexpectedly."
                        .to_string(),
                );
                return true;
            }
        };
        self.external_hook_catalog_rx = None;
        match result {
            Ok(snapshot) => {
                chat_state.add_system_message(render_external_hook_catalog(&snapshot));
                chat_view.set_status(Some(format!(
                    "Hook catalog: {} sources, {} Hooks",
                    snapshot.sources.len(),
                    snapshot.entries.len(),
                )));
            }
            Err(error) => {
                chat_state.add_system_message(format!(
                    "Hooks are unavailable ({}): {}",
                    error.code.as_str(),
                    error.detail,
                ));
                chat_view.set_status(Some("Hook catalog unavailable".to_string()));
            }
        }
        true
    }
}
