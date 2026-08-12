use bitfun_product_domains::external_hook_contributions::{
    ExternalHookContributionDeclaration, ExternalHookContributionId, ExternalHookPoint,
    ExternalHookRiskCapability, ExternalHookSafetyDeclaration,
};
use std::collections::BTreeSet;

#[test]
fn contribution_identity_is_open_but_validated() {
    let id = ExternalHookContributionId::new("opencode-hook-contribution-v1:abc").unwrap();
    assert_eq!(id.as_str(), "opencode-hook-contribution-v1:abc");
    assert!(ExternalHookContributionId::new("").is_err());
    assert!(ExternalHookContributionId::new("hook\nidentity").is_err());
}

#[test]
fn static_declaration_preserves_source_hook_and_incomplete_safety_facts() {
    let declaration = ExternalHookContributionDeclaration {
        contribution_id: ExternalHookContributionId::new("hook-before").unwrap(),
        hook_point: ExternalHookPoint::ToolBefore,
        safety: ExternalHookSafetyDeclaration {
            declared_risks: BTreeSet::from([
                ExternalHookRiskCapability::ReadToolArguments,
                ExternalHookRiskCapability::ModifyToolArguments,
            ]),
            complete: false,
        },
    };

    assert_eq!(declaration.hook_point.as_str(), "tool_before");
    assert!(!declaration.safety.complete);
}

#[test]
fn diagnostic_labels_are_closed_and_low_cardinality() {
    assert_eq!(ExternalHookPoint::ToolBefore.as_str(), "tool_before");
    assert_eq!(ExternalHookPoint::ToolAfter.as_str(), "tool_after");
    assert_eq!(
        ExternalHookRiskCapability::ReadToolArguments.as_str(),
        "read_tool_arguments"
    );
    assert_eq!(
        ExternalHookRiskCapability::ModifyToolArguments.as_str(),
        "modify_tool_arguments"
    );
    assert_eq!(
        ExternalHookRiskCapability::ReadToolResult.as_str(),
        "read_tool_result"
    );
    assert_eq!(
        ExternalHookRiskCapability::ModifyToolResult.as_str(),
        "modify_tool_result"
    );
}
