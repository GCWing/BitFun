//! Linux AT-SPI2 (via `atspi`) BFS over accessible objects for stable screen coordinates.
//!
//! Requires session D-Bus, `at-spi2` registry, and apps exposing AT-SPI (typical on GNOME/KDE with a11y).

use crate::computer_use::ui_locate_common;
use atspi::connection::P2P;
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::proxy_ext::ProxyExt;
use atspi::AccessibilityConnection;
use atspi::CoordType;
use bitfun_core::agentic::tools::computer_use_host::{UiElementLocateQuery, UiElementLocateResult};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use std::collections::VecDeque;

async fn component_extents_screen(acc: &AccessibleProxy<'_>) -> Option<(i32, i32, i32, i32)> {
    let proxies = acc.proxies().await.ok()?;
    let comp = proxies.component().await.ok()?;
    comp.get_extents(CoordType::Screen).await.ok()
}

async fn role_match_string(acc: &AccessibleProxy<'_>) -> String {
    match acc.get_role_name().await {
        Ok(s) if !s.is_empty() => s,
        _ => match acc.get_role().await {
            Ok(r) => format!("{:?}", r),
            Err(_) => String::new(),
        },
    }
}

/// Registry application roots → BFS until first match with non-empty screen extents.
pub async fn locate_ui_element_center(
    query: UiElementLocateQuery,
) -> BitFunResult<UiElementLocateResult> {
    ui_locate_common::validate_query(&query)?;

    if query.node_idx.is_some() {
        return Err(BitFunError::tool(
            "[AX_IDX_NOT_SUPPORTED] node_idx lookup is only implemented on macOS. \
             Fall back to `text_contains` / `title_contains` + `role_substring` on this host."
                .to_string(),
        ));
    }

    let max_depth = query.max_depth.unwrap_or(48).clamp(1, 200);
    let max_nodes = 12_000usize;

    let conn = AccessibilityConnection::new()
        .await
        .map_err(|e| BitFunError::tool(format!("AT-SPI connection: {}.", e)))?;

    let registry_root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| BitFunError::tool(format!("AT-SPI registry root: {}.", e)))?;

    let children = registry_root
        .get_children()
        .await
        .map_err(|e| BitFunError::tool(format!("AT-SPI get_children (registry): {}.", e)))?;

    let mut queue = VecDeque::new();
    for c in children {
        queue.push_back((c, 0u32));
    }

    let mut visited = 0usize;

    while let Some((obj_ref, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }
        visited += 1;
        if visited > max_nodes {
            return Err(BitFunError::tool(
                "AT-SPI search limit reached; narrow title/role/identifier filters.".to_string(),
            ));
        }

        let acc = match conn.object_as_accessible(&obj_ref).await {
            Ok(a) => a,
            Err(_) => continue,
        };

        let name = acc.name().await.unwrap_or_default();
        let ident = acc.accessible_id().await.unwrap_or_default();
        let role = role_match_string(&acc).await;
        let description = acc.description().await.unwrap_or_default();

        let attrs = ui_locate_common::NodeAttrs {
            role: Some(role.as_str()),
            subrole: None,
            title: Some(name.as_str()),
            value: None,
            description: if description.is_empty() {
                None
            } else {
                Some(description.as_str())
            },
            identifier: Some(ident.as_str()),
            help: None,
        };
        let matched = ui_locate_common::matches_filters_attrs(&query, &attrs);
        if matched {
            if let Some((x, y, w, h)) = component_extents_screen(&acc).await {
                if w > 0 && h > 0 {
                    let gx = x as f64 + w as f64 / 2.0;
                    let gy = y as f64 + h as f64 / 2.0;
                    let bl = x as f64;
                    let bt = y as f64;
                    let bw = w as f64;
                    let bh = h as f64;
                    return ui_locate_common::ok_result(
                        gx,
                        gy,
                        bl,
                        bt,
                        bw,
                        bh,
                        role,
                        if name.is_empty() { None } else { Some(name) },
                        if ident.is_empty() { None } else { Some(ident) },
                    );
                }
            }
        }

        let ch = match acc.get_children().await {
            Ok(c) => c,
            Err(_) => continue,
        };
        for child in ch {
            queue.push_back((child, depth + 1));
        }
    }

    Err(BitFunError::tool(
        "No AT-SPI accessible matched the query (try different substrings, enable desktop accessibility services, or use ComputerUse screenshot). Locate uses the same AT-SPI accessibility session as other automation."
            .to_string(),
    ))
}
