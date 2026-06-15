//! MCP config service data helpers.

use crate::mcp::server::MCPServerConfig;
use log::warn;
use std::collections::{BTreeMap, HashMap};

const AUTHORIZATION_KEYS: [&str; 3] = ["Authorization", "authorization", "AUTHORIZATION"];

fn config_signature(config: &MCPServerConfig) -> String {
    let env: BTreeMap<_, _> = config.env.clone().into_iter().collect();
    let headers: BTreeMap<_, _> = config.headers.clone().into_iter().collect();
    serde_json::json!({
        "serverType": config.server_type,
        "transport": config.resolved_transport().as_str(),
        "command": config.command,
        "args": config.args,
        "env": env,
        "headers": headers,
        "url": config.url,
        "oauth": config.oauth,
        "xaa": config.xaa,
    })
    .to_string()
}

fn precedence(location: super::ConfigLocation) -> u8 {
    match location {
        super::ConfigLocation::BuiltIn => 0,
        super::ConfigLocation::User => 1,
        super::ConfigLocation::Project => 2,
    }
}

pub fn merge_mcp_server_config_source(
    merged: &mut Vec<MCPServerConfig>,
    source: Vec<MCPServerConfig>,
    signature_index: &mut HashMap<String, usize>,
    id_index: &mut HashMap<String, usize>,
) {
    for config in source {
        let config_id = config.id.clone();
        let signature = config_signature(&config);

        if let Some(existing_index) = id_index.get(&config_id).copied() {
            let previous = &merged[existing_index];
            warn!(
                "Overriding MCP config by id: id={} previous_location={:?} new_location={:?}",
                config_id, previous.location, config.location
            );

            let previous_signature = config_signature(previous);
            merged[existing_index] = config;
            signature_index.remove(&previous_signature);
            signature_index.insert(signature, existing_index);
            continue;
        }

        if let Some(existing_index) = signature_index.get(&signature).copied() {
            let previous = &merged[existing_index];
            if precedence(previous.location) <= precedence(config.location) {
                warn!(
                    "Deduplicating MCP config by content signature: previous_id={} previous_location={:?} replacement_id={} replacement_location={:?}",
                    previous.id, previous.location, config_id, config.location
                );

                id_index.remove(&previous.id);
                merged[existing_index] = config;
                id_index.insert(config_id, existing_index);
                signature_index.insert(signature, existing_index);
            }
            continue;
        }

        let next_index = merged.len();
        signature_index.insert(signature, next_index);
        id_index.insert(config_id, next_index);
        merged.push(config);
    }
}

pub fn merge_mcp_server_config_sources<I>(sources: I) -> Vec<MCPServerConfig>
where
    I: IntoIterator<Item = Vec<MCPServerConfig>>,
{
    let mut configs = Vec::new();
    let mut signature_index = HashMap::new();
    let mut id_index = HashMap::new();

    for source in sources {
        merge_mcp_server_config_source(&mut configs, source, &mut signature_index, &mut id_index);
    }

    configs
}

pub fn parse_mcp_config_array(
    servers: &[serde_json::Value],
    location: super::ConfigLocation,
) -> Vec<MCPServerConfig> {
    servers
        .iter()
        .filter_map(
            |value| match serde_json::from_value::<MCPServerConfig>(value.clone()) {
                Ok(mut config) => {
                    config.location = location;
                    Some(config)
                }
                Err(e) => {
                    warn!(
                        "Failed to parse MCP config item at {:?} scope: {}",
                        location, e
                    );
                    None
                }
            },
        )
        .collect()
}

pub fn normalize_mcp_authorization_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.to_ascii_lowercase().starts_with("bearer ") || trimmed.contains(char::is_whitespace)
    {
        return Some(trimmed.to_string());
    }

    Some(format!("Bearer {}", trimmed))
}

fn authorization_from_map(map: &HashMap<String, String>) -> Option<String> {
    AUTHORIZATION_KEYS
        .iter()
        .find_map(|key| map.get(*key).cloned())
        .filter(|value| !value.trim().is_empty())
}

pub fn remove_mcp_authorization_keys(map: &mut HashMap<String, String>) {
    for key in AUTHORIZATION_KEYS {
        map.remove(key);
    }
}

pub fn get_mcp_remote_authorization_value(config: &MCPServerConfig) -> Option<String> {
    authorization_from_map(&config.headers).or_else(|| authorization_from_map(&config.env))
}

pub fn get_mcp_remote_authorization_source(config: &MCPServerConfig) -> Option<&'static str> {
    if authorization_from_map(&config.headers).is_some() {
        Some("headers")
    } else if authorization_from_map(&config.env).is_some() {
        Some("env")
    } else {
        None
    }
}

pub fn has_mcp_remote_authorization(config: &MCPServerConfig) -> bool {
    get_mcp_remote_authorization_value(config).is_some()
}

pub fn has_mcp_remote_oauth(config: &MCPServerConfig) -> bool {
    config.oauth.is_some()
}

pub fn has_mcp_remote_xaa(config: &MCPServerConfig) -> bool {
    config.xaa.is_some()
}
