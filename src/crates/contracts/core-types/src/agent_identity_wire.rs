//! Compatibility adapter for the existing peer/mobile wire dialect.
//! Domain records always use canonical IDs; supported older clients still speak v0.
use crate::agent_identity::{canonical_agent_config_id, canonical_agent_id, HarnessId};
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
pub enum AgentIdentityDialect {
    Canonical,
    Legacy,
}

pub fn serialize_legacy_agent_id<S: serde::Serializer>(
    id: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&identity(id, AgentIdentityDialect::Legacy, false))
}

fn identity(id: &str, dialect: AgentIdentityDialect, profile: bool) -> String {
    let canonical = if profile {
        canonical_agent_config_id(id)
    } else {
        canonical_agent_id(id)
    };
    match dialect {
        AgentIdentityDialect::Canonical => canonical.to_owned(),
        AgentIdentityDialect::Legacy if profile && canonical == "Standard" => {
            "coding_shared".into()
        }
        AgentIdentityDialect::Legacy => HarnessId::from_legacy_id(canonical)
            .map(|id| id.legacy_wire_id())
            .unwrap_or(canonical)
            .to_owned(),
    }
}

/// Only product identity fields are translated. Prompt text, tool arguments,
/// user metadata, external definitions, and arbitrary config values are opaque.
pub fn translate_agent_identity_fields(
    value: &mut Value,
    dialect: AgentIdentityDialect,
) -> Result<(), String> {
    match value {
        Value::Array(items) => {
            for item in items {
                translate_agent_identity_fields(item, dialect)?;
            }
        }
        Value::Object(object) => {
            if object
                .get("source")
                .and_then(Value::as_str)
                .is_some_and(|s| s != "builtin")
            {
                return Ok(());
            }
            let catalog_entry =
                object.contains_key("toolCount") || object.contains_key("defaultTools");
            for (key, value) in object {
                if matches!(key.as_str(), "agent_profiles" | "ai.agent_profiles") {
                    translate_profile_map(value, dialect)?;
                    continue;
                }
                let profile = matches!(key.as_str(), "configProfileId" | "profile_id");
                let agent = matches!(
                    key.as_str(),
                    "agentType"
                        | "agent_type"
                        | "parentAgentType"
                        | "parent_agent_type"
                        | "modeId"
                        | "mode_id"
                        | "default_mode_id"
                        | "last_mode_id"
                        | "lastUserDialogAgentType"
                        | "last_user_dialog_agent_type"
                        | "lastSubmittedAgentType"
                        | "last_submitted_agent_type"
                ) || (catalog_entry && key == "id");
                if profile || agent {
                    if let Some(id) = value.as_str() {
                        *value = Value::String(identity(id, dialect, profile));
                    }
                } else if matches!(
                    key.as_str(),
                    "configProfileMemberModeIds" | "allowedParentAgentIds" | "deniedParentAgentIds"
                ) {
                    if let Some(ids) = value.as_array_mut() {
                        for id in ids {
                            if let Some(text) = id.as_str() {
                                *id = Value::String(identity(text, dialect, false));
                            }
                        }
                    }
                } else if !matches!(
                    key.as_str(),
                    "content"
                        | "prompt"
                        | "arguments"
                        | "args"
                        | "input"
                        | "output"
                        | "metadata"
                        | "userMessageMetadata"
                        | "user_message_metadata"
                        | "value"
                ) {
                    translate_agent_identity_fields(value, dialect)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn merge_profile_records(previous: &mut Value, incoming: Value) -> Result<(), String> {
    if previous == &incoming || incoming.is_null() {
        return Ok(());
    }
    if previous.is_null() {
        *previous = incoming;
        return Ok(());
    }
    if let (Value::Object(previous), Value::Object(incoming)) = (previous, incoming) {
        for (key, value) in incoming {
            if let Some(existing) = previous.get_mut(&key) {
                merge_profile_records(existing, value)?;
            } else {
                previous.insert(key, value);
            }
        }
        return Ok(());
    }
    Err("Conflicting Agent profile aliases; original configuration was preserved".into())
}

fn translate_profile(value: &mut Value, dialect: AgentIdentityDialect) {
    if let Some(id) = value.get_mut("profile_id") {
        if let Some(text) = id.as_str() {
            *id = Value::String(identity(text, dialect, true));
        }
    }
}

fn translate_profile_map(value: &mut Value, dialect: AgentIdentityDialect) -> Result<(), String> {
    if let Some(profiles) = value.as_object() {
        let mut translated = Map::new();
        for (id, config) in profiles {
            let key = identity(id, dialect, true);
            let mut config = config.clone();
            translate_profile(&mut config, dialect);
            if let Some(existing) = translated.get_mut(&key) {
                merge_profile_records(existing, config)?;
            } else {
                translated.insert(key, config);
            }
        }
        *value = Value::Object(translated);
    }
    Ok(())
}

fn config_path(path: &str, dialect: AgentIdentityDialect) -> String {
    let Some(rest) = path.strip_prefix("ai.agent_profiles.") else {
        return path.into();
    };
    let (id, suffix) = rest.split_once('.').unwrap_or((rest, ""));
    let suffix = if suffix.is_empty() {
        String::new()
    } else {
        format!(".{suffix}")
    };
    format!("ai.agent_profiles.{}{suffix}", identity(id, dialect, true))
}

fn config_value(
    path: &str,
    value: &mut Value,
    dialect: AgentIdentityDialect,
) -> Result<(), String> {
    if path == "ai.agent_profiles" {
        return translate_profile_map(value, dialect);
    }
    if matches!(
        path,
        "app.flow_chat.default_mode_id" | "app.flow_chat.last_mode_id"
    ) {
        if let Some(id) = value.as_str() {
            *value = Value::String(identity(id, dialect, false));
        }
    } else if let Some(rest) = path.strip_prefix("ai.agent_profiles.") {
        match rest.split_once('.') {
            None => translate_profile(value, dialect),
            Some((_, "profile_id")) => {
                if let Some(id) = value.as_str() {
                    *value = Value::String(identity(id, dialect, true));
                }
            }
            _ => {}
        }
    } else if matches!(path, "" | "app" | "app.flow_chat" | "ai") {
        translate_agent_identity_fields(value, dialect)?;
    }
    Ok(())
}

pub fn translate_agent_identity_command(
    command: &str,
    mut value: Value,
    dialect: AgentIdentityDialect,
) -> Result<Value, String> {
    translate_agent_identity_fields(&mut value, dialect)?;
    if matches!(command, "set_config" | "get_config" | "get_configs") {
        let request = if value.get("request").is_some() {
            value.get_mut("request").unwrap()
        } else {
            &mut value
        };
        if let Some(request) = request.as_object_mut() {
            let path = request
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if command == "set_config" {
                if let Some(config) = request.get_mut("value") {
                    config_value(&path, config, dialect)?;
                }
            }
            if request.contains_key("path") {
                request.insert("path".into(), Value::String(config_path(&path, dialect)));
            }
            if command == "get_configs" {
                if let Some(paths) = request.get_mut("paths").and_then(Value::as_array_mut) {
                    for path in paths {
                        if let Some(text) = path.as_str() {
                            *path = Value::String(config_path(text, dialect));
                        }
                    }
                }
            }
        }
    }
    Ok(value)
}

/// A scoped config reply may be a scalar, profile record, or keyed profile map.
pub fn translate_agent_identity_response(
    command: &str,
    mut value: Value,
    args: &Value,
    dialect: AgentIdentityDialect,
) -> Result<Value, String> {
    if command == "get_agent_profile_configs" {
        translate_profile_map(&mut value, dialect)?;
    } else if command == "get_config" {
        let request = args.get("request").unwrap_or(args);
        let path = request
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        config_value(path, &mut value, dialect)?;
    } else if command == "get_configs" {
        if let Some(configs) = value.as_object_mut() {
            let mut translated = Map::new();
            for (path, mut config) in std::mem::take(configs) {
                config_value(&path, &mut config, dialect)?;
                translated.insert(config_path(&path, dialect), config);
            }
            *configs = translated;
        }
    } else {
        translate_agent_identity_fields(&mut value, dialect)?;
    }
    Ok(value)
}
