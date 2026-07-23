use crate::service::config::get_global_config_service;
pub(crate) use terminal_core::ResolvedLocalExecShell;

pub(crate) async fn resolve_local_exec_shell() -> ResolvedLocalExecShell {
    let configured_shell = configured_shell_preference().await;
    terminal_core::resolve_local_exec_shell(configured_shell.as_deref())
}

async fn configured_shell_preference() -> Option<String> {
    let config_service = get_global_config_service().await.ok()?;
    config_service
        .get_config::<String>(Some("terminal.default_shell"))
        .await
        .ok()
}
