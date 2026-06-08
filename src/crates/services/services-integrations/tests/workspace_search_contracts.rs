#[cfg(feature = "workspace-search")]
mod workspace_search {
    use bitfun_services_integrations::workspace_search::{
        flashgrep::SearchModeConfig, workspace_search_daemon_binary_name,
        workspace_search_daemon_binary_names, workspace_search_daemon_missing_hint,
        ContentSearchOutputMode, WorkspaceSearchService,
    };

    #[test]
    fn daemon_binary_contract_lists_current_platform_candidate() {
        let primary = workspace_search_daemon_binary_name();

        assert!(!primary.is_empty());
        assert!(workspace_search_daemon_binary_names().contains(&primary));
    }

    #[test]
    fn daemon_missing_hint_preserves_env_override_guidance() {
        let hint = workspace_search_daemon_missing_hint();

        assert!(hint.contains("FLASHGREP_DAEMON_BIN"));
        assert!(hint.contains("flashgrep/"));
        assert!(hint.contains(workspace_search_daemon_binary_name()));
    }

    #[test]
    fn service_constructs_without_core_runtime_dependencies() {
        let _service = WorkspaceSearchService::new();

        assert_eq!(
            ContentSearchOutputMode::Content.search_mode(),
            SearchModeConfig::LineMatches
        );
        assert_eq!(
            ContentSearchOutputMode::Count.search_mode(),
            SearchModeConfig::CountOnly
        );
        assert_eq!(
            ContentSearchOutputMode::FilesWithMatches.search_mode(),
            SearchModeConfig::FilesWithMatches
        );
    }
}
