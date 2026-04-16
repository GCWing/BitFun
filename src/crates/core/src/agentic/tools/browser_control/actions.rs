//! Atomic browser actions implemented via CDP commands.

use super::cdp_client::CdpClient;
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::{json, Value};

/// High-level browser actions backed by CDP method calls.
pub struct BrowserActions<'a> {
    client: &'a CdpClient,
}

impl<'a> BrowserActions<'a> {
    pub fn new(client: &'a CdpClient) -> Self {
        Self { client }
    }

    // ── Navigation ─────────────────────────────────────────────────────

    pub async fn navigate(&self, url: &str) -> BitFunResult<Value> {
        let result = self
            .client
            .send("Page.navigate", Some(json!({ "url": url })))
            .await?;
        // Wait for load event
        let _ = self.client.send("Page.enable", None).await;
        Ok(json!({
            "success": true,
            "url": url,
            "frameId": result.get("frameId"),
        }))
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
    pub async fn snapshot(&self) -> BitFunResult<Value> {
        let script = r#"
        (function() {
            const selectors = 'a, button, input, textarea, select, [role="button"], [role="link"], [role="tab"], [role="menuitem"], [role="combobox"], [role="option"], [tabindex="0"], [contenteditable="true"]';
            const els = document.querySelectorAll(selectors);
            const items = [];
            let idx = 1;
            els.forEach(el => {
                const rect = el.getBoundingClientRect();
                if (rect.width < 2 || rect.height < 2) return;
                if (rect.right < 0 || rect.bottom < 0 || rect.left > window.innerWidth || rect.top > window.innerHeight) return;
                const style = window.getComputedStyle(el);
                if (style.display === 'none' || style.visibility === 'hidden') return;
                const text = (el.textContent || '').trim().slice(0, 100);
                const tag = el.tagName.toLowerCase();
                const type = el.getAttribute('type') || '';
                const name = el.getAttribute('name') || '';
                const ariaLabel = el.getAttribute('aria-label') || '';
                const placeholder = el.placeholder || '';
                const role = el.getAttribute('role') || '';
                const href = el.href || '';
                const id = el.id || '';
                items.push({
                    ref: '@e' + idx,
                    tag, type, name, text, ariaLabel, placeholder, role, href, id,
                    rect: { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) }
                });
                el.setAttribute('data-cdp-ref', '@e' + idx);
                idx++;
            });
            return JSON.stringify({ url: location.href, title: document.title, elements: items });
        })()
        "#;
        let result = self.evaluate(script).await?;
        let text = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let parsed: Value = serde_json::from_str(text).unwrap_or(json!({}));
        Ok(parsed)
    }

    /// Get the text content of an element by CSS selector.
    pub async fn get_text(&self, selector: &str) -> BitFunResult<String> {
        let js = format!(
            r#"(function(){{ const el = document.querySelector('{}'); return el ? (el.textContent || '').trim().slice(0, 5000) : null; }})()"#,
            selector.replace('\'', "\\'")
        );
        let result = self.evaluate(&js).await?;
        let text = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(text)
    }

    // ── Interaction ────────────────────────────────────────────────────

    /// Click an element by CSS selector or by `@eN` ref.
    pub async fn click(&self, selector: &str) -> BitFunResult<Value> {
        let js = Self::resolve_element_js(selector);
        let center_js = format!(
            r#"(function(){{ {} const rect = el.getBoundingClientRect(); return JSON.stringify({{ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }}); }})()"#,
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
        let delta_y = match direction {
            "up" => -px,
            "down" => px,
            "top" => {
                self.evaluate("window.scrollTo(0, 0)").await?;
                return Ok(json!({ "success": true, "action": "scroll", "direction": "top" }));
            }
            "bottom" => {
                self.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                    .await?;
                return Ok(json!({ "success": true, "action": "scroll", "direction": "bottom" }));
            }
            _ => px,
        };
        self.client
            .send(
                "Input.dispatchMouseEvent",
                Some(json!({
                    "type": "mouseWheel",
                    "x": 400, "y": 300,
                    "deltaX": 0, "deltaY": delta_y,
                })),
            )
            .await?;
        Ok(json!({ "success": true, "action": "scroll", "direction": direction, "amount": px }))
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
                "networkidle" | "load" => {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
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
        let result = self
            .client
            .send(
                "Page.captureScreenshot",
                Some(json!({ "format": "jpeg", "quality": 80 })),
            )
            .await?;
        let data = result.get("data").and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!({
            "success": true,
            "action": "screenshot",
            "format": "jpeg",
            "data_length": data.len(),
            "base64_data": data,
        }))
    }

    // ── JavaScript ─────────────────────────────────────────────────────

    /// Evaluate a JavaScript expression in the page context.
    pub async fn evaluate(&self, expression: &str) -> BitFunResult<Value> {
        self.client
            .send(
                "Runtime.evaluate",
                Some(json!({
                    "expression": expression,
                    "returnByValue": true,
                })),
            )
            .await
    }

    // ── Close ──────────────────────────────────────────────────────────

    pub async fn close_page(&self) -> BitFunResult<Value> {
        let _ = self.client.send("Page.close", None).await;
        Ok(json!({ "success": true, "action": "close" }))
    }

    // ── Internal helpers ───────────────────────────────────────────────

    /// Generate JS to resolve an element from selector or `@eN` ref.
    fn resolve_element_js(selector: &str) -> String {
        if selector.starts_with("@e") {
            format!(
                r#"const el = document.querySelector('[data-cdp-ref="{}"]'); if (!el) throw new Error('Ref {} not found — take a fresh snapshot');"#,
                selector, selector
            )
        } else {
            format!(
                r#"const el = document.querySelector('{}'); if (!el) throw new Error('Element not found: {}');"#,
                selector.replace('\'', "\\'"),
                selector.replace('\'', "\\'")
            )
        }
    }
}
