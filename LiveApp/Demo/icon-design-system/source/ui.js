/** @jsx h */
import { h, render, Fragment } from 'preact';
import { useState, useEffect, useRef, useCallback } from 'preact/hooks';

const app = window.app;

// ── Inline SVG icons (no emoji) ─────────────────────────────────────────────

function path(d) {
  return h('path', { d });
}

function ic(size, children, extra = {}) {
  return h('svg', {
    xmlns: 'http://www.w3.org/2000/svg',
    width: size,
    height: size,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    'stroke-width': 1.5,
    'stroke-linecap': 'round',
    'stroke-linejoin': 'round',
    'aria-hidden': 'true',
    ...extra,
  }, children);
}

const Ico = {
  chat: (s = 18) => ic(s, path('M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z')),
  bolt: (s = 18) => ic(s, path('M13 10V3L4 14h7v7l9-11h-7z')),
  library: (s = 18) => ic(s, path('M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z')),
  tokens: (s = 18) => ic(s, [
    path('M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z'),
  ]),
  user: (s = 16) => ic(s, path('M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z')),
  assistant: (s = 16) => ic(s, path('M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z')),
  copy: (s = 14) => ic(s, path('M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2M8 16a2 2 0 002 2h8a2 2 0 002-2v-8a2 2 0 00-2-2h-2M8 16V8a2 2 0 012-2h8')),
  check: (s = 14) => ic(s, path('M4.5 12.75l6 6 9-13.5')),
  refresh: (s = 16) => ic(s, path('M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15')),
  download: (s = 16) => ic(s, path('M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4')),
  loader: (s = 48) => ic(s, [
    path('M12 2v4'),
    path('M12 18v4'),
    path('M4.93 4.93l2.83 2.83'),
    path('M16.24 16.24l2.83 2.83'),
    path('M2 12h4'),
    path('M18 12h4'),
    path('M4.93 19.07l2.83-2.83'),
    path('M16.24 7.76l2.83-2.83'),
  ], { class: 'icon-spin' }),
  empty: (s = 48) => ic(s, path('M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4')),
  close: (s = 10) => ic(s, path('M6 18L18 6M6 6l12 12')),
};

// ── Toast ─────────────────────────────────────────────────────────────────────

function useToasts() {
  const [toasts, setToasts] = useState([]);
  const show = useCallback((msg, type = 'info') => {
    const id = Date.now();
    setToasts(prev => [...prev, { id, msg, type }]);
    setTimeout(() => setToasts(prev => prev.filter(t => t.id !== id)), 3000);
  }, []);
  return { toasts, show };
}

function ToastContainer({ toasts }) {
  return h('div', { class: 'toast-container' },
    toasts.map(t => h('div', { key: t.id, class: `toast ${t.type}` }, t.msg))
  );
}

// ── System Prompt ─────────────────────────────────────────────────────────────

const ICON_DESIGN_SYSTEM_PROMPT = `You are an expert icon designer specializing in SVG icon systems.

When asked to generate icons, output ONLY the SVG code, no explanations.

SVG requirements:
- viewBox="0 0 24 24"
- width="24" height="24"
- Use currentColor for strokes/fills (do NOT hardcode colors)
- stroke-width="1.5" unless specified otherwise
- stroke-linecap="round" stroke-linejoin="round" for line icons
- No <title>, no <desc>, no XML declaration
- Clean, minimal paths

When discussing design style or tokens, respond naturally in the conversation language.
When generating icons, output ONLY the SVG element, starting with <svg`.trim();

// ── Design Tokens Panel ───────────────────────────────────────────────────────

function TokenField(label, value, onChange, min, max, step) {
  return h('div', { class: 'token-field' },
    h('label', null, label),
    h('input', { type: 'range', min, max, step, value, onInput: e => onChange(e.target.value) }),
    h('div', { class: 'token-value' }, value)
  );
}

function TokensPanel({ tokens, onSave, showToast }) {
  const [local, setLocal] = useState(tokens);
  const [saving, setSaving] = useState(false);

  useEffect(() => setLocal(tokens), [tokens]);

  const set = (key, val) => setLocal(prev => ({ ...prev, [key]: val }));

  const handleSave = async () => {
    setSaving(true);
    try {
      await app.call('saveTokens', { tokens: local });
      onSave(local);
      showToast('Design tokens saved', 'success');
    } catch (e) {
      showToast('Failed to save: ' + e.message, 'error');
    } finally {
      setSaving(false);
    }
  };

  return h(Fragment, null,
    h('div', { class: 'panel-header' },
      h('div', { class: 'panel-title' }, 'Design Tokens'),
      h('button', { class: 'btn btn-primary btn-sm', onClick: handleSave, disabled: saving },
        saving ? 'Saving…' : 'Save'
      )
    ),
    h('div', { class: 'token-grid' },
      TokenField('Stroke Width', local.strokeWidth, v => set('strokeWidth', Number(v)), 0.5, 4, 0.25),
      TokenField('Corner Radius', local.cornerRadius, v => set('cornerRadius', Number(v)), 0, 12, 0.5),
      TokenField('Grid Size', local.gridSize, v => set('gridSize', Number(v)), 16, 48, 8),
      TokenField('Optical Padding', local.opticalPadding, v => set('opticalPadding', Number(v)), 0, 4, 0.5),
    ),
    h('div', { class: 'token-section' },
      h('div', { class: 'detail-section-title' }, 'Size Variants (px)'),
      h('div', { class: 'tags' },
        (local.sizeVariants || [16, 20, 24, 32, 48]).map(s =>
          h('span', { key: s, class: 'tag' }, `${s}px`)
        )
      )
    ),
    h('div', { class: 'token-section' },
      h('div', { class: 'detail-section-title' }, 'Style Variants'),
      h('div', { class: 'tags' },
        (local.styleVariants || ['outlined', 'filled']).map(s =>
          h('span', { key: s, class: 'tag' }, s)
        )
      )
    )
  );
}

// ── AI Style Chat Panel ───────────────────────────────────────────────────────

function ChatPanel({ tokens, showToast }) {
  const [messages, setMessages] = useState([
    { role: 'assistant', content: 'Hi! I can help you define the icon design style and generate icons. Describe the style you want — e.g., "sharp geometric, 2px stroke, minimal" or ask me to generate a specific icon.' }
  ]);
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [streamText, setStreamText] = useState('');
  const cancelRef = useRef(null);
  const bottomRef = useRef(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamText]);

  const sendMessage = async () => {
    const text = input.trim();
    if (!text || busy) return;
    setInput('');

    const userMsg = { role: 'user', content: text };
    const history = [...messages, userMsg];
    setMessages(history);
    setBusy(true);
    setStreamText('');

    const contextHint = `\n\n[Current design tokens: strokeWidth=${tokens.strokeWidth}, cornerRadius=${tokens.cornerRadius}, gridSize=${tokens.gridSize}]`;

    try {
      let accumulated = '';
      const handle = await app.ai.chat(
        history.map(m => ({ role: m.role, content: m.content })),
        {
          systemPrompt: ICON_DESIGN_SYSTEM_PROMPT + contextHint,
          model: 'primary',
          onChunk: ({ text: t }) => {
            if (t) { accumulated += t; setStreamText(accumulated); }
          },
          onDone: ({ fullText }) => {
            const final = fullText || accumulated;
            setMessages(prev => [...prev, { role: 'assistant', content: final }]);
            setStreamText('');
            setBusy(false);
            cancelRef.current = null;
            if (/<svg/i.test(final)) showToast('SVG detected — save it from the Generate tab!', 'info');
          },
          onError: ({ message }) => {
            showToast('AI error: ' + message, 'error');
            setStreamText('');
            setBusy(false);
            cancelRef.current = null;
          },
        }
      );
      cancelRef.current = handle.cancel;
    } catch (e) {
      showToast('Failed to send: ' + e.message, 'error');
      setBusy(false);
    }
  };

  const handleKeyDown = (e) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendMessage(); }
  };

  const cancel = () => {
    if (cancelRef.current) { cancelRef.current(); cancelRef.current = null; }
    setStreamText('');
    setBusy(false);
  };

  return h('div', { class: 'chat-layout' },
    h('div', { class: 'messages' },
      messages.map((m, i) => h(ChatMessage, { key: i, message: m, showToast })),
      streamText && h('div', { class: 'message assistant' },
        h('div', { class: 'message-avatar' }, Ico.assistant()),
        h('div', { class: 'message-bubble' },
          streamText,
          h('div', { class: 'typing-indicator' },
            h('div', { class: 'dot' }), h('div', { class: 'dot' }), h('div', { class: 'dot' })
          )
        )
      ),
      h('div', { ref: bottomRef })
    ),
    h('div', { class: 'chat-input-area' },
      h('textarea', {
        class: 'chat-input',
        placeholder: 'Describe a style or ask to generate an icon…',
        value: input,
        onInput: e => setInput(e.target.value),
        onKeyDown: handleKeyDown,
        disabled: busy,
        rows: 1,
      }),
      busy
        ? h('button', { class: 'btn btn-secondary btn-sm', onClick: cancel }, 'Stop')
        : h('button', { class: 'btn btn-primary btn-sm', onClick: sendMessage, disabled: !input.trim() }, 'Send')
    )
  );
}

function ChatMessage({ message, showToast }) {
  const isUser = message.role === 'user';
  const svgMatch = !isUser && message.content.match(/<svg[\s\S]*?<\/svg>/i);
  const [copied, setCopied] = useState(false);

  const copySvg = async () => {
    if (!svgMatch) return;
    try {
      await app.clipboard.writeText(svgMatch[0]);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // fallback: show in a prompt so the user can copy manually
      window.prompt('Copy the SVG code below:', svgMatch[0]);
    }
  };

  return h('div', { class: `message ${message.role}` },
    h('div', { class: 'message-avatar' }, isUser ? Ico.user() : Ico.assistant()),
    h('div', { class: 'message-bubble' },
      svgMatch
        ? h(Fragment, null,
            h('span', null, message.content.replace(svgMatch[0], '')),
            h('div', { class: 'svg-preview-box msg-svg-preview' },
              h('div', { dangerouslySetInnerHTML: { __html: svgMatch[0] } })
            ),
            h('div', { class: 'svg-actions' },
              h('button', { class: 'btn btn-secondary btn-sm', onClick: copySvg },
                copied
                  ? h(Fragment, null, Ico.check(14), ' Copied!')
                  : h(Fragment, null, Ico.copy(14), ' Copy SVG')
              )
            )
          )
        : message.content
    )
  );
}

// ── Generate Panel ────────────────────────────────────────────────────────────

function GeneratePanel({ tokens, onIconAdded, showToast }) {
  const [prompt, setPrompt] = useState('');
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [generated, setGenerated] = useState('');

  const generate = async () => {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    setGenerated('');

    try {
      const tokenCtx = `Stroke width: ${tokens.strokeWidth}px, corner radius: ${tokens.cornerRadius}, grid: ${tokens.gridSize}x${tokens.gridSize}`;
      const result = await app.ai.complete(
        `Generate a ${tokens.gridSize}x${tokens.gridSize} SVG icon for: ${prompt}\n\nDesign constraints: ${tokenCtx}`,
        { systemPrompt: ICON_DESIGN_SYSTEM_PROMPT, model: 'fast', maxTokens: 2048 }
      );
      const svgMatch = result.text.match(/<svg[\s\S]*?<\/svg>/i);
      if (!svgMatch) showToast('No SVG found in AI response. Try rephrasing.', 'error');
      else setGenerated(svgMatch[0]);
    } catch (e) {
      showToast('Generation failed: ' + e.message, 'error');
    } finally {
      setBusy(false);
    }
  };

  const saveToLibrary = async () => {
    if (!generated) return;
    try {
      await app.call('saveIcon', {
        name: name.trim() || prompt.trim().slice(0, 32),
        tags: [],
        category: 'general',
        svgSource: generated,
      });
      onIconAdded();
      showToast('Icon saved to library!', 'success');
      setGenerated(''); setName(''); setPrompt('');
    } catch (e) {
      showToast('Save failed: ' + e.message, 'error');
    }
  };

  return h(Fragment, null,
    h('div', { class: 'panel-header' },
      h('div', { class: 'panel-title' }, 'Generate Icon')
    ),
    h('div', { class: 'generate-form' },
      h('div', { class: 'token-field' },
        h('label', null, 'Icon description'),
        h('input', {
          type: 'text',
          placeholder: 'e.g. "settings gear with minimalist style"',
          value: prompt,
          onInput: e => setPrompt(e.target.value),
          onKeyDown: e => e.key === 'Enter' && generate(),
          disabled: busy,
        })
      ),
      h('div', { class: 'token-field' },
        h('label', null, 'Name (optional)'),
        h('input', {
          type: 'text',
          placeholder: 'e.g. "Settings"',
          value: name,
          onInput: e => setName(e.target.value),
          disabled: busy,
        })
      ),
      h('button', { class: 'btn btn-primary', onClick: generate, disabled: busy || !prompt.trim() },
        busy ? 'Generating…' : 'Generate'
      ),

      generated && h(Fragment, null,
        h('div', { class: 'detail-section-title' }, 'Preview'),
        h('div', { class: 'svg-preview-box' },
          h('div', { dangerouslySetInnerHTML: { __html: generated }, class: 'svg-icon-wrap' })
        ),
        h('div', { class: 'btn-row' },
          h('button', { class: 'btn btn-primary', onClick: saveToLibrary }, 'Save to Library'),
          h('button', { class: 'btn btn-secondary', onClick: () => setGenerated('') }, 'Discard'),
        ),
        h('div', { class: 'detail-section-title' }, 'SVG Source'),
        h('pre', { class: 'svg-code' }, generated)
      )
    )
  );
}

// ── Library Panel ─────────────────────────────────────────────────────────────

function LibraryPanel({ refreshKey, showToast }) {
  const [icons, setIcons] = useState([]);
  const [selected, setSelected] = useState(null);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(true);
  const [iconData, setIconData] = useState({});

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const list = await app.call('listIcons', {});
      setIcons(list || []);
    } catch (e) {
      showToast('Failed to load library: ' + e.message, 'error');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [refreshKey]);

  const selectIcon = async (id) => {
    setSelected(id);
    if (!iconData[id]) {
      try {
        const data = await app.call('getIcon', { id });
        setIconData(prev => ({ ...prev, [id]: data }));
      } catch { /* ignore */ }
    }
  };

  const deleteIcon = async (e, id) => {
    e.stopPropagation();
    try {
      await app.call('deleteIcon', { id });
      if (selected === id) setSelected(null);
      load();
      showToast('Icon deleted', 'info');
    } catch (err) {
      showToast('Delete failed: ' + err.message, 'error');
    }
  };

  const copyToClipboard = async (text) => {
    try {
      await app.clipboard.writeText(text);
      showToast('Copied!', 'success');
    } catch {
      showToast('Copy failed', 'error');
    }
  };

  const exportAll = async () => {
    try {
      const dir = await app.dialog.open({ directory: true, title: 'Export icons to folder' });
      if (!dir) return;
      const result = await app.call('exportIcons', { targetDir: dir, format: 'svg' });
      showToast(`Exported ${result.exported} icons`, 'success');
    } catch (e) {
      showToast('Export failed: ' + e.message, 'error');
    }
  };

  const filtered = icons.filter(i =>
    !search || i.name.toLowerCase().includes(search.toLowerCase())
  );
  const selectedData = selected ? iconData[selected] : null;

  return h('div', { class: 'library-layout' },
    h('div', { class: 'panel library-main' },
      h('div', { class: 'panel-header' },
        h('div', { class: 'panel-title' }, `Library (${icons.length})`),
        h('div', { class: 'btn-row' },
          h('button', { class: 'btn btn-secondary btn-sm btn-icon', onClick: load, 'aria-label': 'Refresh library' }, Ico.refresh(16)),
          h('button', { class: 'btn btn-secondary btn-sm', onClick: exportAll },
            Ico.download(16), ' Export')
        )
      ),
      h('div', { class: 'search-row' },
        h('input', {
          type: 'text',
          class: 'search-input',
          placeholder: 'Search icons…',
          value: search,
          onInput: e => setSearch(e.target.value),
        })
      ),
      loading
        ? h('div', { class: 'empty-state' }, h('div', { class: 'big-icon' }, Ico.loader(48)), 'Loading…')
        : filtered.length === 0
          ? h('div', { class: 'empty-state' },
              h('div', { class: 'big-icon' }, Ico.empty(48)),
              h('p', null, search ? 'No icons match your search' : 'No icons yet'),
              !search && h('p', { class: 'hint-text' }, 'Use the Generate tab to create your first icon')
            )
          : h('div', { class: 'icon-grid' },
              filtered.map(icon => h('div', {
                key: icon.id,
                class: `icon-card ${selected === icon.id ? 'selected' : ''}`,
                onClick: () => selectIcon(icon.id),
              },
                h('button', { class: 'delete-btn', onClick: e => deleteIcon(e, icon.id), 'aria-label': 'Delete icon' }, Ico.close(10)),
                h('div', {
                  class: 'preview',
                  dangerouslySetInnerHTML: {
                    __html: iconData[icon.id]?.svgSource ||
                      '<svg viewBox="0 0 24 24" width="24" height="24"><rect width="24" height="24" fill="none"/></svg>'
                  }
                }),
                h('div', { class: 'name' }, icon.name)
              ))
            )
    ),
    selectedData && h('div', { class: 'detail-panel' },
      h('div', { class: 'detail-panel-content' },
        h('div', { class: 'detail-section' },
          h('div', { class: 'detail-section-title' }, 'Preview'),
          h('div', { class: 'svg-preview-box' },
            h('div', { dangerouslySetInnerHTML: { __html: selectedData.svgSource }, class: 'svg-icon-wrap' })
          )
        ),
        h('div', { class: 'detail-section' },
          h('div', { class: 'detail-section-title' }, 'Info'),
          h('div', { class: 'icon-name-text' }, selectedData.name),
          selectedData.category && h('div', { class: 'tags mt6' },
            h('span', { class: 'tag' }, selectedData.category)
          )
        ),
        h('div', { class: 'detail-section' },
          h('div', { class: 'detail-section-title' }, 'SVG Source'),
          h('pre', { class: 'svg-code' }, selectedData.svgSource),
          h('button', {
            class: 'btn btn-secondary btn-sm btn-full',
            onClick: () => copyToClipboard(selectedData.svgSource)
          }, Ico.copy(14), ' Copy SVG')
        )
      )
    )
  );
}

// ── App Root ──────────────────────────────────────────────────────────────────

function App() {
  const [tab, setTab] = useState('chat');
  const [tokens, setTokens] = useState({
    strokeWidth: 1.5,
    cornerRadius: 2,
    gridSize: 24,
    opticalPadding: 1,
    sizeVariants: [16, 20, 24, 32, 48],
    styleVariants: ['outlined', 'filled'],
  });
  const [libraryKey, setLibraryKey] = useState(0);
  const { toasts, show: showToast } = useToasts();

  useEffect(() => {
    app.call('getTokens', {}).then(t => {
      if (t && t.strokeWidth) setTokens(t);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    app.on('worker:progress', (data) => {
      if (data && data.total > 0) {
        showToast(`Exporting ${data.name}… (${data.done}/${data.total})`, 'info');
      }
    });
  }, []);

  const nav = (id, iconEl, label) => h('div', {
    class: `nav-item ${tab === id ? 'active' : ''}`,
    onClick: () => setTab(id),
  }, h('span', { class: 'icon' }, iconEl), label);

  return h(Fragment, null,
    h('div', { id: 'app' },
      h('div', { class: 'sidebar' },
        h('div', { class: 'nav-section' }, 'Icon Design'),
        nav('chat', Ico.chat(), 'Style Chat'),
        nav('generate', Ico.bolt(), 'Generate'),
        nav('library', Ico.library(), 'Library'),
        h('div', { class: 'nav-section nav-section--bottom' }, 'Settings'),
        nav('tokens', Ico.tokens(), 'Tokens'),
      ),
      h('div', { class: 'main' },
        h('div', { class: `panel chat-panel-wrap ${tab === 'chat' ? 'panel--visible' : 'panel--hidden'}` },
          h(ChatPanel, { tokens, showToast })
        ),
        tab === 'generate' && h('div', { class: 'panel' },
          h(GeneratePanel, { tokens, onIconAdded: () => setLibraryKey(k => k + 1), showToast })
        ),
        tab === 'library' && h('div', { class: 'library-wrapper' },
          h(LibraryPanel, { refreshKey: libraryKey, showToast })
        ),
        tab === 'tokens' && h('div', { class: 'panel' },
          h(TokensPanel, { tokens, onSave: setTokens, showToast })
        ),
      )
    ),
    h(ToastContainer, { toasts })
  );
}

render(h(App, null), document.getElementById('root'));
