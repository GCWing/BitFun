use bitfun_services_core::diff::{DiffConfig, DiffLineType, DiffService};
use bitfun_services_core::system::check_command;

#[test]
fn diff_service_preserves_line_count_contract() {
    let service = DiffService::new(DiffConfig::new());
    let result = service.compute_diff("one\ntwo\n", "one\nthree\n");

    assert_eq!(result.additions, 1);
    assert_eq!(result.deletions, 1);
    assert_eq!(result.changes, 2);
    assert_eq!(result.hunks.len(), 1);
    assert!(result
        .hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .any(|line| line.line_type == DiffLineType::Add && line.content == "three"));
}

#[test]
fn system_check_command_preserves_missing_command_shape() {
    let result = check_command("__bitfun_missing_command_for_services_core_test__");

    assert!(!result.exists);
    assert_eq!(result.path, None);
}
