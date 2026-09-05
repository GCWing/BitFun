//! Small, build-free UI customizations layered over the installed frontend.

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

pub(super) const CSS: &str = "openbitfun-creation.css";
pub(super) const JS: &str = "openbitfun-creation.js";
pub(super) const API_DOC: &str = "openbitfun-creation-api.md";
pub(super) const MANIFEST: &str = ".creation-overlay.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DraftBase {
    pub base_revision: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayManifest {
    api_version: u32,
}

pub(super) fn is_overlay(root: &Path) -> bool {
    fs::read(root.join(MANIFEST))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<OverlayManifest>(&bytes).ok())
        .is_some_and(|manifest| manifest.api_version == 1)
}

pub(super) fn write_manifest(root: &Path) -> Result<(), String> {
    fs::write(root.join(MANIFEST), b"{\"apiVersion\":1}\n")
        .map_err(|error| format!("Failed to write frontend overlay manifest: {error}"))
}

pub(super) fn owns_asset(relative: &Path) -> bool {
    relative == Path::new(CSS)
        || relative == Path::new(JS)
        || relative.starts_with("creation-assets")
}

pub(super) fn validate(root: &Path) -> Result<(), String> {
    if fs::symlink_metadata(root)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Frontend customization roots cannot be symbolic links".to_string());
    }
    for file in [CSS, JS] {
        if !root.join(file).is_file() {
            return Err(format!("Frontend customization is missing {file}"));
        }
    }
    super::visit_tree(root, &mut |path, metadata| {
        if metadata.file_type().is_symlink() {
            return Err("Frontend customizations cannot contain symbolic links".to_string());
        }
        let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
        if !owns_asset(relative)
            && !["CREATION.md", API_DOC, MANIFEST]
                .iter()
                .any(|name| relative == Path::new(name))
        {
            return Err(format!(
                "Unsupported customization file: {}. Edit the CSS/JS entrypoints or creation-assets; the installed application is supplied by the host",
                relative.display()
            ));
        }
        Ok(())
    })
}
