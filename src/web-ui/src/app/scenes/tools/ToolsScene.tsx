/**
 * ToolsScene — unified tool kit browser.
 *
 * Built-in tools and MCP tools live in the same list, grouped under a single
 * category tree. MCP servers are administered through a dedicated modal that
 * is launched from the sidebar (add / start / stop / restart / delete / edit
 * JSON config).
 *
 * Agent / Subagent composition lives elsewhere (Agents scene).
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Plus,
  FileJson,
  Server,
  AlertTriangle,
  Play,
  Square,
  RotateCw,
  Trash2,
  Plug,
  Wrench,
  ChevronRight,
  Settings2,
} from 'lucide-react';
import { Badge, Button, ConfirmDialog, Empty, Modal, Search } from '@/component-library';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import MCPAPI, { type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import {
  BUILTIN_TOOLS,
  CATEGORY_ORDER,
  countByCategory,
  type BuiltinToolMeta,
  type ToolCategory,
  type ToolPermission,
} from './data/builtinTools';
import './ToolsScene.scss';

const log = createLogger('ToolsScene');

type DetailLang = 'zh' | 'en';

const MCP_TOOL_PREFIX = 'mcp__';
const MCP_LOCAL_PROCESS_EXAMPLE = `{
  "mcpServers": {
    "zai-mcp-server": {
      "command": "npx",
      "args": ["-y", "@z_ai/mcp-server"],
      "env": { "Z_AI_API_KEY": "your_api_key" }
    }
  }
}`;
const MCP_REMOTE_SERVICE_EXAMPLE = `{
  "mcpServers": {
    "remote-mcp": {
      "url": "http://localhost:3000/sse"
    }
  }
}`;

interface RegisteredTool {
  name: string;
  description?: string;
}

interface McpToolEntry extends RegisteredTool {
  serverId: string;
  shortName: string;
}

/**
 * Unified tool entry — either a built-in tool or an MCP tool.
 * The browsing/detail UI does not need to care about the difference
 * beyond rendering: both share the same row + detail template.
 */
type UnifiedTool =
  | { kind: 'builtin'; meta: BuiltinToolMeta }
  | { kind: 'mcp'; mcp: McpToolEntry };

/** Sidebar selection model. */
type Selection =
  | { kind: 'all' }
  | { kind: 'builtin-category'; category: ToolCategory }
  | { kind: 'mcp-all' }
  | { kind: 'mcp-server'; serverId: string };

const SEL_ALL: Selection = { kind: 'all' };

// ─────────────────────────────────────────────────────────────────────────────

const ToolsScene: React.FC = () => {
  const { t } = useTranslation('scenes/tools');
  const [selection, setSelection] = useState<Selection>(SEL_ALL);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<UnifiedTool | null>(null);
  const [managerOpen, setManagerOpen] = useState(false);

  // ── Data: MCP servers and registered tools ───────────────────────────────
  const [servers, setServers] = useState<MCPServerInfo[]>([]);
  const [registeredTools, setRegisteredTools] = useState<RegisteredTool[]>([]);

  const loadMcp = useCallback(async () => {
    try {
      const [serverList, toolList] = await Promise.all([
        MCPAPI.getServers(),
        toolAPI.getAllToolsInfo().catch(() => [] as RegisteredTool[]),
      ]);
      setServers(serverList);
      setRegisteredTools((toolList as RegisteredTool[]) ?? []);
    } catch (error) {
      log.error('Failed to load MCP data', error);
    }
  }, []);

  useEffect(() => { void loadMcp(); }, [loadMcp]);

  // ── Derived: MCP tool entries grouped by server ──────────────────────────
  const mcpToolsByServer = useMemo(() => {
    const map = new Map<string, McpToolEntry[]>();
    for (const tool of registeredTools) {
      if (!tool.name.startsWith(MCP_TOOL_PREFIX)) continue;
      const rest = tool.name.slice(MCP_TOOL_PREFIX.length);
      const sepIdx = rest.indexOf('__');
      if (sepIdx < 0) continue;
      const serverId = rest.slice(0, sepIdx);
      const shortName = rest.slice(sepIdx + 2);
      const entry: McpToolEntry = { ...tool, serverId, shortName };
      const arr = map.get(serverId);
      if (arr) arr.push(entry); else map.set(serverId, [entry]);
    }
    return map;
  }, [registeredTools]);

  const totalMcpTools = useMemo(
    () => Array.from(mcpToolsByServer.values()).reduce((n, arr) => n + arr.length, 0),
    [mcpToolsByServer],
  );

  // ── Filtered tool list for the center pane ───────────────────────────────
  const visibleTools: UnifiedTool[] = useMemo(() => {
    const q = query.trim().toLowerCase();
    const items: UnifiedTool[] = [];

    const pushBuiltin = (pred: (m: BuiltinToolMeta) => boolean) => {
      for (const m of BUILTIN_TOOLS) {
        if (!pred(m)) continue;
        if (q && !m.name.toLowerCase().includes(q)) continue;
        items.push({ kind: 'builtin', meta: m });
      }
    };
    const pushMcp = (pred: (e: McpToolEntry) => boolean) => {
      mcpToolsByServer.forEach((arr) => {
        for (const e of arr) {
          if (!pred(e)) continue;
          const hay = `${e.shortName} ${e.name}`.toLowerCase();
          if (q && !hay.includes(q)) continue;
          items.push({ kind: 'mcp', mcp: e });
        }
      });
    };

    switch (selection.kind) {
      case 'all':
        pushBuiltin(() => true);
        pushMcp(() => true);
        break;
      case 'builtin-category':
        pushBuiltin((m) => m.category === selection.category);
        break;
      case 'mcp-all':
        pushMcp(() => true);
        break;
      case 'mcp-server':
        pushMcp((e) => e.serverId === selection.serverId);
        break;
    }
    return items;
  }, [selection, query, mcpToolsByServer]);

  // Keep the selected detail in sync when the underlying list changes.
  useEffect(() => {
    if (!selected) return;
    if (selected.kind === 'mcp') {
      const stillThere = mcpToolsByServer.get(selected.mcp.serverId)?.some(e => e.name === selected.mcp.name);
      if (!stillThere) setSelected(null);
    }
  }, [mcpToolsByServer, selected]);

  const counts = useMemo(() => countByCategory(), []);

  return (
    <div className="bitfun-tools-scene">
      <header className="bitfun-tools-scene__header">
        <div className="bitfun-tools-scene__identity">
          <h1 className="bitfun-tools-scene__title">{t('page.title')}</h1>
          <div className="bitfun-tools-scene__subline">
            <p className="bitfun-tools-scene__subtitle">{t('page.subtitle')}</p>
            <div className="bitfun-tools-scene__actions">
              <Search
                className="bitfun-tools-scene__search"
                value={query}
                onChange={setQuery}
                onSearch={setQuery}
                onClear={() => setQuery('')}
                placeholder={t('search.placeholder')}
                size="small"
                clearable
              />
            </div>
          </div>
        </div>
      </header>

      <div className="bitfun-tools-scene__body">
        <div className="tools-split">
          {/* ── Left: category tree ───────────────────────────────── */}
          <aside className="tools-split__sidebar">
            <button
              type="button"
              className="tools-sidebar__manage"
              onClick={() => setManagerOpen(true)}
            >
              <Settings2 size={13} />
              <span>{t('sidebar.manageServers')}</span>
              {servers.length > 0 && (
                <span className="tools-sidebar__manage-count">{servers.length}</span>
              )}
            </button>

            <CategoryTree
              selection={selection}
              onSelect={setSelection}
              counts={counts}
              totalBuiltin={BUILTIN_TOOLS.length}
              totalMcp={totalMcpTools}
              servers={servers}
              mcpToolsByServer={mcpToolsByServer}
            />
          </aside>

          {/* ── Middle: list ──────────────────────────────────────── */}
          <section className="tools-split__list">
            <div className="tools-split__toolbar">
              <span className="tools-split__count">
                {t('list.countSuffix', { count: visibleTools.length })}
              </span>
            </div>

            <div className="tools-split__rows">
              {visibleTools.length === 0 ? (
                <Empty description={t('list.emptyAll')} />
              ) : (
                visibleTools.map(tool => (
                  <ToolRow
                    key={tool.kind === 'builtin' ? `b:${tool.meta.name}` : `m:${tool.mcp.name}`}
                    tool={tool}
                    active={isSameTool(tool, selected)}
                    onClick={() => setSelected(tool)}
                  />
                ))
              )}
            </div>
          </section>

          {/* ── Right: detail ─────────────────────────────────────── */}
          <section className="tools-split__detail">
            {selected ? (
              selected.kind === 'builtin'
                ? <BuiltinToolDetail tool={selected.meta} />
                : <McpToolDetail
                    tool={selected.mcp}
                    server={servers.find(s => s.id === selected.mcp.serverId) ?? null}
                  />
            ) : (
              <div className="tools-split__detail-empty">
                <Wrench size={32} strokeWidth={1.4} />
                <span>{t('detail.selectHint')}</span>
              </div>
            )}
          </section>
        </div>
      </div>

      <McpManagerModal
        open={managerOpen}
        onClose={() => setManagerOpen(false)}
        servers={servers}
        onRefresh={loadMcp}
      />
    </div>
  );
};

function isSameTool(a: UnifiedTool, b: UnifiedTool | null): boolean {
  if (!b || a.kind !== b.kind) return false;
  if (a.kind === 'builtin' && b.kind === 'builtin') return a.meta.name === b.meta.name;
  if (a.kind === 'mcp' && b.kind === 'mcp') return a.mcp.name === b.mcp.name;
  return false;
}

// ─────────────────────────────────────────────────────────────────────────────
// Category tree
// ─────────────────────────────────────────────────────────────────────────────

const CategoryTree: React.FC<{
  selection: Selection;
  onSelect: (s: Selection) => void;
  counts: Record<ToolCategory, number>;
  totalBuiltin: number;
  totalMcp: number;
  servers: MCPServerInfo[];
  mcpToolsByServer: Map<string, McpToolEntry[]>;
}> = ({ selection, onSelect, counts, totalBuiltin, totalMcp, servers, mcpToolsByServer }) => {
  const { t } = useTranslation('scenes/tools');

  const isActive = (pred: (s: Selection) => boolean): string =>
    pred(selection) ? ' is-active' : '';

  return (
    <div className="tools-tree">
      <button
        type="button"
        className={`tools-tree__item tools-tree__item--root${isActive(s => s.kind === 'all')}`}
        onClick={() => onSelect({ kind: 'all' })}
      >
        <span>{t('categories.all')}</span>
        <span className="tools-tree__count">{totalBuiltin + totalMcp}</span>
      </button>

      <div className="tools-tree__group-label">{t('sidebar.builtin')}</div>
      {CATEGORY_ORDER.map(c => (
        <button
          key={c}
          type="button"
          className={`tools-tree__item${isActive(s => s.kind === 'builtin-category' && s.category === c)}`}
          onClick={() => onSelect({ kind: 'builtin-category', category: c })}
        >
          <span>{t(`categories.${c}`)}</span>
          <span className="tools-tree__count">{counts[c]}</span>
        </button>
      ))}

      <div className="tools-tree__group-label">{t('sidebar.mcp')}</div>
      <button
        type="button"
        className={`tools-tree__item${isActive(s => s.kind === 'mcp-all')}`}
        onClick={() => onSelect({ kind: 'mcp-all' })}
      >
        <span>{t('sidebar.mcp')}</span>
        <span className="tools-tree__count">{totalMcp}</span>
      </button>

      {servers.length === 0 ? (
        <div className="tools-tree__empty">{t('sidebar.noServers')}</div>
      ) : (
        servers.map(s => {
          const n = mcpToolsByServer.get(s.id)?.length ?? 0;
          return (
            <button
              key={s.id}
              type="button"
              className={`tools-tree__item tools-tree__item--sub${isActive(sel => sel.kind === 'mcp-server' && sel.serverId === s.id)}`}
              onClick={() => onSelect({ kind: 'mcp-server', serverId: s.id })}
            >
              <StatusDot status={s.status} />
              <span className="tools-tree__sub-name">{s.name || s.id}</span>
              <span className="tools-tree__count">{n}</span>
            </button>
          );
        })
      )}
    </div>
  );
};

// ─────────────────────────────────────────────────────────────────────────────
// Row + badges
// ─────────────────────────────────────────────────────────────────────────────

const PermissionBadge: React.FC<{ level: ToolPermission }> = ({ level }) => {
  const { t } = useTranslation('scenes/tools');
  const variant: 'success' | 'warning' | 'error' =
    level === 'read' ? 'success' : level === 'write' ? 'warning' : 'error';
  return <Badge variant={variant}>{t(`permissions.${level}`)}</Badge>;
};

const ToolRow: React.FC<{
  tool: UnifiedTool;
  active: boolean;
  onClick: () => void;
}> = ({ tool, active, onClick }) => {
  const { t } = useTranslation('scenes/tools');

  if (tool.kind === 'builtin') {
    const Icon = tool.meta.Icon;
    return (
      <button type="button" className={`tools-row${active ? ' is-active' : ''}`} onClick={onClick}>
        <span className="tools-row__icon"><Icon size={15} strokeWidth={1.6} /></span>
        <span className="tools-row__body">
          <span className="tools-row__name">{tool.meta.name}</span>
          <span className="tools-row__desc">{t(`builtin.${tool.meta.name}.summary`)}</span>
        </span>
        <span className="tools-row__tail">
          <PermissionBadge level={tool.meta.permission} />
          <ChevronRight size={13} className="tools-row__chev" aria-hidden="true" />
        </span>
      </button>
    );
  }

  const mcp = tool.mcp;
  return (
    <button type="button" className={`tools-row${active ? ' is-active' : ''}`} onClick={onClick}>
      <span className="tools-row__icon tools-row__icon--mcp"><Plug size={14} strokeWidth={1.6} /></span>
      <span className="tools-row__body">
        <span className="tools-row__name">{mcp.shortName}</span>
        <span className="tools-row__desc">{mcp.description || mcp.serverId}</span>
      </span>
      <span className="tools-row__tail">
        <Badge variant="purple">{mcp.serverId}</Badge>
        <ChevronRight size={13} className="tools-row__chev" aria-hidden="true" />
      </span>
    </button>
  );
};

// ─────────────────────────────────────────────────────────────────────────────
// Detail views
// ─────────────────────────────────────────────────────────────────────────────

const BuiltinToolDetail: React.FC<{ tool: BuiltinToolMeta }> = ({ tool }) => {
  const { i18n, t } = useTranslation('scenes/tools');
  const defaultLang: DetailLang = i18n.language?.startsWith('zh') ? 'zh' : 'en';
  const [lang, setLang] = useState<DetailLang>(defaultLang);

  useEffect(() => {
    setLang(i18n.language?.startsWith('zh') ? 'zh' : 'en');
  }, [tool.name, i18n.language]);

  const Icon = tool.Icon;

  const localized = useCallback(
    (key: string, options?: Record<string, unknown>): string =>
      t(key, { ...(options ?? {}), lng: lang === 'zh' ? 'zh-CN' : 'en-US' }) as string,
    [t, lang],
  );

  const getList = useCallback((key: string): string[] => {
    const raw = i18n.getResource(lang === 'zh' ? 'zh-CN' : 'en-US', 'scenes/tools', key);
    return Array.isArray(raw) ? (raw as string[]) : [];
  }, [i18n, lang]);

  const getRelated = useCallback((): Array<{ name: string; note: string }> => {
    const raw = i18n.getResource(
      lang === 'zh' ? 'zh-CN' : 'en-US', 'scenes/tools',
      `builtin.${tool.name}.related`,
    );
    return Array.isArray(raw) ? (raw as Array<{ name: string; note: string }>) : [];
  }, [i18n, lang, tool.name]);

  const whenList = getList(`builtin.${tool.name}.when`);
  const whenNotList = getList(`builtin.${tool.name}.whenNot`);
  const relatedList = getRelated();
  const notes = localized(`builtin.${tool.name}.notes`);
  const hasNotes = notes && !notes.endsWith(`.notes`);

  return (
    <div className="tools-detail">
      <header className="tools-detail__head">
        <span className="tools-detail__icon"><Icon size={20} strokeWidth={1.5} /></span>
        <div className="tools-detail__identity">
          <div className="tools-detail__title-row">
            <h2 className="tools-detail__title">{tool.name}</h2>
            <Badge variant="info">{t('detail.sourceBuiltin')}</Badge>
            <PermissionBadge level={tool.permission} />
          </div>
          <p className="tools-detail__summary">{localized(`builtin.${tool.name}.summary`)}</p>
        </div>
        <LangToggle lang={lang} onChange={setLang} />
      </header>

      <div className="tools-detail__body">
        <Section title={t('detail.sections.what')}>
          <p>{localized(`builtin.${tool.name}.what`)}</p>
        </Section>

        {whenList.length > 0 && (
          <Section title={t('detail.sections.when')}>
            <ul className="tools-detail__bullets">
              {whenList.map((item, i) => <li key={i}>{item}</li>)}
            </ul>
          </Section>
        )}

        {whenNotList.length > 0 && (
          <Section title={t('detail.sections.whenNot')}>
            <ul className="tools-detail__bullets tools-detail__bullets--warn">
              {whenNotList.map((item, i) => <li key={i}>{item}</li>)}
            </ul>
          </Section>
        )}

        <Section title={t('detail.sections.related')}>
          {relatedList.length === 0 ? (
            <p className="tools-detail__muted">{t('detail.relatedNone')}</p>
          ) : (
            <ul className="tools-detail__related">
              {relatedList.map(r => (
                <li key={r.name}><code>{r.name}</code><span>— {r.note}</span></li>
              ))}
            </ul>
          )}
        </Section>

        {hasNotes && (
          <Section title={t('detail.sections.notes')}>
            <p className="tools-detail__notes">
              <AlertTriangle size={13} strokeWidth={1.6} />
              <span>{notes}</span>
            </p>
          </Section>
        )}
      </div>
    </div>
  );
};

const McpToolDetail: React.FC<{
  tool: McpToolEntry;
  server: MCPServerInfo | null;
}> = ({ tool, server }) => {
  const { t } = useTranslation('scenes/tools');

  return (
    <div className="tools-detail">
      <header className="tools-detail__head">
        <span className="tools-detail__icon"><Plug size={20} strokeWidth={1.5} /></span>
        <div className="tools-detail__identity">
          <div className="tools-detail__title-row">
            <h2 className="tools-detail__title">{tool.shortName}</h2>
            <Badge variant="purple">{t('detail.sourceMcp', { server: tool.serverId })}</Badge>
            {server && <Badge variant="neutral">{server.status}</Badge>}
          </div>
          <p className="tools-detail__summary">
            <code>{tool.name}</code>
          </p>
        </div>
      </header>

      <div className="tools-detail__body">
        <Section title={t('detail.sections.what')}>
          {tool.description
            ? <p>{tool.description}</p>
            : <p className="tools-detail__muted">{t('detail.outputsNone')}</p>}
        </Section>

        {server && (
          <Section title={t('mcp.server.configSection')}>
            <dl className="tools-mcp__kv">
              <dt>{t('mcp.server.transport')}</dt>
              <dd>{server.transport}</dd>
              {server.url && (<><dt>{t('mcp.server.url')}</dt><dd><code>{server.url}</code></dd></>)}
              {server.command && (<><dt>{t('mcp.server.command')}</dt><dd><code>{server.command}</code></dd></>)}
            </dl>
          </Section>
        )}
      </div>
    </div>
  );
};

const LangToggle: React.FC<{
  lang: DetailLang;
  onChange: (l: DetailLang) => void;
}> = ({ lang, onChange }) => {
  const { t } = useTranslation('scenes/tools');
  return (
    <div className="tools-detail__lang">
      <button
        type="button"
        className={`tools-detail__lang-btn${lang === 'en' ? ' is-active' : ''}`}
        onClick={() => onChange('en')}
      >{t('detail.langToggleEn')}</button>
      <button
        type="button"
        className={`tools-detail__lang-btn${lang === 'zh' ? ' is-active' : ''}`}
        onClick={() => onChange('zh')}
      >{t('detail.langToggleZh')}</button>
    </div>
  );
};

const Section: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <section className="tools-detail__section">
    <h3 className="tools-detail__section-title">{title}</h3>
    <div className="tools-detail__section-body">{children}</div>
  </section>
);

// ─────────────────────────────────────────────────────────────────────────────
// MCP status dot (used in both sidebar tree and manager modal)
// ─────────────────────────────────────────────────────────────────────────────

const StatusDot: React.FC<{ status: string }> = ({ status }) => {
  const tone: 'ok' | 'warn' | 'err' | 'idle' =
    /Connected|Healthy/.test(status) ? 'ok'
      : /Starting|Reconnecting|Stopping|NeedsAuth/.test(status) ? 'warn'
      : /Fail|Error/.test(status) ? 'err'
      : 'idle';
  return <span className={`tools-mcp__dot tools-mcp__dot--${tone}`} aria-hidden="true" />;
};

// ─────────────────────────────────────────────────────────────────────────────
// MCP Manager Modal — add / start / stop / restart / delete / edit JSON
// ─────────────────────────────────────────────────────────────────────────────

const McpManagerModal: React.FC<{
  open: boolean;
  onClose: () => void;
  servers: MCPServerInfo[];
  onRefresh: () => Promise<void>;
}> = ({ open, onClose, servers, onRefresh }) => {
  const { t } = useTranslation('scenes/tools');
  const notification = useNotification();

  const [editorOpen, setEditorOpen] = useState(false);
  const [editorInitial, setEditorInitial] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<MCPServerInfo | null>(null);

  const errorCount = useMemo(
    () => servers.filter(s => /Fail|Error|NeedsAuth/i.test(s.status)).length,
    [servers],
  );

  const handleOpenEditor = useCallback(async () => {
    try {
      const config = await MCPAPI.loadMCPJsonConfig();
      setEditorInitial(config ?? '');
      setEditorOpen(true);
    } catch (error) {
      notification.error(t('mcp.editor.loadFailed', { error: String(error) }));
    }
  }, [notification, t]);

  const handleSaveEditor = useCallback(async (raw: string) => {
    try {
      await MCPAPI.saveMCPJsonConfig(raw);
      notification.success(t('mcp.editor.saveSuccess'));
      setEditorOpen(false);
      await onRefresh();
    } catch (error) {
      notification.error(t('mcp.editor.saveFailed', { error: String(error) }));
    }
  }, [onRefresh, notification, t]);

  const handleAction = useCallback(async (action: 'start' | 'stop' | 'restart', serverId: string) => {
    try {
      if (action === 'start') await MCPAPI.startServer(serverId);
      else if (action === 'stop') await MCPAPI.stopServer(serverId);
      else await MCPAPI.restartServer(serverId);
      await onRefresh();
    } catch (error) {
      log.error('MCP action failed', { action, serverId, error });
      notification.error(String(error));
    }
  }, [onRefresh, notification]);

  const handleDelete = useCallback(async () => {
    if (!deleteTarget) return;
    try {
      await MCPAPI.deleteServer({ serverId: deleteTarget.id });
      setDeleteTarget(null);
      await onRefresh();
    } catch (error) {
      notification.error(String(error));
    }
  }, [deleteTarget, onRefresh, notification]);

  return (
    <>
      <Modal
        isOpen={open}
        onClose={onClose}
        title={t('sidebar.manageServers')}
        size="large"
        contentInset
        contentClassName="modal__content--fill-flex"
      >
        <div className="tools-mcp__manager">
          <div className="tools-mcp__manager-stats">
            <span>{t('mcp.header.serversTotal', { count: servers.length })}</span>
            {errorCount > 0 && (
              <span className="tools-mcp__stat--danger">{t('mcp.header.errors', { count: errorCount })}</span>
            )}
            <div className="tools-mcp__manager-spacer" />
            <Button variant="primary" size="small" onClick={() => void handleOpenEditor()}>
              <FileJson size={13} />
              <span>{t('mcp.actions.editConfig')}</span>
            </Button>
          </div>

          {servers.length === 0 ? (
            <div className="tools-mcp__empty">
              <Server size={36} strokeWidth={1.3} />
              <h3>{t('mcp.empty.title')}</h3>
              <p>{t('mcp.empty.hint')}</p>
              <Button variant="primary" onClick={() => void handleOpenEditor()}>
                <Plus size={14} />
                <span>{t('mcp.empty.cta')}</span>
              </Button>
            </div>
          ) : (
            <ul className="tools-mcp__manager-list">
              {servers.map(s => {
                const isRunning = /Connected|Healthy|Starting|Reconnecting/.test(s.status);
                return (
                  <li key={s.id} className="tools-mcp__manager-row">
                    <StatusDot status={s.status} />
                    <div className="tools-mcp__manager-main">
                      <span className="tools-mcp__manager-name">{s.name || s.id}</span>
                      <span className="tools-mcp__manager-meta">
                        <code>{s.id}</code>
                        <span>· {s.transport}</span>
                        <span>· {s.status}</span>
                      </span>
                    </div>
                    <div className="tools-mcp__actions">
                      {isRunning ? (
                        <button type="button" className="tools-mcp__icon-btn" onClick={() => void handleAction('stop', s.id)} aria-label={t('mcp.server.stop')}>
                          <Square size={13} />
                        </button>
                      ) : (
                        <button type="button" className="tools-mcp__icon-btn" onClick={() => void handleAction('start', s.id)} aria-label={t('mcp.server.start')} disabled={!s.startSupported}>
                          <Play size={13} />
                        </button>
                      )}
                      <button type="button" className="tools-mcp__icon-btn" onClick={() => void handleAction('restart', s.id)} aria-label={t('mcp.server.restart')}>
                        <RotateCw size={13} />
                      </button>
                      <button type="button" className="tools-mcp__icon-btn tools-mcp__icon-btn--danger" onClick={() => setDeleteTarget(s)} aria-label={t('mcp.server.delete')}>
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </Modal>

      <McpConfigEditor
        open={editorOpen}
        initialValue={editorInitial}
        onCancel={() => setEditorOpen(false)}
        onSave={handleSaveEditor}
      />

      <ConfirmDialog
        isOpen={Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('mcp.server.deleteConfirmTitle')}
        message={t('mcp.server.deleteConfirmMessage', { id: deleteTarget?.id ?? '' })}
        type="warning"
        confirmDanger
      />
    </>
  );
};

// ─────────────────────────────────────────────────────────────────────────────
// JSON config editor
// ─────────────────────────────────────────────────────────────────────────────

const McpConfigEditor: React.FC<{
  open: boolean;
  initialValue: string;
  onCancel: () => void;
  onSave: (raw: string) => Promise<void> | void;
}> = ({ open, initialValue, onCancel, onSave }) => {
  const { t } = useTranslation('scenes/tools');
  const [value, setValue] = useState(initialValue);
  const [busy, setBusy] = useState(false);

  useEffect(() => { if (open) setValue(initialValue); }, [open, initialValue]);

  return (
    <Modal isOpen={open} onClose={onCancel} title={t('mcp.editor.title')} size="large" contentInset>
      <div className="tools-mcp__editor">
        <p className="tools-mcp__editor-hint">{t('mcp.editor.hint')}</p>
        <div className="tools-mcp__editor-examples">
          <div className="tools-mcp__editor-example">
            <h4>{t('mcp.editor.examples.localProcess')}</h4>
            <pre>{MCP_LOCAL_PROCESS_EXAMPLE}</pre>
          </div>
          <div className="tools-mcp__editor-example">
            <h4>{t('mcp.editor.examples.remoteService')}</h4>
            <pre>{MCP_REMOTE_SERVICE_EXAMPLE}</pre>
          </div>
        </div>
        <textarea
          className="tools-mcp__editor-area"
          spellCheck={false}
          value={value}
          onChange={e => setValue(e.target.value)}
          rows={20}
        />
        <div className="tools-mcp__editor-actions">
          <Button variant="secondary" size="small" onClick={onCancel} disabled={busy}>
            {t('mcp.editor.cancel')}
          </Button>
          <Button
            variant="primary"
            size="small"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try { await onSave(value); } finally { setBusy(false); }
            }}
          >
            {t('mcp.editor.save')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default ToolsScene;
