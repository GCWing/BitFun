use openbitfun_product_domains::plugin_capabilities::{
    PluginCapabilityProjection, PluginContributorIdentity, PluginToolRef,
};

#[test]
fn contributor_ownership_and_behavior_identities_remain_distinct() {
    let first = PluginContributorIdentity::new("owner:first", "shared-behavior", "Plugin");
    let second = PluginContributorIdentity::new("owner:second", "shared-behavior", "Plugin");

    assert_ne!(first, second);
    assert_eq!(first.behavior_key(), second.behavior_key());
    assert_eq!(first.label(), "Plugin");
}

#[test]
fn tool_references_are_provider_neutral_and_owner_scoped() {
    let contributor = PluginContributorIdentity::new(
        "owner:second-ecosystem",
        "second-plugin-behavior",
        "Second ecosystem plugin",
    );
    let tool = PluginToolRef::new(contributor.clone(), "run_code");
    let projection = PluginCapabilityProjection::default();

    assert_eq!(tool.contributor(), &contributor);
    assert_eq!(tool.id(), "run_code");
    assert!(projection.agents.is_empty());
    assert!(projection.skill_roots.is_empty());
}
