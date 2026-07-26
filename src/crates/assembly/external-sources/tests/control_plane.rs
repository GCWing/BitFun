use bitfun_external_sources::ExternalSourceControlPlane;
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalMcpRevisionKey, ExternalSourceContext,
};
use std::collections::BTreeSet;

fn context() -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: None,
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn revision_key() -> ExternalMcpRevisionKey {
    ExternalMcpRevisionKey::new([7; 32])
}

#[test]
fn control_plane_owns_all_typed_coordinator_snapshots() {
    let plane = ExternalSourceControlPlane::new(
        context(),
        revision_key(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();

    assert!(plane.commands(|coordinator| coordinator.snapshot().sources.is_empty()));
    assert!(plane.tools(|coordinator| coordinator.snapshot().sources.is_empty()));
    assert!(plane.subagents(|coordinator| coordinator.snapshot().sources.is_empty()));
    assert!(plane.mcp(|coordinator| coordinator.snapshot().sources.is_empty()));
}

#[test]
fn suppression_replacement_is_applied_to_every_typed_coordinator() {
    let plane = ExternalSourceControlPlane::new(
        context(),
        revision_key(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let suppressed = ["source-key".to_string()]
        .into_iter()
        .collect::<BTreeSet<_>>();

    plane.replace_suppressed_sources(suppressed.clone());

    assert_eq!(
        plane.commands(|coordinator| coordinator.suppressed_sources().clone()),
        suppressed
    );
    assert_eq!(
        plane.tools(|coordinator| coordinator.suppressed_sources().clone()),
        suppressed
    );
    assert_eq!(
        plane.subagents(|coordinator| coordinator.suppressed_sources().clone()),
        suppressed
    );
    assert_eq!(
        plane.mcp(|coordinator| coordinator.suppressed_sources().clone()),
        suppressed
    );
}
