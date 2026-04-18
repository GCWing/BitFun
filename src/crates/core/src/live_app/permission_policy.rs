//! Permission policy — resolve manifest permissions to JSON policy for JS Worker.

use crate::live_app::types::{LiveAppPermissions, PathScope};
use serde_json::{Map, Value};
use std::path::Path;

/// Resolve permission manifest to a JSON policy object passed to the Worker as startup argument.
/// Path variables {appdata}, {workspace}, {home} are resolved to absolute paths.
/// `granted_paths` are user-granted paths (e.g. from grant_path) to include in read+write.
pub fn resolve_policy(
    perms: &LiveAppPermissions,
    app_id: &str,
    app_data_dir: &Path,
    workspace_dir: Option<&Path>,
    granted_paths: &[std::path::PathBuf],
) -> Value {
    let mut policy = Map::new();

    if let Some(ref fs) = perms.fs {
        let read = resolve_fs_scopes(
            fs.read.as_deref().unwrap_or(&[]),
            app_id,
            app_data_dir,
            workspace_dir,
        );
        let write = resolve_fs_scopes(
            fs.write.as_deref().unwrap_or(&[]),
            app_id,
            app_data_dir,
            workspace_dir,
        );
        let mut read_paths: Vec<String> = read.into_iter().collect();
        let mut write_paths: Vec<String> = write.into_iter().collect();
        for gp in granted_paths {
            if let Some(s) = gp.to_str() {
                read_paths.push(s.to_string());
                write_paths.push(s.to_string());
            }
        }
        if !read_paths.is_empty() || !write_paths.is_empty() {
            let mut fs_map = Map::new();
            fs_map.insert(
                "read".to_string(),
                Value::Array(read_paths.into_iter().map(Value::String).collect()),
            );
            fs_map.insert(
                "write".to_string(),
                Value::Array(write_paths.into_iter().map(Value::String).collect()),
            );
            policy.insert("fs".to_string(), Value::Object(fs_map));
        }
    }

    if let Some(ref shell) = perms.shell {
        let allow = shell
            .allow
            .as_ref()
            .map(|v| Value::Array(v.iter().map(|s| Value::String(s.clone())).collect()))
            .unwrap_or_else(|| Value::Array(Vec::new()));
        policy.insert("shell".to_string(), serde_json::json!({ "allow": allow }));
    }

    if let Some(ref net) = perms.net {
        let allow = net
            .allow
            .as_ref()
            .map(|v| Value::Array(v.iter().map(|s| Value::String(s.clone())).collect()))
            .unwrap_or_else(|| Value::Array(Vec::new()));
        policy.insert("net".to_string(), serde_json::json!({ "allow": allow }));
    }

    Value::Object(policy)
}

fn resolve_fs_scopes(
    scopes: &[String],
    _app_id: &str,
    app_data_dir: &Path,
    workspace_dir: Option<&Path>,
) -> Vec<String> {
    let mut result = Vec::with_capacity(scopes.len());
    for s in scopes {
        let scope = PathScope::from_manifest_value(s);
        let paths = match &scope {
            PathScope::AppData => vec![app_data_dir.to_path_buf()],
            PathScope::Workspace => workspace_dir.map(|p| p.to_path_buf()).into_iter().collect(),
            PathScope::UserSelected | PathScope::Home => {
                if let PathScope::Home = scope {
                    dirs::home_dir().into_iter().collect()
                } else {
                    Vec::new()
                }
            }
            PathScope::Custom(paths) => paths.clone(),
        };
        for p in paths {
            if let Some(s) = p.to_str() {
                result.push(s.to_string());
            }
        }
    }
    result
}
