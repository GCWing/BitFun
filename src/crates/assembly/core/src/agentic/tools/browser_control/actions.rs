//! Atomic browser actions implemented via CDP commands.

use super::cdp_client::{CdpClient, CdpEvent};
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::sync::broadcast;

/// Result of waiting for a CDP `Page.lifecycleEvent`.
enum LifecycleOutcome {
    /// One of the requested lifecycle names fired in time. Carries the name
    /// (e.g. `"load"`, `"networkIdle"`) so callers can report which condition
    /// actually matched.
    Reached(String),
    /// Timed out before any of the requested events fired.
    Timeout,
    /// Subscription closed (typically: page navigated away or browser quit).
    Closed,
}

/// Block until a `Page.lifecycleEvent` whose `name` ∈ `wanted` arrives for the
/// given `frame_id` (or any frame if `frame_id` is `None`). Bounded by a hard
/// timeout so a hung page can never wedge the agent.
async fn wait_for_lifecycle(
    events: &mut broadcast::Receiver<CdpEvent>,
    frame_id: Option<&str>,
    wanted: &[&str],
    timeout_ms: u64,
) -> LifecycleOutcome {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return LifecycleOutcome::Timeout;
        }
        let recv_fut = events.recv();
        let evt = match tokio::time::timeout(remaining, recv_fut).await {
            Err(_) => return LifecycleOutcome::Timeout,
            Ok(Err(broadcast::error::RecvError::Closed)) => return LifecycleOutcome::Closed,
            // We deliberately swallow Lagged: lifecycle bursts can outpace
            // our buffer briefly; the next iteration will catch the next one.
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Ok(evt)) => evt,
        };
        if evt.method != "Page.lifecycleEvent" {
            continue;
        }
        let name = evt
            .params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !wanted.contains(&name) {
            continue;
        }
        if let Some(want_frame) = frame_id {
            let evt_frame = evt
                .params
                .get("frameId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if evt_frame != want_frame {
                continue;
            }
        }
        return LifecycleOutcome::Reached(name.to_string());
    }
}

/// High-level browser actions backed by CDP method calls.
pub struct BrowserActions<'a> {
    client: &'a CdpClient,
}

impl<'a> BrowserActions<'a> {
    pub fn new(client: &'a CdpClient) -> Self {
        Self { client }
    }

    pub async fn enable_observers(&self) -> BitFunResult<Value> {
        let _ = self.client.send("Page.enable", None).await;
        let _ = self.client.send("Runtime.enable", None).await;
        let _ = self.client.send("Network.enable", None).await;
        let _ = self.client.send("DOM.enable", None).await;
        Ok(json!({ "success": true, "action": "enable_observers" }))
    }

    // ── Navigation ─────────────────────────────────────────────────────

    pub async fn navigate(&self, url: &str) -> BitFunResult<Value> {
        // Subscribe **before** issuing the navigate so we can never miss the
        // `Page.lifecycleEvent` ("load") that fires while we are awaiting the
        // command response. Page lifecycle events must be enabled explicitly.
        let _ = self.client.send("Page.enable", None).await;
        let _ = self
            .client
            .send(
                "Page.setLifecycleEventsEnabled",
                Some(json!({ "enabled": true })),
            )
            .await;
        let mut events = self.client.subscribe_events();

        let result = self
            .client
            .send("Page.navigate", Some(json!({ "url": url })))
            .await?;
        let frame_id = result
            .get("frameId")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        // Wait for the matching "load" lifecycle event (or "DOMContentLoaded"
        // as an early signal). Capped at ~15s so a hung page eventually
        // surfaces a Timeout error to the model rather than blocking forever.
        let outcome = wait_for_lifecycle(&mut events, frame_id.as_deref(), &["load"], 15_000).await;

        let mut body = json!({
            "url": url,
            "frameId": frame_id,
        });
        match outcome {
            LifecycleOutcome::Reached(name) => {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("success".to_string(), json!(true));
                    obj.insert("loaded".to_string(), json!(true));
                    obj.insert("lifecycle_event".to_string(), json!(name));
                }
            }
            LifecycleOutcome::Timeout => {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("success".to_string(), json!(true));
                    obj.insert("loaded".to_string(), json!(false));
                    obj.insert(
                        "warning".to_string(),
                        json!("navigation timed out before lifecycle 'load' event; page may still be loading"),
                    );
                }
            }
            LifecycleOutcome::Closed => {
                return Err(BitFunError::tool(
                    "Browser closed the CDP connection before page finished loading.".to_string(),
                ));
            }
        }
        Ok(body)
    }

    pub async fn back(&self) -> BitFunResult<Value> {
        self.evaluate("history.back(); undefined").await?;
        Ok(json!({ "success": true, "action": "back" }))
    }

    pub async fn forward(&self) -> BitFunResult<Value> {
        self.evaluate("history.forward(); undefined").await?;
        Ok(json!({ "success": true, "action": "forward" }))
    }

    pub async fn reload(&self, ignore_cache: bool) -> BitFunResult<Value> {
        self.client
            .send("Page.reload", Some(json!({ "ignoreCache": ignore_cache })))
            .await?;
        Ok(json!({ "success": true, "action": "reload", "ignore_cache": ignore_cache }))
    }

    pub async fn get_url(&self) -> BitFunResult<String> {
        let result = self.evaluate("window.location.href").await?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    pub async fn get_title(&self) -> BitFunResult<String> {
        let result = self.evaluate("document.title").await?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    // ── Snapshot / DOM ─────────────────────────────────────────────────

    /// Get an accessibility-tree snapshot of interactive elements.
    ///
    /// Phase 3: traversal now descends into **open shadow roots** and
    /// **same-origin iframes**, which the old flat `document.querySelectorAll`
    /// path silently skipped. Each element's `frame_path` reports where in
    /// the frame tree it lives (`""` for top frame,
    /// `"iframe[src='/foo']"` for an iframe child) and its `scope` reports
    /// `"document" | "shadow" | "iframe"`. The synthetic `data-cdp-ref`
    /// attribute is set in the host scope so subsequent `click` / `fill`
    /// can locate it via the same recursive walk.
    pub async fn snapshot(&self) -> BitFunResult<Value> {
        self.snapshot_with_options(false).await
    }

    /// Snapshot variant that can additionally resolve a stable
    /// **backendNodeId** (CDP `DOM.Node.backendNodeId`) for each element.
    /// `backendNodeId` is invariant across reflows and JS re-renders within
    /// the same DOM lifetime, so saving it lets the agent re-target an
    /// element after a partial mutation without taking a full snapshot.
    ///
    /// The call is opt-in (and slightly more expensive) because it costs
    /// one extra CDP round-trip plus a `DOM.querySelectorAll` walk. When
    /// `with_backend_node_ids` is `true`, every snapshot element gets a
    /// `backend_node_id` field; pages where `DOM.getDocument` errors out
    /// (very rare — e.g. about:blank) silently fall back to no ids.
    pub async fn snapshot_with_options(&self, with_backend_node_ids: bool) -> BitFunResult<Value> {
        let script = r#"
        (function() {
            const SEL = 'a, button, input, textarea, select, [role="button"], [role="link"], [role="tab"], [role="menuitem"], [role="combobox"], [role="option"], [tabindex="0"], [contenteditable="true"]';
            const items = [];
            let idx = 1;

            function visible(el, win) {
                const rect = el.getBoundingClientRect();
                if (rect.width < 2 || rect.height < 2) return null;
                if (rect.right < 0 || rect.bottom < 0 || rect.left > win.innerWidth || rect.top > win.innerHeight) return null;
                const style = win.getComputedStyle(el);
                if (style.display === 'none' || style.visibility === 'hidden') return null;
                return rect;
            }

            function record(el, rect, scope, framePath) {
                const text = (el.textContent || '').trim().slice(0, 100);
                items.push({
                    ref: '@e' + idx,
                    tag: el.tagName.toLowerCase(),
                    type: el.getAttribute('type') || '',
                    name: el.getAttribute('name') || '',
                    text,
                    ariaLabel: el.getAttribute('aria-label') || '',
                    placeholder: el.placeholder || '',
                    role: el.getAttribute('role') || '',
                    href: el.href || '',
                    id: el.id || '',
                    scope,
                    frame_path: framePath,
                    rect: { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) }
                });
                try { el.setAttribute('data-cdp-ref', '@e' + idx); } catch (_) {}
                idx++;
            }

            // Recursive walk: collects from `root` (Document or ShadowRoot)
            // and recurses into open shadow roots of every descendant. Iframes
            // are handled by the caller because we need the iframe's own
            // window for visibility checks.
            function walk(root, win, scope, framePath) {
                const els = root.querySelectorAll(SEL);
                els.forEach(el => {
                    const rect = visible(el, win);
                    if (rect) record(el, rect, scope, framePath);
                });
                // Open shadow roots
                const allHosts = root.querySelectorAll('*');
                allHosts.forEach(h => {
                    if (h.shadowRoot) {
                        try { walk(h.shadowRoot, win, 'shadow', framePath); } catch (_) {}
                    }
                });
            }

            walk(document, window, 'document', '');

            // Same-origin iframes
            const frames = document.querySelectorAll('iframe, frame');
            frames.forEach((frame, fi) => {
                let doc = null;
                try { doc = frame.contentDocument; } catch (_) {}
                if (!doc) return; // cross-origin: skip silently
                const subWin = frame.contentWindow;
                const path = `iframe[${fi}]${frame.src ? `[src="${frame.src.slice(0, 80)}"]` : ''}`;
                try { walk(doc, subWin, 'iframe', path); } catch (_) {}
            });

            return JSON.stringify({
                url: location.href,
                title: document.title,
                elements: items,
                features: { shadow_dom_traversed: true, same_origin_iframes_traversed: true },
            });
        })()
        "#;
        let result = self.evaluate(script).await?;
        let text = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let mut parsed: Value = serde_json::from_str(text).unwrap_or(json!({}));

        if with_backend_node_ids {
            if let Err(e) = self.attach_backend_node_ids(&mut parsed).await {
                // Don't fail the snapshot — the elements list is still
                // useful without backendNodeIds. Surface the failure so the
                // model can decide whether to retry.
                if let Value::Object(m) = &mut parsed {
                    m.insert(
                        "backend_node_ids_warning".to_string(),
                        json!(format!("Failed to resolve backendNodeIds: {}", e)),
                    );
                }
            }
        }
        Self::attach_snapshot_text(&mut parsed);
        Ok(parsed)
    }

    fn attach_snapshot_text(parsed: &mut Value) {
        let Some(elements) = parsed.get("elements").and_then(|v| v.as_array()) else {
            return;
        };
        let mut lines = Vec::<String>::new();
        let mut refs = BTreeMap::<String, Value>::new();
        for element in elements {
            let reference = element.get("ref").and_then(|v| v.as_str()).unwrap_or("");
            let tag = element
                .get("tag")
                .and_then(|v| v.as_str())
                .unwrap_or("element");
            let role = element.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let text = element
                .get("ariaLabel")
                .or_else(|| element.get("placeholder"))
                .or_else(|| element.get("text"))
                .or_else(|| element.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let type_text = element.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let frame_path = element
                .get("frame_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let scope = element
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("document");
            let mut label = if role.is_empty() {
                tag.to_string()
            } else {
                role.to_string()
            };
            if !type_text.is_empty() {
                label.push(':');
                label.push_str(type_text);
            }
            let mut line = if reference.is_empty() {
                format!("- {}", label)
            } else {
                format!("- {} [{}]", label, reference)
            };
            if !text.is_empty() {
                let clipped = if text.chars().count() > 80 {
                    format!("{}...", text.chars().take(77).collect::<String>())
                } else {
                    text.to_string()
                };
                line.push(' ');
                line.push_str(
                    &serde_json::to_string(&clipped).unwrap_or_else(|_| "\"\"".to_string()),
                );
            }
            if !id.is_empty() {
                line.push_str(&format!(" id={}", id));
            }
            if scope != "document" || !frame_path.is_empty() {
                line.push_str(&format!(" scope={}", scope));
                if !frame_path.is_empty() {
                    line.push_str(&format!(" frame={}", frame_path));
                }
            }
            lines.push(line);
            if !reference.is_empty() {
                refs.insert(reference.to_string(), element.clone());
            }
        }
        if let Some(obj) = parsed.as_object_mut() {
            obj.insert("snapshot".to_string(), json!(lines.join("\n")));
            obj.insert("refs".to_string(), json!(refs));
        }
    }

    /// Resolve `backend_node_id` for every snapshot element by walking the
    /// DOM through CDP. Mutates `parsed["elements"][i]["backend_node_id"]`
    /// in place. Returns `Err` if the document tree could not be fetched.
    async fn attach_backend_node_ids(&self, parsed: &mut Value) -> BitFunResult<()> {
        let doc = self.client.send("DOM.getDocument", None).await?;
        let root_id = doc
            .get("root")
            .and_then(|r| r.get("nodeId"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| BitFunError::tool("DOM.getDocument: missing root nodeId".to_string()))?;
        let qsa = self
            .client
            .send(
                "DOM.querySelectorAll",
                Some(json!({ "nodeId": root_id, "selector": "[data-cdp-ref]" })),
            )
            .await?;
        let node_ids: Vec<i64> = qsa
            .get("nodeIds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|n| n.as_i64()).collect())
            .unwrap_or_default();

        let mut by_ref: std::collections::HashMap<String, i64> = Default::default();
        for nid in node_ids {
            let described = match self
                .client
                .send("DOM.describeNode", Some(json!({ "nodeId": nid })))
                .await
            {
                Ok(d) => d,
                Err(_) => continue,
            };
            let backend = described
                .get("node")
                .and_then(|n| n.get("backendNodeId"))
                .and_then(|v| v.as_i64());
            // Read the data-cdp-ref attribute from the node's attributes
            // (DOM.describeNode returns flat [name, value, name, value]).
            let attrs = described
                .get("node")
                .and_then(|n| n.get("attributes"))
                .and_then(|v| v.as_array());
            let ref_name = attrs.and_then(|a| {
                a.chunks(2)
                    .find(|c| c.first().and_then(|n| n.as_str()) == Some("data-cdp-ref"))
                    .and_then(|c| c.get(1).and_then(|v| v.as_str().map(str::to_string)))
            });
            if let (Some(rn), Some(b)) = (ref_name, backend) {
                by_ref.insert(rn, b);
            }
        }

        if let Some(elements) = parsed.get_mut("elements").and_then(|v| v.as_array_mut()) {
            for el in elements.iter_mut() {
                let r = el.get("ref").and_then(|v| v.as_str()).map(str::to_string);
                if let Some(r) = r {
                    if let Some(b) = by_ref.get(&r) {
                        if let Value::Object(m) = el {
                            m.insert("backend_node_id".to_string(), json!(b));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Get the text content of an element by CSS selector or `@eN` ref.
    ///
    /// Phase 3: returns `Ok(None)` when the selector matched nothing (so
    /// ControlHub can surface a `NOT_FOUND` error instead of a misleading
    /// empty string), and `Ok(Some(""))` when the element was found but
    /// genuinely empty. The lookup walks shadow roots / same-origin
    /// iframes, matching the rest of the browser action surface.
    pub async fn get_text(&self, selector: &str) -> BitFunResult<Option<String>> {
        self.get_attribute(selector, "text")
            .await
            .map(|v| v.map(|v| v.as_str().unwrap_or("").to_string()))
    }

    pub async fn get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> BitFunResult<Option<Value>> {
        let resolve = Self::resolve_element_js(selector);
        let getter = match attribute {
            "text" => "(el.textContent || '').trim().slice(0, 5000)".to_string(),
            "value" => "('value' in el ? el.value : '')".to_string(),
            "html" => "el.outerHTML".to_string(),
            other => format!(
                "el.getAttribute('{}')",
                other.replace('\\', "\\\\").replace('\'', "\\'")
            ),
        };
        let js = format!(
            r#"(function(){{
                try {{
                    {resolve}
                    return JSON.stringify({{ found: true, value: {getter} }});
                }} catch (e) {{
                    return JSON.stringify({{ found: false, error: String(e && e.message || e) }});
                }}
            }})()"#,
            resolve = resolve,
            getter = getter,
        );
        let result = self.evaluate(&js).await?;
        let raw = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let parsed: Value = serde_json::from_str(raw).unwrap_or(json!({}));
        if parsed
            .get("found")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            Ok(Some(parsed.get("value").cloned().unwrap_or(Value::Null)))
        } else {
            Ok(None)
        }
    }

    // ── Interaction ────────────────────────────────────────────────────

    /// Click an element by CSS selector or by `@eN` ref.
    pub async fn click(&self, selector: &str) -> BitFunResult<Value> {
        let (x, y) = self.element_center(selector).await?;

        self.client
            .send(
                "Input.dispatchMouseEvent",
                Some(json!({
                    "type": "mousePressed",
                    "x": x, "y": y,
                    "button": "left", "clickCount": 1
                })),
            )
            .await?;
        self.client
            .send(
                "Input.dispatchMouseEvent",
                Some(json!({
                    "type": "mouseReleased",
                    "x": x, "y": y,
                    "button": "left", "clickCount": 1
                })),
            )
            .await?;

        Ok(json!({
            "success": true,
            "action": "click",
            "selector": selector,
            "coordinates": { "x": x, "y": y }
        }))
    }

    async fn element_center(&self, selector: &str) -> BitFunResult<(f64, f64)> {
        let js = Self::resolve_element_js(selector);
        let center_js = format!(
            r#"(function(){{ {} el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }}); const rect = el.getBoundingClientRect(); return JSON.stringify({{ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }}); }})()"#,
            js
        );
        let result = self.evaluate(&center_js).await?;
        let coords_str = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let coords: Value = serde_json::from_str(coords_str).unwrap_or(json!({}));
        let x = coords.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = coords.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok((x, y))
    }

    pub async fn hover(&self, selector: &str) -> BitFunResult<Value> {
        let (x, y) = self.element_center(selector).await?;
        self.client
            .send(
                "Input.dispatchMouseEvent",
                Some(json!({
                    "type": "mouseMoved",
                    "x": x, "y": y,
                    "button": "none"
                })),
            )
            .await?;
        Ok(json!({
            "success": true,
            "action": "hover",
            "selector": selector,
            "coordinates": { "x": x, "y": y }
        }))
    }

    /// Fill (clear + type) a text input identified by selector or `@eN` ref.
    pub async fn fill(&self, selector: &str, value: &str) -> BitFunResult<Value> {
        let js = Self::resolve_element_js(selector);
        let focus_js = format!(
            r#"(function(){{ {} el.focus(); el.value = ''; el.dispatchEvent(new Event('input', {{ bubbles: true }})); return true; }})()"#,
            js
        );
        self.evaluate(&focus_js).await?;

        self.client
            .send("Input.insertText", Some(json!({ "text": value })))
            .await?;

        Ok(json!({
            "success": true,
            "action": "fill",
            "selector": selector,
        }))
    }

    /// Type text at the currently focused element (appends, does not clear).
    pub async fn type_text(&self, text: &str) -> BitFunResult<Value> {
        self.client
            .send("Input.insertText", Some(json!({ "text": text })))
            .await?;
        Ok(json!({ "success": true, "action": "type", "text": text }))
    }

    pub async fn set_checked(&self, selector: &str, checked: bool) -> BitFunResult<Value> {
        let js = Self::resolve_element_js(selector);
        let script = format!(
            r#"(function(){{
                {js}
                el.checked = {checked};
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return JSON.stringify({{ success: true, checked: !!el.checked }});
            }})()"#,
            js = js,
            checked = if checked { "true" } else { "false" }
        );
        let result = self.evaluate(&script).await?;
        let text = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let parsed: Value = serde_json::from_str(text).unwrap_or(json!({}));
        Ok(json!({
            "success": parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(true),
            "action": if checked { "check" } else { "uncheck" },
            "selector": selector,
            "checked": parsed.get("checked").cloned().unwrap_or(json!(checked)),
        }))
    }

    /// Select a dropdown option by visible text.
    pub async fn select(&self, selector: &str, option_text: &str) -> BitFunResult<Value> {
        let js = format!(
            r#"(function(){{
                const sel = document.querySelector('{}');
                if (!sel) return JSON.stringify({{ error: 'Select not found' }});
                const opts = Array.from(sel.options);
                const opt = opts.find(o => o.text.includes('{}'));
                if (!opt) return JSON.stringify({{ error: 'Option not found', available: opts.map(o => o.text) }});
                sel.value = opt.value;
                sel.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return JSON.stringify({{ success: true, value: opt.value, text: opt.text }});
            }})()"#,
            selector.replace('\'', "\\'"),
            option_text.replace('\'', "\\'")
        );
        let result = self.evaluate(&js).await?;
        let text = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let parsed: Value = serde_json::from_str(text).unwrap_or(json!({}));
        Ok(parsed)
    }

    /// Press a key (Enter, Escape, Tab, etc.).
    pub async fn press_key(&self, key: &str) -> BitFunResult<Value> {
        self.client
            .send(
                "Input.dispatchKeyEvent",
                Some(json!({
                    "type": "keyDown",
                    "key": key,
                })),
            )
            .await?;
        if key.chars().count() == 1 {
            self.client
                .send(
                    "Input.dispatchKeyEvent",
                    Some(json!({
                        "type": "char",
                        "key": key,
                        "text": key,
                    })),
                )
                .await?;
        }
        self.client
            .send(
                "Input.dispatchKeyEvent",
                Some(json!({
                    "type": "keyUp",
                    "key": key,
                })),
            )
            .await?;
        Ok(json!({ "success": true, "action": "press_key", "key": key }))
    }

    /// Scroll the page.
    pub async fn scroll(&self, direction: &str, amount: Option<i64>) -> BitFunResult<Value> {
        let px = amount.unwrap_or(500);
        let (delta_x, delta_y) = match direction {
            "up" => (0, -px),
            "down" => (0, px),
            "left" => (-px, 0),
            "right" => (px, 0),
            "top" => {
                self.evaluate("window.scrollTo(0, 0)").await?;
                return Ok(json!({ "success": true, "action": "scroll", "direction": "top" }));
            }
            "bottom" => {
                self.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                    .await?;
                return Ok(json!({ "success": true, "action": "scroll", "direction": "bottom" }));
            }
            _ => (0, px),
        };
        self.client
            .send(
                "Input.dispatchMouseEvent",
                Some(json!({
                    "type": "mouseWheel",
                    "x": 400, "y": 300,
                    "deltaX": delta_x, "deltaY": delta_y,
                })),
            )
            .await?;
        Ok(json!({ "success": true, "action": "scroll", "direction": direction, "amount": px }))
    }

    pub async fn auto_scroll(
        &self,
        direction: &str,
        max_scrolls: u64,
        delay_ms: u64,
    ) -> BitFunResult<Value> {
        let max_scrolls = max_scrolls.clamp(1, 200);
        let delay_ms = delay_ms.clamp(0, 5_000);
        let delta = if direction == "up" {
            "-window.innerHeight"
        } else {
            "window.innerHeight"
        };
        let boundary = if direction == "up" {
            "window.scrollY === 0"
        } else {
            "window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2"
        };
        let script = format!(
            r#"(async () => {{
                let scrolls = 0;
                while (scrolls < {max_scrolls}) {{
                    const before = window.scrollY;
                    window.scrollBy(0, {delta});
                    await new Promise(resolve => setTimeout(resolve, {delay_ms}));
                    scrolls++;
                    if ({boundary} || window.scrollY === before) break;
                }}
                return {{ scrolls, scrollY: window.scrollY, height: document.documentElement.scrollHeight }};
            }})()"#
        );
        let result = self.evaluate(&script).await?;
        Ok(json!({
            "success": true,
            "action": "auto_scroll",
            "direction": direction,
            "result": result.get("result").and_then(|r| r.get("value")).cloned().unwrap_or(Value::Null),
        }))
    }

    /// Wait for a duration or a condition.
    pub async fn wait(
        &self,
        duration_ms: Option<u64>,
        condition: Option<&str>,
    ) -> BitFunResult<Value> {
        if let Some(ms) = duration_ms {
            let clamped = ms.min(30_000);
            tokio::time::sleep(std::time::Duration::from_millis(clamped)).await;
            return Ok(json!({ "success": true, "action": "wait", "ms": clamped }));
        }
        if let Some(cond) = condition {
            match cond {
                "networkidle" | "load" | "domcontentloaded" => {
                    // Phase 1: replace the previous "sleep 2s and hope" with
                    // a real `Page.lifecycleEvent` subscription. Lifecycle
                    // event names per CDP: `load`, `DOMContentLoaded`,
                    // `networkIdle`, `firstMeaningfulPaint`, etc.
                    let _ = self.client.send("Page.enable", None).await;
                    let _ = self
                        .client
                        .send(
                            "Page.setLifecycleEventsEnabled",
                            Some(json!({ "enabled": true })),
                        )
                        .await;
                    let mut events = self.client.subscribe_events();
                    let wanted: &[&str] = match cond {
                        "networkidle" => &["networkIdle"],
                        "domcontentloaded" => &["DOMContentLoaded", "load"],
                        _ => &["load"],
                    };
                    let outcome = wait_for_lifecycle(&mut events, None, wanted, 15_000).await;
                    let (success, lifecycle_event, timed_out) = match outcome {
                        LifecycleOutcome::Reached(n) => (true, Some(n), false),
                        LifecycleOutcome::Timeout => (false, None, true),
                        LifecycleOutcome::Closed => (false, None, false),
                    };
                    return Ok(json!({
                        "success": success,
                        "action": "wait",
                        "condition": cond,
                        "lifecycle_event": lifecycle_event,
                        "timed_out": timed_out,
                    }));
                }
                selector => {
                    for _ in 0..30 {
                        let js = format!(
                            "!!document.querySelector('{}')",
                            selector.replace('\'', "\\'")
                        );
                        let result = self.evaluate(&js).await?;
                        let found = result
                            .get("result")
                            .and_then(|r| r.get("value"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        if found {
                            return Ok(
                                json!({ "success": true, "action": "wait", "condition": cond }),
                            );
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    return Err(BitFunError::tool(format!(
                        "Timeout waiting for element: {}",
                        cond
                    )));
                }
            }
        }
        Ok(json!({ "success": true, "action": "wait" }))
    }

    // ── Capture ────────────────────────────────────────────────────────

    /// Take a screenshot of the current page, returns base64 JPEG data.
    pub async fn screenshot(&self) -> BitFunResult<Value> {
        self.screenshot_with_options("jpeg", Some(80), true).await
    }

    pub async fn screenshot_with_options(
        &self,
        format: &str,
        quality: Option<u8>,
        from_surface: bool,
    ) -> BitFunResult<Value> {
        self.screenshot_with_options_ext(format, quality, from_surface, false)
            .await
    }

    pub async fn screenshot_with_options_ext(
        &self,
        format: &str,
        quality: Option<u8>,
        from_surface: bool,
        full_page: bool,
    ) -> BitFunResult<Value> {
        let normalized = if format.eq_ignore_ascii_case("png") {
            "png"
        } else {
            "jpeg"
        };

        if full_page {
            if let Ok(metrics) = self.client.send("Page.getLayoutMetrics", None).await {
                let size = metrics
                    .get("cssContentSize")
                    .or_else(|| metrics.get("contentSize"));
                if let Some(size) = size {
                    let width = size
                        .get("width")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0)
                        .ceil() as u64;
                    let height = size
                        .get("height")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0)
                        .ceil() as u64;
                    if width > 0 && height > 0 {
                        let _ = self
                            .client
                            .send(
                                "Emulation.setDeviceMetricsOverride",
                                Some(json!({
                                    "mobile": false,
                                    "width": width,
                                    "height": height,
                                    "deviceScaleFactor": 1,
                                })),
                            )
                            .await;
                    }
                }
            }
        }

        let mut params = json!({
            "format": normalized,
            "fromSurface": from_surface,
        });
        if normalized == "jpeg" {
            params["quality"] = json!(quality.unwrap_or(80).min(100));
        }
        let result = self
            .client
            .send("Page.captureScreenshot", Some(params))
            .await?;
        if full_page {
            let _ = self
                .client
                .send("Emulation.clearDeviceMetricsOverride", None)
                .await;
        }
        let data = result.get("data").and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!({
            "success": true,
            "action": "screenshot",
            "format": normalized,
            "full_page": full_page,
            "data_length": data.len(),
            "base64_data": data,
            "data_url": format!("data:image/{};base64,{}", normalized, data),
        }))
    }

    // ── JavaScript ─────────────────────────────────────────────────────

    /// Evaluate a JavaScript expression in the page context.
    pub async fn evaluate(&self, expression: &str) -> BitFunResult<Value> {
        self.evaluate_with_options(expression, true, true).await
    }

    pub async fn evaluate_with_options(
        &self,
        expression: &str,
        await_promise: bool,
        return_by_value: bool,
    ) -> BitFunResult<Value> {
        let mut last_error: Option<BitFunError> = None;
        for attempt in 0..2 {
            let result = self
                .client
                .send(
                    "Runtime.evaluate",
                    Some(json!({
                        "expression": expression,
                        "returnByValue": return_by_value,
                        "awaitPromise": await_promise,
                    })),
                )
                .await;
            match result {
                Ok(value) => {
                    if let Some(details) = value.get("exceptionDetails") {
                        let message = details
                            .get("exception")
                            .and_then(|e| e.get("description"))
                            .and_then(|v| v.as_str())
                            .or_else(|| details.get("text").and_then(|v| v.as_str()))
                            .unwrap_or("Runtime.evaluate failed");
                        return Err(BitFunError::tool(format!("JS error: {}", message)));
                    }
                    return Ok(value);
                }
                Err(err) => {
                    let message = err.to_string();
                    let retryable = message.contains("Inspected target navigated")
                        || message.contains("Target closed")
                        || message.contains("Cannot find context with specified id");
                    last_error = Some(err);
                    if retryable && attempt == 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        continue;
                    }
                    break;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| BitFunError::tool("Runtime.evaluate failed".to_string())))
    }

    pub async fn get_cookies(&self, urls: Option<Vec<String>>) -> BitFunResult<Value> {
        let params = urls
            .filter(|items| !items.is_empty())
            .map(|urls| json!({ "urls": urls }))
            .unwrap_or_else(|| json!({}));
        let result = self.client.send("Network.getCookies", Some(params)).await?;
        Ok(json!({
            "success": true,
            "action": "cookies",
            "cookies": result.get("cookies").cloned().unwrap_or_else(|| json!([])),
        }))
    }

    pub async fn set_cookies(&self, cookies: &[Value]) -> BitFunResult<Value> {
        let mut set = 0usize;
        let mut errors = Vec::<Value>::new();
        for cookie in cookies {
            match self
                .client
                .send("Network.setCookie", Some(cookie.clone()))
                .await
            {
                Ok(result)
                    if result
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true) =>
                {
                    set += 1;
                }
                Ok(result) => errors.push(json!({ "cookie": cookie, "result": result })),
                Err(err) => errors.push(json!({ "cookie": cookie, "error": err.to_string() })),
            }
        }
        Ok(json!({
            "success": errors.is_empty(),
            "action": "set_cookies",
            "set": set,
            "errors": errors,
        }))
    }

    pub async fn set_file_input_files(
        &self,
        selector: Option<&str>,
        files: &[String],
    ) -> BitFunResult<Value> {
        if files.is_empty() {
            return Err(BitFunError::tool(
                "set_file_input_files requires non-empty 'files'".to_string(),
            ));
        }
        let query = selector.unwrap_or("input[type=\"file\"]");
        let css_selector = if query.starts_with("@e") {
            format!(r#"[data-cdp-ref="{}"]"#, query)
        } else {
            query.to_string()
        };
        let document = self.client.send("DOM.getDocument", None).await?;
        let root_id = document
            .get("root")
            .and_then(|r| r.get("nodeId"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| BitFunError::tool("DOM.getDocument: missing root nodeId".to_string()))?;
        let node = self
            .client
            .send(
                "DOM.querySelector",
                Some(json!({ "nodeId": root_id, "selector": css_selector })),
            )
            .await?;
        let node_id = node.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
        if node_id == 0 {
            return Err(BitFunError::tool(format!(
                "No file input found for selector: {}",
                query
            )));
        }
        self.client
            .send(
                "DOM.setFileInputFiles",
                Some(json!({ "nodeId": node_id, "files": files })),
            )
            .await?;
        Ok(json!({
            "success": true,
            "action": "set_file_input_files",
            "selector": query,
            "count": files.len(),
        }))
    }

    pub async fn fetch(
        &self,
        url: &str,
        method: &str,
        headers: Value,
        body: Option<&str>,
    ) -> BitFunResult<Value> {
        let script = format!(
            r#"(async () => {{
                try {{
                    const init = {{
                        method: {method},
                        credentials: 'include',
                        headers: {headers}
                    }};
                    const body = {body};
                    if (body !== null && init.method !== 'GET' && init.method !== 'HEAD') init.body = body;
                    const resp = await fetch({url}, init);
                    const contentType = resp.headers.get('content-type') || '';
                    let responseBody;
                    if (contentType.includes('application/json') && resp.status !== 204) {{
                        try {{ responseBody = await resp.json(); }} catch (_) {{ responseBody = await resp.text(); }}
                    }} else {{
                        responseBody = await resp.text();
                    }}
                    return JSON.stringify({{
                        ok: resp.ok,
                        status: resp.status,
                        status_text: resp.statusText,
                        content_type: contentType,
                        url: resp.url,
                        body: responseBody
                    }});
                }} catch (e) {{
                    return JSON.stringify({{ error: String(e && e.message || e) }});
                }}
            }})()"#,
            url = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string()),
            method = serde_json::to_string(&method.to_uppercase())
                .unwrap_or_else(|_| "\"GET\"".to_string()),
            headers = headers,
            body = body
                .map(|b| serde_json::to_string(b).unwrap_or_else(|_| "null".to_string()))
                .unwrap_or_else(|| "null".to_string()),
        );
        let result = self.evaluate(&script).await?;
        let raw = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let parsed: Value = serde_json::from_str(raw).unwrap_or(json!({}));
        Ok(json!({ "success": parsed.get("error").is_none(), "action": "fetch", "result": parsed }))
    }

    pub async fn read_article(&self) -> BitFunResult<Value> {
        let script = r#"
        (function() {
            function textOf(node) {
                return (node && node.textContent || '').replace(/\s+/g, ' ').trim();
            }
            const article = document.querySelector('article') || document.querySelector('main') || document.body;
            const title = document.querySelector('meta[property="og:title"]')?.content || document.title || '';
            const description = document.querySelector('meta[name="description"]')?.content || document.querySelector('meta[property="og:description"]')?.content || '';
            const siteName = document.querySelector('meta[property="og:site_name"]')?.content || location.hostname;
            const publishedTime = document.querySelector('meta[property="article:published_time"]')?.content || document.querySelector('time[datetime]')?.getAttribute('datetime') || null;
            const textContent = textOf(article);
            const headings = Array.from(article.querySelectorAll('h1,h2,h3')).slice(0, 20).map(h => ({ level: h.tagName.toLowerCase(), text: textOf(h) })).filter(h => h.text);
            return {
                title,
                description,
                siteName,
                publishedTime,
                url: location.href,
                length: textContent.length,
                excerpt: textContent.slice(0, 500),
                textContent,
                headings,
            };
        })()
        "#;
        let result = self.evaluate(script).await?;
        Ok(json!({
            "success": true,
            "action": "read_article",
            "article": result.get("result").and_then(|r| r.get("value")).cloned().unwrap_or(Value::Null),
        }))
    }

    // ── Close ──────────────────────────────────────────────────────────

    pub async fn close_page(&self) -> BitFunResult<Value> {
        let _ = self.client.send("Page.close", None).await;
        Ok(json!({ "success": true, "action": "close" }))
    }

    // ── Internal helpers ───────────────────────────────────────────────

    /// Generate JS to resolve an element from selector or `@eN` ref.
    ///
    /// Phase 3: ref / selector lookup walks open shadow roots and
    /// same-origin iframes so refs / selectors created by `snapshot()` for
    /// elements inside a shadow root or iframe actually resolve. The legacy
    /// `document.querySelector` path returned `null` for any element nested
    /// inside a shadow root, which made `click @e7` mysteriously fail
    /// whenever the page used a web-component design system.
    fn resolve_element_js(selector: &str) -> String {
        let attr_selector = if selector.starts_with("@e") {
            format!(r#"[data-cdp-ref="{}"]"#, selector)
        } else {
            selector.to_string()
        };
        let escaped = attr_selector.replace('\\', "\\\\").replace('\'', "\\'");
        format!(
            r#"
            const __sel = '{escaped}';
            function __findIn(root) {{
                try {{
                    const direct = root.querySelector(__sel);
                    if (direct) return direct;
                }} catch (_) {{}}
                const all = root.querySelectorAll('*');
                for (const node of all) {{
                    if (node.shadowRoot) {{
                        const hit = __findIn(node.shadowRoot);
                        if (hit) return hit;
                    }}
                }}
                return null;
            }}
            function __findAnywhere() {{
                const top = __findIn(document);
                if (top) return top;
                const frames = document.querySelectorAll('iframe, frame');
                for (const f of frames) {{
                    let doc = null;
                    try {{ doc = f.contentDocument; }} catch (_) {{}}
                    if (doc) {{
                        const hit = __findIn(doc);
                        if (hit) return hit;
                    }}
                }}
                return null;
            }}
            const el = __findAnywhere();
            if (!el) throw new Error('Element not found: ' + __sel + ' — take a fresh snapshot or check shadow/iframe scope');
            "#,
            escaped = escaped
        )
    }
}
