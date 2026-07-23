//! Legion preset storage.
//!
//! Each preset is a JSON file under `<user-config>/legions/<id>.json` describing
//! a team topology (nodes + edges) that the Team mode agent can materialise at
//! runtime via SessionControl / SessionMessage.

use crate::infrastructure::get_path_manager_arc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const LEGIONS_SUBDIR: &str = "legions";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegionPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<LegionNode>,
    pub edges: Vec<LegionEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegionNode {
    pub id: String,
    pub agent: String,
    pub role: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegionEdge {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

fn legions_dir() -> PathBuf {
    get_path_manager_arc()
        .user_config_dir()
        .join(LEGIONS_SUBDIR)
}

fn preset_path(id: &str) -> Result<PathBuf, String> {
    validate_preset_id(id)?;
    Ok(legions_dir().join(format!("{id}.json")))
}

/// Validate preset id to prevent path traversal.
/// Allowed characters: alphanumeric, underscore, and hyphen.
fn validate_preset_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("Legion preset id must not be empty".to_string());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "Invalid legion preset id '{id}': only letters, digits, underscores, and hyphens are allowed"
        ));
    }
    Ok(())
}

fn ensure_legions_dir() -> std::io::Result<()> {
    let dir = legions_dir();
    std::fs::create_dir_all(&dir)
}

/// List all saved legion presets (sorted by id).
pub fn list_presets() -> Result<Vec<LegionPreset>, String> {
    let dir = legions_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("Failed to read legions dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let preset: LegionPreset = serde_json::from_str(&raw)
                .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
            out.push(preset);
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Load a single preset by id.
pub fn get_preset(id: &str) -> Result<LegionPreset, String> {
    let path = preset_path(id)?;
    if !path.is_file() {
        return Err(format!("Legion preset '{id}' not found"));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read preset: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse preset: {e}"))
}

/// Create or overwrite a preset.
pub fn create_preset(preset: &LegionPreset) -> Result<(), String> {
    ensure_legions_dir().map_err(|e| format!("Failed to create legions dir: {e}"))?;
    let path = preset_path(&preset.id)?;
    let raw =
        serde_json::to_string_pretty(preset).map_err(|e| format!("Failed to serialise: {e}"))?;
    std::fs::write(&path, raw).map_err(|e| format!("Failed to write preset: {e}"))
}

/// Update an existing preset (id must already exist).
pub fn update_preset(preset: &LegionPreset) -> Result<(), String> {
    let path = preset_path(&preset.id)?;
    if !path.is_file() {
        return Err(format!("Legion preset '{}' not found", preset.id));
    }
    let raw =
        serde_json::to_string_pretty(preset).map_err(|e| format!("Failed to serialise: {e}"))?;
    std::fs::write(&path, raw).map_err(|e| format!("Failed to write preset: {e}"))
}

/// Delete a preset by id.
pub fn delete_preset(id: &str) -> Result<(), String> {
    let path = preset_path(id)?;
    if !path.is_file() {
        return Err(format!("Legion preset '{id}' not found"));
    }
    std::fs::remove_file(&path).map_err(|e| format!("Failed to delete preset: {e}"))
}
