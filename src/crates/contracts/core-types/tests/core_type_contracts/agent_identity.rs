use openbitfun_core_types::agent_identity::{
    canonical_agent_config_id, canonical_agent_id, HarnessId,
};

#[test]
fn legacy_harness_ids_read_as_canonical_and_write_only_canonical_ids() {
    for (legacy, canonical) in [
        ("minimal", "Minimal"),
        ("balanced", "Standard"),
        ("agentic", "Standard"),
        ("Agentic", "Standard"),
        ("Ultra", "Ultimate"),
        ("ultimate", "Ultimate"),
        ("creative", "Creative"),
    ] {
        let parsed: HarnessId = serde_json::from_value(serde_json::json!(legacy)).unwrap();
        assert_eq!(parsed.as_str(), canonical);
        assert_eq!(serde_json::to_value(parsed).unwrap(), canonical);
        assert_eq!(
            HarnessId::from_legacy_id(parsed.legacy_wire_id()),
            Some(parsed)
        );
    }
}

#[test]
fn unknown_and_source_qualified_agent_identity_survives_normalization() {
    for id in [
        "Claw",
        "DeepResearch",
        "custom::agentic",
        "external::Ultra",
        "FutureHarness",
    ] {
        assert_eq!(canonical_agent_id(id), id);
    }
    assert_eq!(canonical_agent_config_id("coding_shared"), "Standard");
    assert_eq!(canonical_agent_id("coding_shared"), "coding_shared");
}

#[test]
fn peer_wire_round_trip_changes_only_product_identity_fields() {
    use openbitfun_core_types::agent_identity_wire::{
        translate_agent_identity_command, AgentIdentityDialect,
    };
    let current = serde_json::json!({"request": {
        "agentType": "Standard", "modeId": "Ultimate",
        "config": {"profile_id": "Standard"},
        "userMessageMetadata": {"agentType": "Standard"},
        "arguments": {"agentType": "Standard"},
        "prompt": "agentic Ultra Minimal"
    }});
    let wire = translate_agent_identity_command(
        "start_dialog_turn",
        current.clone(),
        AgentIdentityDialect::Legacy,
    )
    .unwrap();
    assert_eq!(wire["request"]["agentType"], "agentic");
    assert_eq!(wire["request"]["modeId"], "Ultra");
    assert_eq!(wire["request"]["config"]["profile_id"], "coding_shared");
    assert_eq!(wire["request"]["arguments"]["agentType"], "Standard");
    assert_eq!(
        wire["request"]["userMessageMetadata"]["agentType"],
        "Standard"
    );
    assert_eq!(
        translate_agent_identity_command(
            "start_dialog_turn",
            wire,
            AgentIdentityDialect::Canonical
        )
        .unwrap(),
        current
    );
    let external = serde_json::json!({"source":"external", "id":"agentic", "toolCount":2});
    assert_eq!(
        translate_agent_identity_command(
            "get_available_modes",
            external.clone(),
            AgentIdentityDialect::Canonical
        )
        .unwrap(),
        external
    );
}

#[test]
fn scoped_config_wire_contract_matches_all_surfaces() {
    use openbitfun_core_types::agent_identity_wire::{
        translate_agent_identity_command, translate_agent_identity_response, AgentIdentityDialect,
    };
    let fixtures: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../shared/agent-harness/wire-fixtures.json"
    )))
    .unwrap();
    for fixture in fixtures {
        let command = fixture["command"].as_str().unwrap();
        for (from, to, dialect) in [
            ("canonical", "legacy", AgentIdentityDialect::Legacy),
            ("legacy", "canonical", AgentIdentityDialect::Canonical),
        ] {
            assert_eq!(
                translate_agent_identity_command(command, fixture[from].clone(), dialect).unwrap(),
                fixture[to]
            );
            if let Some(response) = fixture.get(format!("{from}Response")) {
                assert_eq!(
                    translate_agent_identity_response(
                        command,
                        response.clone(),
                        &fixture[from],
                        dialect
                    )
                    .unwrap(),
                    fixture[format!("{to}Response")]
                );
            }
        }
    }
}

#[test]
fn conflicting_profile_aliases_fail_before_dispatch() {
    use openbitfun_core_types::agent_identity_wire::{
        translate_agent_identity_command, AgentIdentityDialect,
    };
    let request = serde_json::json!({"request": {"path":"ai.agent_profiles", "value": {
        "agentic": {"removed_tools": ["Bash"]}, "Standard": {"removed_tools": ["Read"]}
    }}});
    assert!(translate_agent_identity_command(
        "set_config",
        request,
        AgentIdentityDialect::Canonical
    )
    .is_err());
}
