(function() {
            const SEL = 'a, button, input, textarea, select, [role="button"], [role="link"], [role="tab"], [role="menuitem"], [role="combobox"], [role="option"], [tabindex="0"], [contenteditable="true"]';
            const items = [];
            let idx = 1;
            let offscreen = 0;
            let crossOriginFrames = 0;

            function visible(el, win) {
                const rect = el.getBoundingClientRect();
                if (rect.width < 2 || rect.height < 2) return null;
                if (rect.right <= 0 || rect.bottom <= 0 || rect.left >= win.innerWidth || rect.top >= win.innerHeight) {
                    offscreen++;
                    return null;
                }
                const style = win.getComputedStyle(el);
                if (style.display === 'none' || style.visibility === 'hidden') return null;
                // A child viewport can be visible while its containing frame is
                // hidden or outside a parent viewport. Check every frame boundary.
                let owner = win;
                while (owner.frameElement) {
                    const frame = owner.frameElement;
                    const parent = frame.ownerDocument.defaultView;
                    const fr = frame.getBoundingClientRect();
                    const fs = parent.getComputedStyle(frame);
                    if (fr.width < 2 || fr.height < 2 || fs.display === 'none' || fs.visibility === 'hidden') return null;
                    if (fr.right <= 0 || fr.bottom <= 0 || fr.left >= parent.innerWidth || fr.top >= parent.innerHeight) { offscreen++; return null; }
                    owner = parent;
                }
                return rect;
            }

            function record(el, rect, scope, framePath) {
                const rawText = (el.textContent || '').trim();
                const text = Array.from(rawText).slice(0, 100).join('');
                const labelledBy = (el.getAttribute('aria-labelledby') || '').split(/\s+/)
                    .map(id => el.getRootNode().getElementById?.(id)?.textContent?.trim() || '').filter(Boolean).join(' ');
                const label = labelledBy || Array.from(el.labels || []).map(l => l.textContent.trim()).filter(Boolean).join(' ');
                items.push({
                    ref: '@e' + idx,
                    tag: el.tagName.toLowerCase(),
                    type: el.getAttribute('type') || '',
                    name: el.getAttribute('name') || '',
                    text,
                    text_truncated: Array.from(rawText).length > 100,
                    label,
                    value: el.type === 'password' ? undefined : (typeof el.value === 'string' ? Array.from(el.value).slice(0, 2000).join('') : undefined),
                    value_truncated: el.type !== 'password' && typeof el.value === 'string' && Array.from(el.value).length > 2000,
                    disabled: el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true',
                    checked: el.hasAttribute('aria-checked') ? el.getAttribute('aria-checked') : (el.type === 'checkbox' || el.type === 'radio' ? el.checked : undefined),
                    selected: el.hasAttribute('aria-selected') ? el.getAttribute('aria-selected') : undefined,
                    expanded: el.hasAttribute('aria-expanded') ? el.getAttribute('aria-expanded') : undefined,
                    rect_coordinate_space: 'frame_viewport',
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

            // Every snapshot renumbers refs from @e1, so refs left behind by
            // the previous snapshot MUST be dropped first: an element that
            // dropped out of this snapshot would otherwise keep an @eN that
            // the new numbering hands to a different element, and
            // `click @eN` — which resolves by attribute — would silently hit
            // the stale one.
            function clearRefs(root) {
                try {
                    root.querySelectorAll('[data-cdp-ref]').forEach(el => el.removeAttribute('data-cdp-ref'));
                } catch (_) {}
                try {
                    root.querySelectorAll('*').forEach(host => {
                        if (host.shadowRoot) clearRefs(host.shadowRoot);
                        if (host.tagName === 'IFRAME' || host.tagName === 'FRAME') {
                            let doc = null;
                            try { doc = host.contentDocument; } catch (_) {}
                            if (doc) clearRefs(doc);
                        }
                    });
                } catch (_) {}
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
                let frameIndex = 0;
                allHosts.forEach((h, hostIndex) => {
                    if (h.shadowRoot) {
                        walk(h.shadowRoot, win, 'shadow', `${framePath ? framePath + '/' : ''}shadow[${hostIndex}]`);
                    }
                    if (h.tagName === 'IFRAME' || h.tagName === 'FRAME') {
                        let doc = null;
                        try { doc = h.contentDocument; } catch (_) {}
                        const localPath = `iframe[${frameIndex++}]`;
                        const path = framePath ? `${framePath}/${localPath}` : localPath;
                        if (doc) {
                            walk(doc, h.contentWindow, 'iframe', path);
                        } else {
                            crossOriginFrames++;
                        }
                    }
                });
            }

            clearRefs(document);
            walk(document, window, 'document', '');

            return JSON.stringify({
                url: location.href,
                title: document.title,
                elements: items,
                offscreen_count: offscreen,
                cross_origin_frames: crossOriginFrames,
                features: { shadow_dom_traversed: true, same_origin_iframes_traversed: true, viewport_only: true },
            });
        })()
