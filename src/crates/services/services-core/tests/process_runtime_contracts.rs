#![cfg(feature = "process-runtime")]

use openbitfun_services_core::system::check_command;

#[test]
fn system_check_command_preserves_missing_command_shape() {
    let result = check_command("__openbitfun_missing_command_for_services_core_test__");

    assert!(!result.exists);
    assert_eq!(result.path, None);
}
