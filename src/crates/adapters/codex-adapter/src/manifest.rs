//! Codex `.codex-plugin/plugin.json` manifest parsing.
//!
//! Parses plugin.json files according to the Codex plugin specification.
//! This module is read-only — it only deserializes and validates manifests.
//! Plugin execution (hooks, MCP server startup) belongs in the assembly layer.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Errors that can occur during manifest parsing.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("I/O error reading manifest: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

/// The raw plugin.json structure as deserialized from disk.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPluginManifest {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<PluginAuthor>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub skills: Option<SkillPathValue>,
    #[serde(default)]
    pub hooks: Option<HookPathValue>,
    #[serde(default)]
    pub mcp_servers: Option<McpServersValue>,
    #[serde(default)]
    pub apps: Option<String>,
    #[serde(default)]
    pub interface: Option<PluginInterface>,
}

/// Skills field: single path or array of paths.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SkillPathValue {
    Single(String),
    Multiple(Vec<String>),
}

impl<'de> Deserialize<'de> for SkillPathValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = SkillPathValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string or array of strings")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(SkillPathValue::Single(v.to_string()))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut s: A) -> Result<Self::Value, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = s.next_element::<String>()? { items.push(item); }
                Ok(SkillPathValue::Multiple(items))
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl SkillPathValue {
    pub fn as_paths(&self) -> Vec<&str> {
        match self { Self::Single(s) => vec![s], Self::Multiple(v) => v.iter().map(|s| s.as_str()).collect() }
    }
}

/// Hooks field: path, array of paths, or inline object.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum HookPathValue {
    Single(String),
    Multiple(Vec<String>),
    Inline(serde_json::Value),
}

impl<'de> Deserialize<'de> for HookPathValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = HookPathValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string, array of strings, or inline hooks object")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(HookPathValue::Single(v.to_string()))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut s: A) -> Result<Self::Value, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = s.next_element::<String>()? { items.push(item); }
                Ok(HookPathValue::Multiple(items))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<Self::Value, A::Error> {
                use serde::de::value::MapAccessDeserializer;
                Ok(HookPathValue::Inline(serde_json::Value::deserialize(MapAccessDeserializer::new(m))?))
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl HookPathValue {
    pub fn as_paths(&self) -> Vec<&str> {
        match self { Self::Single(s) => vec![s], Self::Multiple(v) => v.iter().map(|s| s.as_str()).collect(), Self::Inline(_) => vec![] }
    }
    pub fn as_inline(&self) -> Option<&serde_json::Value> {
        match self { Self::Inline(v) => Some(v), _ => None }
    }
}

/// MCP servers field: path or inline object.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum McpServersValue {
    Path(String),
    Inline(serde_json::Map<String, serde_json::Value>),
}

impl<'de> Deserialize<'de> for McpServersValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = McpServersValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string path or an inline MCP server object")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(McpServersValue::Path(v.to_string()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<Self::Value, A::Error> {
                use serde::de::value::MapAccessDeserializer;
                let v = serde_json::Value::deserialize(MapAccessDeserializer::new(m))?;
                Ok(McpServersValue::Inline(v.as_object().cloned().unwrap_or_default()))
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAuthor {
    #[serde(default)] pub name: Option<String>,
    #[serde(default)] pub email: Option<String>,
    #[serde(default)] pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInterface {
    #[serde(default)] pub display_name: Option<String>,
    #[serde(default)] pub short_description: Option<String>,
    #[serde(default)] pub long_description: Option<String>,
    #[serde(default)] pub developer_name: Option<String>,
    #[serde(default)] pub category: Option<String>,
    #[serde(default)] pub capabilities: Vec<String>,
    #[serde(default)] pub website_url: Option<String>,
    #[serde(default)] pub privacy_policy_url: Option<String>,
    #[serde(default)] pub terms_of_service_url: Option<String>,
    #[serde(default)] pub default_prompt: Vec<String>,
    #[serde(default)] pub brand_color: Option<String>,
    #[serde(default)] pub composer_icon: Option<String>,
    #[serde(default)] pub logo: Option<String>,
    #[serde(default)] pub logo_dark: Option<String>,
    #[serde(default)] pub screenshots: Vec<String>,
}

/// Parsed plugin manifest with resolved paths.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<PluginAuthor>,
    pub keywords: Vec<String>,
    pub skill_paths: Vec<String>,
    pub hook_paths: Vec<String>,
    pub hooks_inline: Option<serde_json::Value>,
    pub mcp_servers: Option<McpServersValue>,
    pub app_path: Option<String>,
    pub interface: Option<PluginInterface>,
}

pub const MANIFEST_PATHS: &[&str] = &[".codex-plugin/plugin.json", ".claude-plugin/plugin.json"];

pub fn parse_manifest(path: &Path) -> Result<PluginManifest, ManifestError> {
    parse_manifest_str(&std::fs::read_to_string(path)?)
}

pub fn parse_manifest_str(content: &str) -> Result<PluginManifest, ManifestError> {
    let raw: RawPluginManifest = serde_json::from_str(content)?;
    if raw.name.is_empty() {
        return Err(ManifestError::Validation("'name' is required".to_string()));
    }
    let skill_paths = raw.skills.as_ref()
        .map(|s| s.as_paths().iter().map(|p| p.to_string()).collect())
        .unwrap_or_else(|| vec!["./skills/".to_string()]);
    let (hook_paths, hooks_inline) = match &raw.hooks {
        Some(HookPathValue::Inline(v)) => (vec![], Some(v.clone())),
        Some(other) => (other.as_paths().iter().map(|p| p.to_string()).collect(), None),
        None => (vec![], None),
    };
    Ok(PluginManifest {
        name: raw.name, version: raw.version, description: raw.description,
        author: raw.author, keywords: raw.keywords,
        skill_paths, hook_paths, hooks_inline,
        mcp_servers: raw.mcp_servers, app_path: raw.apps,
        interface: raw.interface,
    })
}

pub fn find_manifest_in_root(plugin_root: &Path) -> Option<std::path::PathBuf> {
    for name in MANIFEST_PATHS {
        let p = plugin_root.join(name);
        if p.exists() { return Some(p); }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_manifest() {
        let m = parse_manifest_str(r#"{"name":"my-plugin"}"#).unwrap();
        assert_eq!(m.name, "my-plugin");
        assert_eq!(m.skill_paths, vec!["./skills/"]);
    }

    #[test]
    fn test_full_manifest() {
        let m = parse_manifest_str(r#"{"name":"p","version":"1.0.0","description":"d","skills":["./a/","./b/"],"hooks":"./h.json","interface":{"displayName":"P","category":"X"}}"#).unwrap();
        assert_eq!(m.name, "p");
        assert_eq!(m.version, Some("1.0.0".into()));
        assert_eq!(m.skill_paths.len(), 2);
        assert_eq!(m.hook_paths, vec!["./h.json"]);
    }

    #[test]
    fn test_inline_mcp() {
        let m = parse_manifest_str(r#"{"name":"m","mcpServers":{"s":{"type":"http","url":"https://e.com"}}}"#).unwrap();
        match &m.mcp_servers {
            Some(McpServersValue::Inline(map)) => assert!(map.contains_key("s")),
            _ => panic!("expected inline MCP"),
        }
    }

    #[test]
    fn test_inline_hooks_empty_object() {
        // superpowers plugin uses "hooks": {}
        let m = parse_manifest_str(r#"{"name":"p","hooks":{}}"#).unwrap();
        assert!(m.hook_paths.is_empty());
        assert!(m.hooks_inline.is_some());
    }

    #[test]
    fn test_empty_name_fails() {
        assert!(parse_manifest_str(r#"{"name":""}"#).is_err());
    }

    #[test]
    fn test_invalid_json_fails() {
        assert!(parse_manifest_str("not json").is_err());
    }
}
