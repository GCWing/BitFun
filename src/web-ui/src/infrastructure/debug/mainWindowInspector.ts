/**
 * Main-window element inspector for desktop debugging.
 *
 * Injected into the main BitFun webview to provide interactive element
 * inspection without relying on external DevTools.
 *
 * Activation:  Cmd/Ctrl + Shift + I  (or programmatic via toggleInspector())
 * Exit:        Escape key
 *
 * Features:
 *  - Hover highlight with animated overlay
 *  - Tooltip showing tag, id, class, dimensions, color, background
 *  - Click to capture full element metadata (computed styles, CSS vars,
 *    box model, colors, attributes) and send to Rust via Tauri command
 *  - Zero performance impact when inactive
 */

const INSPECTOR_SCRIPT_BODY = /* js */ `
(function () {
  if (window.__bitfun_main_inspector_active) {
    window.__bitfun_main_inspector_cancel && window.__bitfun_main_inspector_cancel();
    return;
  }
  window.__bitfun_main_inspector_active = true;

  // ── overlay elements ─────────────────────────────────────────────────────
  var overlay = document.createElement('div');
  overlay.style.cssText = [
    'position:fixed',
    'border:2px solid #3b82f6',
    'background:rgba(59,130,246,0.12)',
    'pointer-events:none',
    'z-index:2147483646',
    'box-sizing:border-box',
    'display:none',
    'border-radius:2px',
    'transition:top 0.05s,left 0.05s,width 0.05s,height 0.05s',
  ].join(';');

  var tooltip = document.createElement('div');
  tooltip.style.cssText = [
    'position:fixed',
    'background:rgba(15,23,42,0.95)',
    'color:#e2e8f0',
    'padding:8px 12px',
    'border-radius:6px',
    'font-size:11px',
    'font-family:ui-monospace,SFMono-Regular,Menlo,monospace',
    'z-index:2147483647',
    'pointer-events:none',
    'display:none',
    'max-width:520px',
    'box-shadow:0 4px 12px rgba(0,0,0,0.5)',
    'line-height:1.5',
    'border:1px solid rgba(59,130,246,0.4)',
  ].join(';');

  var sizeLabel = document.createElement('div');
  sizeLabel.style.cssText = [
    'position:fixed',
    'background:#3b82f6',
    'color:#fff',
    'padding:2px 6px',
    'border-radius:3px',
    'font-size:10px',
    'font-family:monospace',
    'z-index:2147483647',
    'pointer-events:none',
    'display:none',
    'white-space:nowrap',
  ].join(';');

  document.documentElement.appendChild(overlay);
  document.documentElement.appendChild(tooltip);
  document.documentElement.appendChild(sizeLabel);

  var hoveredEl = null;

  // ── helpers ──────────────────────────────────────────────────────────────
  function cssPath(el) {
    var parts = [];
    var cur = el;
    while (cur && cur.nodeType === 1 && cur !== document.documentElement) {
      var seg = cur.tagName.toLowerCase();
      if (cur.id) {
        try { seg += '#' + CSS.escape(cur.id); } catch (e) { seg += '#' + cur.id; }
        parts.unshift(seg);
        break;
      }
      if (cur.parentElement) {
        var siblings = Array.prototype.filter.call(
          cur.parentElement.children,
          function (s) { return s.tagName === cur.tagName; }
        );
        if (siblings.length > 1) {
          seg += ':nth-of-type(' + (siblings.indexOf(cur) + 1) + ')';
        }
      }
      var classes = Array.prototype.slice.call(cur.classList, 0, 3).join('.');
      if (classes) seg += '.' + classes;
      parts.unshift(seg);
      cur = cur.parentElement;
    }
    return parts.join(' > ') || 'html';
  }

  function getComputedStyles(el) {
    var cs = window.getComputedStyle(el);
    return {
      display: cs.display,
      position: cs.position,
      width: cs.width,
      height: cs.height,
      margin: cs.margin,
      padding: cs.padding,
      border: cs.border,
      borderRadius: cs.borderRadius,
      backgroundColor: cs.backgroundColor,
      color: cs.color,
      fontSize: cs.fontSize,
      fontFamily: cs.fontFamily,
      zIndex: cs.zIndex,
      opacity: cs.opacity,
      overflow: cs.overflow,
      boxShadow: cs.boxShadow,
    };
  }

  function getCSSVariables(el) {
    var vars = {};
    var cs = window.getComputedStyle(el);
    for (var i = 0; i < cs.length; i++) {
      var prop = cs[i];
      if (prop.startsWith('--')) {
        vars[prop] = cs.getPropertyValue(prop);
      }
    }
    return vars;
  }

  function getColorInfo(el) {
    var cs = window.getComputedStyle(el);
    return {
      color: cs.color,
      backgroundColor: cs.backgroundColor,
      borderColor: cs.borderColor,
      borderTopColor: cs.borderTopColor,
      borderRightColor: cs.borderRightColor,
      borderBottomColor: cs.borderBottomColor,
      borderLeftColor: cs.borderLeftColor,
      outlineColor: cs.outlineColor,
    };
  }

  function getBoxModel(el) {
    var rect = el.getBoundingClientRect();
    var cs = window.getComputedStyle(el);
    return {
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
      top: rect.top,
      left: rect.left,
      margin: {
        top: cs.marginTop,
        right: cs.marginRight,
        bottom: cs.marginBottom,
        left: cs.marginLeft,
      },
      padding: {
        top: cs.paddingTop,
        right: cs.paddingRight,
        bottom: cs.paddingBottom,
        left: cs.paddingLeft,
      },
      border: {
        top: cs.borderTopWidth,
        right: cs.borderRightWidth,
        bottom: cs.borderBottomWidth,
        left: cs.borderLeftWidth,
      },
    };
  }

  function tooltipContent(el) {
    var tag = el.tagName.toLowerCase();
    var id = el.id ? '#' + el.id : '';
    var cls = el.classList.length
      ? '.' + Array.prototype.slice.call(el.classList, 0, 2).join('.')
      : '';
    var rect = el.getBoundingClientRect();
    var cs = window.getComputedStyle(el);
    return tag + id + cls + '\n' +
      'size: ' + Math.round(rect.width) + ' x ' + Math.round(rect.height) + '\n' +
      'color: ' + cs.color + '\n' +
      'bg: ' + cs.backgroundColor;
  }

  function updateOverlay(el) {
    if (!el) {
      overlay.style.display = 'none';
      tooltip.style.display = 'none';
      sizeLabel.style.display = 'none';
      return;
    }
    var rect = el.getBoundingClientRect();
    overlay.style.display = 'block';
    overlay.style.top = rect.top + 'px';
    overlay.style.left = rect.left + 'px';
    overlay.style.width = rect.width + 'px';
    overlay.style.height = rect.height + 'px';

    tooltip.innerHTML = tooltipContent(el).replace(/\n/g, '<br>');
    tooltip.style.display = 'block';
    var ty = rect.top - tooltip.offsetHeight - 8;
    if (ty < 4) ty = rect.bottom + 8;
    var tx = rect.left;
    if (tx + tooltip.offsetWidth > window.innerWidth) {
      tx = window.innerWidth - tooltip.offsetWidth - 4;
    }
    if (tx < 4) tx = 4;
    tooltip.style.top = ty + 'px';
    tooltip.style.left = tx + 'px';

    sizeLabel.textContent = Math.round(rect.width) + ' x ' + Math.round(rect.height);
    sizeLabel.style.display = 'block';
    sizeLabel.style.top = (rect.bottom + 4) + 'px';
    sizeLabel.style.left = rect.left + 'px';
  }

  function invokeTauri(cmd, payload) {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      window.__TAURI_INTERNALS__.invoke(cmd, { request: payload }).catch(function(){});
    }
  }

  // ── cleanup ──────────────────────────────────────────────────────────────
  function cleanup() {
    document.removeEventListener('mouseover', onMouseOver, true);
    document.removeEventListener('click', onClick, true);
    document.removeEventListener('keydown', onKeyDown, true);
    try { overlay.parentNode && overlay.parentNode.removeChild(overlay); } catch (e) {}
    try { tooltip.parentNode && tooltip.parentNode.removeChild(tooltip); } catch (e) {}
    try { sizeLabel.parentNode && sizeLabel.parentNode.removeChild(sizeLabel); } catch (e) {}
    delete window.__bitfun_main_inspector_active;
    delete window.__bitfun_main_inspector_cancel;
  }

  // ── event handlers ───────────────────────────────────────────────────────
  function onMouseOver(e) {
    var el = e.target;
    if (el === overlay || el === tooltip || el === sizeLabel) return;
    hoveredEl = el;
    updateOverlay(el);
  }

  function onClick(e) {
    if (!hoveredEl) return;
    e.preventDefault();
    e.stopPropagation();

    var data = {
      tagName: hoveredEl.tagName.toLowerCase(),
      path: cssPath(hoveredEl),
      id: hoveredEl.id || null,
      className: hoveredEl.className || null,
      textContent: (hoveredEl.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 500),
      outerHTML: (hoveredEl.outerHTML || '').slice(0, 2000),
      computedStyles: getComputedStyles(hoveredEl),
      cssVariables: getCSSVariables(hoveredEl),
      colorInfo: getColorInfo(hoveredEl),
      boxModel: getBoxModel(hoveredEl),
      attributes: (function() {
        var attrs = {};
        for (var i = 0; i < hoveredEl.attributes.length; i++) {
          attrs[hoveredEl.attributes[i].name] = hoveredEl.attributes[i].value;
        }
        return attrs;
      })(),
    };

    invokeTauri('debug_element_picked', data);

    overlay.style.borderColor = '#22c55e';
    overlay.style.background = 'rgba(34,197,94,0.18)';
    setTimeout(function () {
      overlay.style.borderColor = '#3b82f6';
      overlay.style.background = 'rgba(59,130,246,0.12)';
    }, 400);
  }

  function onKeyDown(e) {
    if (e.key === 'Escape') {
      cleanup();
    }
  }

  window.__bitfun_main_inspector_cancel = cleanup;

  document.addEventListener('mouseover', onMouseOver, true);
  document.addEventListener('click', onClick, true);
  document.addEventListener('keydown', onKeyDown, true);
})();
`;

/** Returns the JavaScript string to eval() inside the main webview. */
export function createMainWindowInspectorScript(): string {
  return INSPECTOR_SCRIPT_BODY;
}

/** Script to cancel an active inspector session. */
export const CANCEL_MAIN_WINDOW_INSPECTOR_SCRIPT =
  `if (window.__bitfun_main_inspector_cancel) { window.__bitfun_main_inspector_cancel(); }`;

/** Check whether the inspector is currently active in the page. */
export const IS_INSPECTOR_ACTIVE_SCRIPT =
  `typeof window.__bitfun_main_inspector_active !== 'undefined' && window.__bitfun_main_inspector_active`;
