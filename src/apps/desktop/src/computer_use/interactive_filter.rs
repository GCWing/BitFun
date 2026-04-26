//! Filter a Codex-style [`AxNode`] tree into a Set-of-Mark
//! [`InteractiveElement`] list (TuriX-CUA inspired).
//!
//! The model's job is "pick a number" — to make that work we need:
//!   1. Drop non-interactive containers (groups, scroll areas, generic AXGroup).
//!   2. Drop nodes with zero / off-screen frames.
//!   3. Sort deterministically so the same UI always yields the same `i`.
//!   4. Assign dense `i` indices (0, 1, 2, …).
//!   5. Project each global frame to JPEG image pixel coordinates so the
//!      overlay renderer knows where to paint the numbered box.
//!
//! Image projection uses [`ComputerScreenshot::image_global_bounds`] when
//! present (the host fills it for both full-display and crop-around-window
//! captures), falling back to a conservative "skip the box" when bounds
//! are unknown — better to omit a label than to paint it on the wrong
//! widget.

#![allow(dead_code)]

use bitfun_core::agentic::tools::computer_use_host::{
    AxNode, ComputerScreenshot, InteractiveElement,
};

/// Per-host filter knobs.
#[derive(Debug, Clone)]
pub(crate) struct FilterOpts {
    /// Hard cap on emitted elements. The filter keeps the largest-area
    /// elements when exceeded so the overlay stays legible.
    pub max_elements: usize,
    /// When `true`, only elements whose frame intersects the focused
    /// window's image rectangle are kept. The host passes the rectangle
    /// via `image_global_bounds`; when bounds are missing we keep
    /// everything.
    pub clip_to_image_bounds: bool,
}

impl Default for FilterOpts {
    fn default() -> Self {
        Self {
            max_elements: 80,
            clip_to_image_bounds: true,
        }
    }
}

/// Build the SoM element list from a raw AX dump + the focused-window
/// screenshot the host already captured. The returned vector is sorted
/// deterministically and densely indexed (`elements[k].i == k as u32`).
pub(crate) fn build_interactive_elements(
    nodes: &[AxNode],
    screenshot: Option<&ComputerScreenshot>,
    opts: &FilterOpts,
) -> Vec<InteractiveElement> {
    let mut staged: Vec<Staged> = Vec::with_capacity(nodes.len() / 4);

    for n in nodes {
        if !is_interactive(n) {
            continue;
        }
        let Some(frame) = n.frame_global else {
            continue;
        };
        let (gx, gy, gw, gh) = frame;
        if gw < 4.0 || gh < 4.0 {
            continue;
        }

        let frame_image = screenshot
            .and_then(|s| project_global_to_image(s, gx, gy, gw, gh, opts.clip_to_image_bounds));

        // When clipping is requested and the host provided bounds, drop
        // anything that falls entirely outside the captured rectangle.
        if opts.clip_to_image_bounds {
            if let Some(s) = screenshot {
                if s.image_global_bounds.is_some() && frame_image.is_none() {
                    continue;
                }
            }
        }

        staged.push(Staged {
            node_idx: n.idx,
            role: n.role.clone(),
            subrole: n.subrole.clone(),
            label: best_label(n),
            frame_global: frame,
            frame_image,
            enabled: n.enabled,
            focused: n.focused,
            ax_actionable: n.actions.iter().any(|a| {
                matches!(
                    a.as_str(),
                    "AXPress" | "AXConfirm" | "AXOpen" | "AXShowMenu" | "AXPick"
                )
            }),
            area: (gw * gh) as f64,
        });
    }

    // Card-merge heuristic: when an actionable container (AXCell / AXRow /
    // AXButton / AXLink / AXGroup-with-AXPress) geometrically contains
    // smaller actionable children that are themselves actionable, drop
    // the children. Without this the SoM overlay shows 3-5 stacked
    // numbers on a single card (icon + label + cell) and the model has
    // to guess which one actually fires the navigation. Keep the card.
    //
    // Containment rule: parent area is at least 1.5x the child, and the
    // child rectangle is fully (with 2pt slop) inside the parent.
    if staged.len() > 1 {
        let originals = staged.clone();
        staged.retain(|child| {
            let (cx, cy, cw, ch) = child.frame_global;
            !originals.iter().any(|parent| {
                if parent.node_idx == child.node_idx {
                    return false;
                }
                if !is_card_container(&parent.role) {
                    return false;
                }
                if parent.area < child.area * 1.5 {
                    return false;
                }
                let (px, py, pw, ph) = parent.frame_global;
                cx + 2.0 >= px
                    && cy + 2.0 >= py
                    && cx + cw <= px + pw + 2.0
                    && cy + ch <= py + ph + 2.0
            })
        });
    }

    // Stable deterministic sort: top-to-bottom, then left-to-right.
    // Buckets of 16pt eliminate jitter from baseline differences between
    // controls on the same row.
    staged.sort_by(|a, b| {
        let (ax, ay, _, _) = a.frame_global;
        let (bx, by, _, _) = b.frame_global;
        let ay_b = (ay / 16.0).floor() as i64;
        let by_b = (by / 16.0).floor() as i64;
        ay_b.cmp(&by_b)
            .then_with(|| ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.node_idx.cmp(&b.node_idx))
    });

    if staged.len() > opts.max_elements {
        // Keep the largest-area elements so the overlay stays readable on
        // dense pages. We still preserve the deterministic display order
        // afterwards by re-sorting the kept slice.
        let mut by_area = staged;
        by_area.sort_by(|a, b| {
            b.area
                .partial_cmp(&a.area)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        by_area.truncate(opts.max_elements);
        by_area.sort_by(|a, b| {
            let (ax, ay, _, _) = a.frame_global;
            let (bx, by, _, _) = b.frame_global;
            let ay_b = (ay / 16.0).floor() as i64;
            let by_b = (by / 16.0).floor() as i64;
            ay_b.cmp(&by_b)
                .then_with(|| ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.node_idx.cmp(&b.node_idx))
        });
        staged = by_area;
    }

    staged
        .into_iter()
        .enumerate()
        .map(|(i, s)| InteractiveElement {
            i: i as u32,
            node_idx: s.node_idx,
            role: s.role,
            subrole: s.subrole,
            label: s.label,
            frame_image: s.frame_image,
            frame_global: Some(s.frame_global),
            enabled: s.enabled,
            focused: s.focused,
            ax_actionable: s.ax_actionable,
        })
        .collect()
}

/// Render a compact one-line-per-element text rendering used in the model
/// prompt alongside the annotated screenshot.
pub(crate) fn render_element_tree_text(elements: &[InteractiveElement]) -> String {
    let mut out = String::with_capacity(elements.len() * 64);
    for e in elements {
        let label = e.label.as_deref().unwrap_or("");
        let role = display_role(&e.role, e.subrole.as_deref());
        let mut line = format!("[{}] {} \"{}\"", e.i, role, label);
        if e.focused {
            line.push_str(" [focused]");
        }
        if !e.enabled {
            line.push_str(" [disabled]");
        }
        if !e.ax_actionable {
            line.push_str(" [pointer-only]");
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Roles eligible to "absorb" smaller actionable descendants in the SoM
/// overlay. Anything else (text fields, sliders, menu items …) keeps its
/// children visible — those tend to need direct interaction at the leaf.
fn is_card_container(role: &str) -> bool {
    matches!(
        role,
        "AXCell"
            | "AXRow"
            | "AXOutlineRow"
            | "AXButton"
            | "AXMenuButton"
            | "AXPopUpButton"
            | "AXLink"
            | "AXGroup"
    )
}

#[derive(Clone)]
struct Staged {
    node_idx: u32,
    role: String,
    subrole: Option<String>,
    label: Option<String>,
    frame_global: (f64, f64, f64, f64),
    frame_image: Option<(u32, u32, u32, u32)>,
    enabled: bool,
    focused: bool,
    ax_actionable: bool,
    area: f64,
}

/// Heuristic — keep elements a sighted user would consider "clickable" /
/// "fillable" / "selectable", and explicit text containers that are large
/// enough to be primary targets (so the model can disambiguate "the
/// button labelled X" from "the row labelled X" when both exist).
fn is_interactive(n: &AxNode) -> bool {
    if !n.enabled {
        return false;
    }
    let role = n.role.as_str();

    // Always interactive roles.
    matches!(
        role,
        "AXButton"
            | "AXMenuButton"
            | "AXPopUpButton"
            | "AXCheckBox"
            | "AXRadioButton"
            | "AXSwitch"
            | "AXToggle"
            | "AXTextField"
            | "AXSecureTextField"
            | "AXSearchField"
            | "AXTextArea"
            | "AXComboBox"
            | "AXLink"
            | "AXTab"
            | "AXTabGroup"
            | "AXSlider"
            | "AXIncrementor"
            | "AXStepper"
            | "AXMenu"
            | "AXMenuItem"
            | "AXMenuBarItem"
            | "AXDisclosureTriangle"
            | "AXRow"
            | "AXOutlineRow"
            | "AXCell"
    ) ||
    // Or: any node that exposes an actionable AX action.
    n.actions.iter().any(|a| {
        matches!(
            a.as_str(),
            "AXPress" | "AXConfirm" | "AXOpen" | "AXShowMenu" | "AXPick" | "AXIncrement" | "AXDecrement"
        )
    })
}

fn best_label(n: &AxNode) -> Option<String> {
    for cand in [&n.title, &n.description, &n.help, &n.value, &n.identifier] {
        if let Some(s) = cand {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(clip(trimmed, 80));
            }
        }
    }
    None
}

fn clip(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn display_role(role: &str, subrole: Option<&str>) -> String {
    let stripped = role.strip_prefix("AX").unwrap_or(role);
    match subrole {
        Some(sr) if !sr.is_empty() => {
            let sr_stripped = sr.strip_prefix("AX").unwrap_or(sr);
            format!("{}({})", stripped, sr_stripped)
        }
        _ => stripped.to_string(),
    }
}

/// Project a global pointer-space rectangle onto the JPEG image pixel
/// grid. Returns `None` when the screenshot has no `image_global_bounds`
/// (host could not resolve the mapping), or the rectangle falls entirely
/// outside the captured area.
fn project_global_to_image(
    shot: &ComputerScreenshot,
    gx: f64,
    gy: f64,
    gw: f64,
    gh: f64,
    require_intersection: bool,
) -> Option<(u32, u32, u32, u32)> {
    let bounds = shot.image_global_bounds.as_ref()?;
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return None;
    }

    let scale_x = shot.image_width as f64 / bounds.width;
    let scale_y = shot.image_height as f64 / bounds.height;

    // Clip the global rectangle to the image rectangle.
    let lx = gx.max(bounds.left);
    let ty = gy.max(bounds.top);
    let rx = (gx + gw).min(bounds.left + bounds.width);
    let by = (gy + gh).min(bounds.top + bounds.height);
    if rx <= lx || by <= ty {
        if require_intersection {
            return None;
        }
        // No intersection but caller wants a best-effort projection — fall
        // through using the unclipped rectangle so the overlay can decide
        // whether to draw a clipped marker.
        let ix = ((gx - bounds.left) * scale_x).round();
        let iy = ((gy - bounds.top) * scale_y).round();
        let iw = (gw * scale_x).round().max(1.0);
        let ih = (gh * scale_y).round().max(1.0);
        return Some((ix.max(0.0) as u32, iy.max(0.0) as u32, iw as u32, ih as u32));
    }

    let ix = ((lx - bounds.left) * scale_x).round();
    let iy = ((ty - bounds.top) * scale_y).round();
    let iw = ((rx - lx) * scale_x).round().max(1.0);
    let ih = ((by - ty) * scale_y).round().max(1.0);

    let max_x = shot.image_width.saturating_sub(1) as f64;
    let max_y = shot.image_height.saturating_sub(1) as f64;
    Some((
        ix.max(0.0).min(max_x) as u32,
        iy.max(0.0).min(max_y) as u32,
        iw as u32,
        ih as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_core::agentic::tools::computer_use_host::ComputerUseImageGlobalBounds;

    fn node(idx: u32, role: &str, frame: Option<(f64, f64, f64, f64)>) -> AxNode {
        AxNode {
            idx,
            parent_idx: None,
            role: role.to_string(),
            title: Some(format!("label-{idx}")),
            value: None,
            description: None,
            identifier: None,
            enabled: true,
            focused: false,
            selected: None,
            frame_global: frame,
            actions: vec!["AXPress".into()],
            role_description: None,
            subrole: None,
            help: None,
            url: None,
            expanded: None,
        }
    }

    fn screenshot() -> ComputerScreenshot {
        ComputerScreenshot {
            screenshot_id: Some("test-shot".to_string()),
            bytes: vec![],
            mime_type: "image/jpeg".to_string(),
            image_width: 1000,
            image_height: 800,
            native_width: 2000,
            native_height: 1600,
            display_origin_x: 0,
            display_origin_y: 0,
            vision_scale: 0.5,
            pointer_image_x: None,
            pointer_image_y: None,
            screenshot_crop_center: None,
            point_crop_half_extent_native: None,
            navigation_native_rect: None,
            quadrant_navigation_click_ready: false,
            image_content_rect: None,
            image_global_bounds: Some(ComputerUseImageGlobalBounds {
                left: 0.0,
                top: 0.0,
                width: 500.0,
                height: 400.0,
            }),
            ui_tree_text: None,
            implicit_confirmation_crop_applied: false,
        }
    }

    #[test]
    fn drops_non_interactive_and_off_screen_nodes() {
        let mut group = node(0, "AXGroup", Some((0.0, 0.0, 100.0, 100.0)));
        group.actions.clear();
        let nodes = vec![
            group,
            node(1, "AXButton", Some((10.0, 10.0, 50.0, 30.0))),
            node(2, "AXButton", None),
            node(3, "AXButton", Some((1.0, 1.0, 2.0, 2.0))),
        ];
        let opts = FilterOpts::default();
        let out = build_interactive_elements(&nodes, Some(&screenshot()), &opts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].i, 0);
        assert_eq!(out[0].node_idx, 1);
    }

    #[test]
    fn projects_frame_to_image_pixels_with_scale() {
        let nodes = vec![node(0, "AXButton", Some((100.0, 80.0, 50.0, 40.0)))];
        let out = build_interactive_elements(&nodes, Some(&screenshot()), &FilterOpts::default());
        let (ix, iy, iw, ih) = out[0].frame_image.expect("frame_image present");
        // bounds 500x400 → image 1000x800 → 2x scale on both axes.
        assert_eq!(ix, 200);
        assert_eq!(iy, 160);
        assert_eq!(iw, 100);
        assert_eq!(ih, 80);
    }

    #[test]
    fn dense_indices_in_top_to_bottom_order() {
        let nodes = vec![
            node(0, "AXButton", Some((400.0, 200.0, 30.0, 20.0))),
            node(1, "AXButton", Some((100.0, 100.0, 30.0, 20.0))),
            node(2, "AXButton", Some((50.0, 200.0, 30.0, 20.0))),
        ];
        let out = build_interactive_elements(&nodes, Some(&screenshot()), &FilterOpts::default());
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].node_idx, 1); // top row
        assert_eq!(out[1].node_idx, 2); // bottom-left
        assert_eq!(out[2].node_idx, 0); // bottom-right
        for (k, e) in out.iter().enumerate() {
            assert_eq!(e.i, k as u32);
        }
    }

    #[test]
    fn caps_at_max_elements() {
        let nodes: Vec<_> = (0..10)
            .map(|k| node(k, "AXButton", Some((k as f64 * 50.0, 10.0, 30.0, 20.0))))
            .collect();
        let opts = FilterOpts {
            max_elements: 4,
            ..FilterOpts::default()
        };
        let out = build_interactive_elements(&nodes, Some(&screenshot()), &opts);
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn card_container_absorbs_contained_actionable_children() {
        // Outer cell (large) + inner button + inner static-text-as-button,
        // all actionable. Card-merge should keep the cell only.
        let cell = node(10, "AXCell", Some((0.0, 0.0, 300.0, 80.0)));
        let inner_btn = node(11, "AXButton", Some((10.0, 10.0, 60.0, 60.0)));
        let inner_btn2 = node(12, "AXButton", Some((100.0, 20.0, 100.0, 30.0)));
        // Sibling button outside the cell stays.
        let outside = node(13, "AXButton", Some((400.0, 0.0, 50.0, 30.0)));
        let nodes = vec![cell, inner_btn, inner_btn2, outside];
        let out = build_interactive_elements(&nodes, Some(&screenshot()), &FilterOpts::default());
        let kept_idx: Vec<u32> = out.iter().map(|e| e.node_idx).collect();
        assert!(kept_idx.contains(&10), "cell must survive: {:?}", kept_idx);
        assert!(
            kept_idx.contains(&13),
            "outside btn must survive: {:?}",
            kept_idx
        );
        assert!(
            !kept_idx.contains(&11),
            "inner btn 11 must be absorbed: {:?}",
            kept_idx
        );
        assert!(
            !kept_idx.contains(&12),
            "inner btn 12 must be absorbed: {:?}",
            kept_idx
        );
    }

    #[test]
    fn render_text_lists_one_per_line() {
        let nodes = vec![
            node(0, "AXButton", Some((10.0, 10.0, 30.0, 20.0))),
            node(1, "AXTextField", Some((10.0, 50.0, 100.0, 20.0))),
        ];
        let elements =
            build_interactive_elements(&nodes, Some(&screenshot()), &FilterOpts::default());
        let text = render_element_tree_text(&elements);
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("[0] Button \"label-0\""));
        assert_eq!(lines.next(), Some("[1] TextField \"label-1\""));
    }
}
