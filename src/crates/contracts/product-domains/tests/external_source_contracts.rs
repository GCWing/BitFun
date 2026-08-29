#![cfg(feature = "external-sources")]

#[path = "external_source_contracts/external_hook_catalog_contracts.rs"]
mod external_hook_catalog_contracts;
#[path = "external_source_contracts/external_hook_contribution_contracts.rs"]
mod external_hook_contribution_contracts;
#[path = "external_source_contracts/external_source_contracts.rs"]
mod external_source_contracts;
#[path = "external_source_contracts/plugin_capability_contracts.rs"]
mod plugin_capability_contracts;
#[path = "external_source_contracts/workspace_reference_contracts.rs"]
mod workspace_reference_contracts;
