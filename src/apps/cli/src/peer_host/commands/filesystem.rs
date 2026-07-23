//! Filesystem HostInvoke handlers.

use serde_json::{json, Value};

use crate::peer_host::args::{get_string, optional_string, request_value};
use crate::peer_host::state::PeerHostState;

fn directory_nodes_to_json(nodes: Vec<bitfun_core::infrastructure::FileTreeNode>) -> Vec<Value> {
    nodes
        .into_iter()
        .map(|node| {
            json!({
                "path": node.path,
                "name": node.name,
                "isDirectory": node.is_directory,
                "size": node.size,
                "extension": node.extension,
                "lastModified": node.last_modified
            })
        })
        .collect()
}

pub(crate) async fn get_directory_children(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let path = get_string(request, "path")?;
    let preferred = optional_string(request, "remoteConnectionId");
    let nodes = state
        .filesystem_service
        .get_directory_contents_with_remote_hint(&path, preferred.as_deref())
        .await
        .map_err(|e| format!("Failed to get directory children: {e}"))?;
    Ok(json!(directory_nodes_to_json(nodes)))
}

pub(crate) async fn get_directory_children_paginated(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let path = get_string(request, "path")?;
    let preferred = optional_string(request, "remoteConnectionId");
    let offset = request.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let limit = request.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    let nodes = state
        .filesystem_service
        .get_directory_contents_with_remote_hint(&path, preferred.as_deref())
        .await
        .map_err(|e| format!("Failed to get paginated directory children: {e}"))?;
    let total = nodes.len();
    let has_more = total > offset + limit;
    let page_nodes: Vec<_> = nodes.into_iter().skip(offset).take(limit).collect();

    Ok(json!({
        "children": directory_nodes_to_json(page_nodes),
        "total": total,
        "hasMore": has_more,
        "offset": offset,
        "limit": limit
    }))
}

pub(crate) async fn check_path_exists(args: &Value) -> Result<Value, String> {
    let path_str = if let Some(req) = args.get("request") {
        get_string(req, "path")?
    } else {
        get_string(args, "path")?
    };
    Ok(json!(std::path::Path::new(&path_str).exists()))
}

pub(crate) async fn create_directory(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let path = get_string(request, "path")?;
    state
        .filesystem_service
        .create_directory(&path)
        .await
        .map_err(|e| format!("Failed to create directory: {e}"))?;
    Ok(Value::Null)
}
