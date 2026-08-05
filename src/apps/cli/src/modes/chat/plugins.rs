/// Plugin browser ChatMode integration.
///
/// Mirrors the MCP toggle pattern: popups never spawn async work directly.
/// Instead, the popup returns a `PluginBrowserAction::Toggle(item)` from
/// `handle_key_event`; the main key handler schedules `pending_plugin_op`,
/// the run loop renders the loading state, then spawns the toggle on
/// `rt_handle`, stores the `JoinHandle` in `pending_plugin_tasks`, polls
/// `is_finished()` each loop, and on completion refreshes the popup items
/// via `refresh_managed_plugin_sources`.
use bitfun_core::plugin_runtime::{activate_managed_plugin, deactivate_managed_plugin};
use bitfun_core::plugin_source::refresh_managed_plugin_sources;

use crate::ui::plugin_browser::plugin_items_from_snapshot;

impl ChatMode {
    /// Show the plugin browser popup. Loads items synchronously via
    /// `block_in_place + block_on` so the popup opens with a complete list.
    fn show_plugin_browser(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let items = self.get_plugin_items(rt_handle);
        chat_view.show_plugin_browser(items);
    }

    /// Load current plugin items from the managed plugin source service.
    pub(super) fn get_plugin_items(&self, rt_handle: &tokio::runtime::Handle) -> Vec<PluginItem> {
        let workspace = self.agent.workspace_path_buf();
        tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                match refresh_managed_plugin_sources(&workspace).await {
                    Ok(snapshot) => plugin_items_from_snapshot(&snapshot),
                    Err(error) => {
                        tracing::error!("Failed to load plugin snapshot: {}", error);
                        Vec::new()
                    }
                }
            })
        })
    }

    /// Schedule a plugin toggle (deferred to allow the loading state to render).
    fn toggle_plugin(&mut self, item: PluginItem, chat_view: &mut ChatView) {
        if self.pending_plugin_op.is_some() || self.is_plugin_task_running(&item.id) {
            return;
        }
        chat_view.plugin_browser_set_loading(Some(item.id.clone()));
        self.pending_plugin_op = Some(PendingPluginOp::Toggle(item));
    }

    fn is_plugin_task_running(&self, plugin_id: &str) -> bool {
        self.pending_plugin_tasks.iter().any(|task| match task {
            PendingPluginTask::Toggle { plugin_id: id, .. } => id == plugin_id,
            PendingPluginTask::Install { .. } => false,
        })
    }

    /// Execute the plugin toggle by spawning an async task on `rt_handle`.
    /// The task returns `Result<(), String>` so the poll loop can render a
    /// uniform error message regardless of the underlying source error type.
    fn execute_plugin_toggle(
        &mut self,
        item: &PluginItem,
        _chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let workspace = self.agent.workspace_path_buf();
        let plugin_id = item.id.clone();
        let content_hash = item.content_hash.clone();
        let was_activated = item.activated;
        let tracked_id = plugin_id.clone();
        let handle = rt_handle.spawn(async move {
            if was_activated {
                deactivate_managed_plugin(&workspace, &plugin_id)
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            } else {
                activate_managed_plugin(&workspace, &plugin_id, Some(&content_hash))
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
        });
        self.pending_plugin_tasks.push(PendingPluginTask::Toggle {
            plugin_id: tracked_id,
            handle,
        });
    }

    fn is_install_task_running(&self) -> bool {
        self.pending_plugin_tasks
            .iter()
            .any(|task| matches!(task, PendingPluginTask::Install { .. }))
    }

    /// Schedule a plugin install (deferred to allow the busy state to render).
    fn install_plugin(
        &mut self,
        spec: String,
        scope: PluginInstallScope,
        chat_view: &mut ChatView,
    ) {
        if self.pending_plugin_op.is_some() || self.is_install_task_running() {
            chat_view.plugin_browser_set_install_busy(false);
            return;
        }
        chat_view.plugin_browser_set_install_busy(true);
        self.pending_plugin_op = Some(PendingPluginOp::Install { spec, scope });
    }

    /// Execute the plugin install by spawning an async task on `rt_handle`.
    fn execute_plugin_install(
        &mut self,
        spec: String,
        scope: PluginInstallScope,
        _chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let workspace = self.agent.workspace_path_buf();
        let spec_for_task = spec.clone();
        let handle = rt_handle
            .spawn(async move { install_managed_plugin(&workspace, &spec_for_task, scope).await });
        self.pending_plugin_tasks
            .push(PendingPluginTask::Install { spec, handle });
    }

    /// Poll in-flight plugin tasks. On completion, clears the loading
    /// indicator and refreshes the popup items. Returns `true` if any state
    /// changed (so the run loop can schedule a redraw).
    fn poll_plugin_task_completion(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < self.pending_plugin_tasks.len() {
            let finished = match &self.pending_plugin_tasks[i] {
                PendingPluginTask::Toggle { handle, .. } => handle.is_finished(),
                PendingPluginTask::Install { handle, .. } => handle.is_finished(),
            };
            if !finished {
                i += 1;
                continue;
            }
            let task = self.pending_plugin_tasks.swap_remove(i);
            changed = true;
            match task {
                PendingPluginTask::Toggle { plugin_id, handle } => {
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));
                    match join_result {
                        Ok(Ok(())) => {
                            chat_state
                                .add_system_message(format!("Plugin '{}' toggled", plugin_id));
                        }
                        Ok(Err(error)) => {
                            tracing::error!("Failed to toggle plugin '{}': {}", plugin_id, error);
                            chat_state.add_system_message(format!(
                                "Failed to toggle plugin '{}': {}",
                                plugin_id, error
                            ));
                        }
                        Err(error) => {
                            tracing::error!(
                                "Plugin toggle task join error for '{}': {}",
                                plugin_id,
                                error
                            );
                            chat_state.add_system_message(format!(
                                "Plugin '{}' toggle task failed: {}",
                                plugin_id, error
                            ));
                        }
                    }
                    chat_view.plugin_browser_set_loading(None);
                    let updated_items = self.get_plugin_items(rt_handle);
                    chat_view.plugin_browser_update_items(updated_items);
                }
                PendingPluginTask::Install { spec, handle } => {
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));
                    match join_result {
                        Ok(Ok(())) => {
                            chat_view.plugin_browser_set_install_message(None);
                            chat_state.add_system_message(format!("Plugin '{}' installed", spec));
                        }
                        Ok(Err(error)) => {
                            tracing::error!("Failed to install plugin '{}': {}", spec, error);
                            chat_view.plugin_browser_set_install_message(Some(error.clone()));
                            chat_state.add_system_message(format!(
                                "Failed to install plugin '{}': {}",
                                spec, error
                            ));
                        }
                        Err(error) => {
                            tracing::error!(
                                "Plugin install task join error for '{}': {}",
                                spec,
                                error
                            );
                            chat_view.plugin_browser_set_install_message(Some(format!(
                                "install task failed: {}",
                                error
                            )));
                            chat_state.add_system_message(format!(
                                "Plugin '{}' install task failed: {}",
                                spec, error
                            ));
                        }
                    }
                    chat_view.plugin_browser_set_install_busy(false);
                    let updated_items = self.get_plugin_items(rt_handle);
                    chat_view.plugin_browser_update_items(updated_items);
                }
            }
        }
        changed
    }
}

/// Install a managed plugin from a package specifier.
///
/// TODO: replace with `bitfun_core::plugin_source::install_managed_plugin`
/// once the core install API lands. This skeleton placeholder reports the
/// operation as not yet implemented so the install UI flow can be exercised
/// without crashing the TUI.
async fn install_managed_plugin(
    _workspace: &std::path::Path,
    _spec: &str,
    _scope: PluginInstallScope,
) -> std::result::Result<(), String> {
    Err("plugin install is not yet implemented (TODO: wire core install API)".to_string())
}
