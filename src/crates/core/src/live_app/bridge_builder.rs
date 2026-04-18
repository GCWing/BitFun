//! Bridge script builder — generate window.app Runtime Adapter (BitFun Hosted) for iframe.

use crate::live_app::types::{EsmDep, LiveAppPermissions};
use serde_json;

/// Build the Runtime Adapter script (JS) to inject into the iframe.
/// Exposes window.app with call(), fs.*, shell.*, net.*, os.*, storage.*, dialog.*,
/// ai.*, clipboard.*, lifecycle, events.
pub fn build_bridge_script(
    app_id: &str,
    app_data_dir: &str,
    workspace_dir: &str,
    theme: &str,
    platform: &str,
) -> String {
    let app_id_esc = escape_js_str(app_id);
    let app_data_esc = escape_js_str(app_data_dir);
    let workspace_esc = escape_js_str(workspace_dir);
    let theme_esc = escape_js_str(theme);
    let platform_esc = escape_js_str(platform);

    format!(
        r#"
(function() {{
  const _rpc = (method, params) => {{
    return new Promise((resolve, reject) => {{
      const id = 'rpc-' + Math.random().toString(36).slice(2) + '-' + Date.now();
      const handler = (e) => {{
        if (!e.data || e.data.id !== id) return;
        window.removeEventListener('message', handler);
        if (e.data.error) reject(new Error(e.data.error.message || 'RPC error'));
        else resolve(e.data.result);
      }};
      window.addEventListener('message', handler);
      window.parent.postMessage({{ jsonrpc: '2.0', id, method, params }}, '*');
    }});
  }};

  const _call = (method, params) => _rpc('worker.call', {{ method, params: params || {{}} }});

  function _applyThemeVars(vars) {{
    if (!vars || typeof vars !== 'object') return;
    const root = document.documentElement.style;
    for (const k of Object.keys(vars)) root.setProperty(k, vars[k]);
  }}

  let _theme = {theme_esc};

  const app = {{
    get theme() {{ return _theme; }},
    appId: {app_id_esc},
    appDataDir: {app_data_esc},
    workspaceDir: {workspace_esc},
    platform: {platform_esc},
    mode: 'hosted',

    call: _call,

    fs: {{
      readFile:   (p, opts) => _call('fs.readFile', {{ path: p, ...(opts||{{}}) }}),
      writeFile:  (p, data, opts) => _call('fs.writeFile', {{ path: p, data: typeof data === 'string' ? data : (data && data.toString ? data.toString() : ''), ...(opts||{{}}) }}),
      readdir:    (p, opts) => _call('fs.readdir', {{ path: p, ...(opts||{{}}) }}),
      stat:       (p) => _call('fs.stat', {{ path: p }}),
      mkdir:      (p, opts) => _call('fs.mkdir', {{ path: p, ...(opts||{{}}) }}),
      rm:         (p, opts) => _call('fs.rm', {{ path: p, ...(opts||{{}}) }}),
      copyFile:   (s, d) => _call('fs.copyFile', {{ src: s, dst: d }}),
      rename:     (o, n) => _call('fs.rename', {{ oldPath: o, newPath: n }}),
      appendFile: (p, data) => _call('fs.appendFile', {{ path: p, data: typeof data === 'string' ? data : String(data) }}),
    }},
    shell: {{ exec: (cmd, opts) => _call('shell.exec', {{ command: cmd, ...(opts||{{}}) }}) }},
    net:   {{ fetch: (url, opts) => _call('net.fetch', {{ url: typeof url === 'string' ? url : (url && url.url), ...(opts||{{}}) }}) }},
    os:    {{ info: () => _call('os.info', {{}}) }},
    storage: {{
      get: (key) => _call('storage.get', {{ key }}),
      set: (key, value) => _call('storage.set', {{ key, value }}),
    }},

    dialog: {{
      open:    (opts) => _rpc('dialog.open', opts || {{}}),
      save:    (opts) => _rpc('dialog.save', opts || {{}}),
      message: (opts) => _rpc('dialog.message', opts || {{}}),
    }},

    // AI namespace — proxies to host application AI client (no API key exposure).
    _aiStreams: {{}},
    ai: {{
      complete: (prompt, opts) => _rpc('ai.complete', {{ prompt, ...(opts || {{}}) }}),
      chat: (messages, opts) => {{
        const streamId = 'ai-stream-' + Math.random().toString(36).slice(2) + '-' + Date.now();
        const handlers = {{
          onChunk: opts && opts.onChunk,
          onDone:  opts && opts.onDone,
          onError: opts && opts.onError,
        }};
        app._aiStreams[streamId] = handlers;
        const rpcOpts = {{}};
        if (opts) {{
          if (opts.systemPrompt !== undefined) rpcOpts.systemPrompt = opts.systemPrompt;
          if (opts.model !== undefined) rpcOpts.model = opts.model;
          if (opts.maxTokens !== undefined) rpcOpts.maxTokens = opts.maxTokens;
          if (opts.temperature !== undefined) rpcOpts.temperature = opts.temperature;
        }}
        return _rpc('ai.chat', {{ messages, streamId, ...rpcOpts }}).then((result) => ({{
          streamId: result && result.streamId ? result.streamId : streamId,
          cancel: () => _rpc('ai.cancel', {{ streamId }}),
        }}));
      }},
      cancel:    (streamId) => _rpc('ai.cancel', {{ streamId }}),
      getModels: () => _rpc('ai.getModels', {{}}),
    }},

    // Clipboard namespace — proxies to host navigator.clipboard (bypasses sandbox restriction).
    clipboard: {{
      writeText: (text) => _rpc('clipboard.writeText', {{ text }}),
      readText:  () => _rpc('clipboard.readText', {{}}),
    }},

    _lifecycleHandlers: {{ activate: [], deactivate: [], themeChange: [] }},
    onActivate:   (fn) => app._lifecycleHandlers.activate.push(fn),
    onDeactivate: (fn) => app._lifecycleHandlers.deactivate.push(fn),
    onThemeChange: (fn) => app._lifecycleHandlers.themeChange.push(fn),

    _eventHandlers: {{}},
    on:  (event, fn) => {{ (app._eventHandlers[event] = app._eventHandlers[event] || []).push(fn); }},
    off: (event, fn) => {{
      if (app._eventHandlers[event])
        app._eventHandlers[event] = app._eventHandlers[event].filter(f => f !== fn);
    }},
  }};

  window.addEventListener('message', (e) => {{
    if (e.data?.type === 'bitfun:event') {{
      const {{ event, payload }} = e.data;
      if (event === 'activate')    app._lifecycleHandlers.activate.forEach(f => f());
      if (event === 'deactivate')  app._lifecycleHandlers.deactivate.forEach(f => f());
      if (event === 'themeChange') {{
        if (payload && typeof payload === 'object') {{
          if (payload.vars) _applyThemeVars(payload.vars);
          if (payload.type) {{ _theme = payload.type; document.documentElement.setAttribute('data-theme-type', _theme); }}
        }}
        app._lifecycleHandlers.themeChange.forEach(f => f(payload));
        (app._eventHandlers[event] || []).forEach(f => f(payload));
      }} else if (event === 'ai:stream') {{
        // Route AI stream chunks to the registered callbacks
        if (payload && payload.streamId) {{
          const h = app._aiStreams[payload.streamId];
          if (h) {{
            if (payload.type === 'chunk' && h.onChunk) h.onChunk(payload.data || {{}});
            if (payload.type === 'done') {{
              if (h.onDone) h.onDone(payload.data || {{}});
              delete app._aiStreams[payload.streamId];
            }}
            if (payload.type === 'error') {{
              if (h.onError) h.onError(payload.data || {{}});
              delete app._aiStreams[payload.streamId];
            }}
          }}
        }}
      }} else if (event === 'worker:event') {{
        // Forward Worker push events to registered app.on('worker:*', ...) handlers
        if (payload && payload.event) {{
          const evtKey = 'worker:' + payload.event;
          (app._eventHandlers[evtKey] || []).forEach(f => f(payload.data));
          (app._eventHandlers['worker:*'] || []).forEach(f => f(payload.event, payload.data));
        }}
      }} else {{
        (app._eventHandlers[event] || []).forEach(f => f(payload));
      }}
    }}
  }});

  window.app = app;
  document.documentElement.setAttribute('data-theme-type', _theme);
  window.parent.postMessage({{ method: 'bitfun/request-theme' }}, '*');
}})();
"#,
        app_id_esc = app_id_esc,
        app_data_esc = app_data_esc,
        workspace_esc = workspace_esc,
        theme_esc = theme_esc,
        platform_esc = platform_esc
    )
}

fn escape_js_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Build Import Map script tag from ESM dependencies (esm.sh URLs).
pub fn build_import_map(deps: &[EsmDep]) -> String {
    let mut imports = serde_json::Map::new();
    for dep in deps {
        let url = dep.url.clone().unwrap_or_else(|| match &dep.version {
            Some(v) => format!("https://esm.sh/{}@{}", dep.name, v),
            None => format!("https://esm.sh/{}", dep.name),
        });
        imports.insert(dep.name.clone(), serde_json::Value::String(url));
    }
    let json = serde_json::json!({ "imports": imports });
    format!(r#"<script type="importmap">{}</script>"#, json)
}

/// Build CSP meta content from permissions (net.allow → connect-src).
pub fn build_csp_content(permissions: &LiveAppPermissions) -> String {
    let net_allow = permissions
        .net
        .as_ref()
        .and_then(|n| n.allow.as_ref())
        .map(|v| v.iter().map(|d| d.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    let connect_src = if net_allow.is_empty() {
        "'self'".to_string()
    } else if net_allow.contains(&"*") {
        "'self' *".to_string()
    } else {
        let safe: Vec<String> = net_allow
            .iter()
            .map(|d| {
                d.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;")
            })
            .collect();
        format!("'self' https://esm.sh {}", safe.join(" "))
    };

    format!(
        "default-src 'none'; script-src 'self' 'unsafe-inline' 'unsafe-eval' https:; style-src 'self' 'unsafe-inline' https:; connect-src 'self' {}; img-src 'self' data: https:; font-src 'self' https:; object-src 'none'; base-uri 'self';",
        connect_src
    )
}

/// Scroll boundary script (reuse same logic as MCP App).
pub fn scroll_boundary_script() -> &'static str {
    r#"<script>(()=>{const s=(e)=>{for(let n=e.target;n;n=n.parentNode){if(!(n instanceof Element))continue;if(n===document.documentElement||n===document.body)continue;const o=window.getComputedStyle(n).overflowY;if(o==='hidden'||o==='visible')continue;if(e.deltaY<0&&n.scrollTop>0)return false;if(e.deltaY>0&&n.scrollTop+n.clientHeight<n.scrollHeight)return false;}return true};window.addEventListener('wheel',e=>{if(!e.defaultPrevented&&s(e))window.parent.postMessage({jsonrpc:'2.0',method:'bitfun/sandbox-wheel',params:{deltaX:e.deltaX,deltaY:e.deltaY,deltaZ:e.deltaZ,deltaMode:e.deltaMode}},'*')},{passive:true});})();</script>"#
}

/// Default dark theme CSS variables for Live App iframe (avoids flash before host sends theme).
pub fn build_live_app_default_theme_css() -> &'static str {
    r#"<style id="bitfun-theme-default">:root{--bitfun-bg:#121214;--bitfun-bg-secondary:#18181a;--bitfun-bg-tertiary:#121214;--bitfun-bg-elevated:#18181a;--bitfun-text:#e8e8e8;--bitfun-text-secondary:#b0b0b0;--bitfun-text-muted:#858585;--bitfun-accent:#60a5fa;--bitfun-accent-hover:#3b82f6;--bitfun-success:#34d399;--bitfun-warning:#f59e0b;--bitfun-error:#ef4444;--bitfun-info:#E1AB80;--bitfun-border:#2e2e32;--bitfun-border-subtle:#27272a;--bitfun-element-bg:#27272a;--bitfun-element-hover:#3f3f46;--bitfun-radius:6px;--bitfun-radius-lg:10px;--bitfun-font-sans:-apple-system,BlinkMacSystemFont,'Segoe UI',Helvetica,Arial,sans-serif;--bitfun-font-mono:ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;--bitfun-scrollbar-thumb:rgba(255,255,255,0.12);}</style>"#
}
