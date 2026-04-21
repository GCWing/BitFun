/**
 * Design Canvas — right-side tab for a single Design Artifact.
 *
 * Full workbench: Preview / Code / Split / Diff / History view modes, Inspector
 * drawer (element/tokens/assets), Export menu (HTML / zip / screenshot / skills),
 * snapshot with auto-thumbnail, element picker, Continue-with-Agent, editing
 * lock (readonly + rebase), viewport switcher.
 */

import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import {
  Code as CodeIcon,
  Eye,
  Columns,
  History,
  GitCompare,
  ExternalLink,
  Download,
  MousePointer2,
  Smartphone,
  Tablet,
  Monitor,
  Wand2,
  Loader2,
  Camera,
  FileArchive,
  FileText,
  Lock,
  Unlock,
  ChevronDown,
} from 'lucide-react';
import { CodeEditor, DiffEditor } from '@/tools/editor';
import { workspaceAPI, systemAPI } from '@/infrastructure/api';
import { globalEventBus } from '@/infrastructure/event-bus';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import DesignArtifactFrame from './DesignArtifactFrame';
import DesignInspector from './DesignInspector';
import { designArtifactAPI } from './api';
import {
  useDesignArtifactStore,
  type DesignArtifactManifest,
  type SelectedElement,
} from './store/designArtifactStore';
import './DesignCanvasPanel.scss';

const log = createLogger('DesignCanvasPanel');

type ViewMode = 'preview' | 'code' | 'split' | 'diff' | 'history';
type Viewport = 'desktop' | 'tablet' | 'mobile';

export interface DesignCanvasPanelProps {
  artifactId: string;
  workspacePath?: string;
  initialManifest?: DesignArtifactManifest;
}

const VIEWPORT_ICONS: Record<Viewport, React.ReactNode> = {
  desktop: <Monitor size={14} />,
  tablet: <Tablet size={14} />,
  mobile: <Smartphone size={14} />,
};

function formatRelativeTime(iso: string | undefined, t: TFunction<'flow-chat'>): string {
  if (!iso) return '';
  try {
    const ts = new Date(iso).getTime();
    const diff = Date.now() - ts;
    if (diff < 60_000) return t('designCanvas.panel.time.justNow');
    if (diff < 3_600_000) {
      return t('designCanvas.panel.time.minutesAgo', { count: Math.round(diff / 60_000) });
    }
    if (diff < 86_400_000) {
      return t('designCanvas.panel.time.hoursAgo', { count: Math.round(diff / 3_600_000) });
    }
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

/** Lock must be refreshed within LOCK_STALE_SECONDS; backend mirrors this. */
const LOCK_STALE_MS = 120_000;

function buildTimestamp(): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, '0');
  return [
    now.getFullYear(),
    pad(now.getMonth() + 1),
    pad(now.getDate()),
    pad(now.getHours()),
    pad(now.getMinutes()),
  ].join('-');
}

async function saveBlobToDownloads(blob: Blob, fileName: string): Promise<string> {
  const downloadsPath = await downloadDir();
  const filePath = await join(downloadsPath, fileName);
  const arrayBuffer = await blob.arrayBuffer();
  await writeFile(filePath, new Uint8Array(arrayBuffer));
  return filePath;
}

async function waitForNextPaint(): Promise<void> {
  await new Promise<void>((resolve) => {
    window.setTimeout(() => {
      window.requestAnimationFrame(() => resolve());
    }, 0);
  });
}

function getSnapshotVersionPath(snapshotManifest?: DesignArtifactManifest | null): string {
  const versionId = snapshotManifest?.current_version;
  const root = snapshotManifest?.root;
  if (!versionId || !root) return '';
  return `${root.replace(/[\\/]$/, '')}/versions/${versionId}`.replace(/\\/g, '/');
}

function notifyPathSuccess(prefix: string, filePath: string): void {
  const revealExportedFile = async () => {
    if (typeof window === 'undefined' || !('__TAURI__' in window)) {
      return;
    }
    try {
      await workspaceAPI.revealInExplorer(filePath);
    } catch (error) {
      log.error('Failed to reveal design export path in file manager', { filePath, error });
    }
  };

  notificationService.success(`${prefix}${filePath}`, {
    messageNode: (
      <>
        {prefix}
        <button
          type="button"
          className="notification-item__path-link"
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            void revealExportedFile();
          }}
        >
          {filePath}
        </button>
      </>
    ),
  });
}

function formatLockAge(since: string | undefined, t: TFunction<'flow-chat'>): string {
  if (!since) return '';
  const parsed = Date.parse(since);
  if (Number.isNaN(parsed)) return '';
  const diff = Date.now() - parsed;
  if (diff < 60_000) {
    return t('designCanvas.panel.lockAge.seconds', {
      count: Math.max(1, Math.round(diff / 1000)),
    });
  }
  if (diff < 3_600_000) {
    return t('designCanvas.panel.lockAge.minutes', { count: Math.round(diff / 60_000) });
  }
  return t('designCanvas.panel.lockAge.hours', { count: Math.round(diff / 3_600_000) });
}

export const DesignCanvasPanel: React.FC<DesignCanvasPanelProps> = ({
  artifactId,
  workspacePath,
  initialManifest,
}) => {
  const { t } = useTranslation('flow-chat');
  const { workspacePath: currentWorkspacePath } = useCurrentWorkspace();
  const artifactStateMeta = useDesignArtifactStore(
    useShallow((s) => {
      const artifact = s.artifacts[artifactId];
      return {
        manifest: artifact?.manifest,
      };
    })
  );
  const filesCache = useDesignArtifactStore((s) => s.artifacts[artifactId]?.fileCache ?? {});
  const selectedElement = useDesignArtifactStore((s) => s.artifacts[artifactId]?.selectedElement);
  const extractedTokens = useDesignArtifactStore((s) => s.artifacts[artifactId]?.tokens);
  const upsertManifest = useDesignArtifactStore((s) => s.upsertManifest);
  const setFileContent = useDesignArtifactStore((s) => s.setFileContent);
  const setSelectedElement = useDesignArtifactStore((s) => s.setSelectedElement);
  const setTokens = useDesignArtifactStore((s) => s.setTokens);

  useEffect(() => {
    if (initialManifest && !artifactStateMeta.manifest) {
      upsertManifest(initialManifest, 'ok');
    }
  }, [initialManifest, artifactStateMeta.manifest, upsertManifest]);

  const manifest = artifactStateMeta.manifest ?? initialManifest;

  const [viewMode, setViewMode] = useState<ViewMode>('preview');
  const [viewport, setViewport] = useState<Viewport>('desktop');
  const [pickerActive, setPickerActive] = useState(false);
  const [activeFile, setActiveFile] = useState<string>(manifest?.entry ?? '');
  const [isLoadingFile, setIsLoadingFile] = useState(false);
  const [isInspectorOpen, setIsInspectorOpen] = useState(false);
  const [isExportMenuOpen, setIsExportMenuOpen] = useState(false);
  const [diffFromVersion, setDiffFromVersion] = useState<string>('');
  const [diffToVersion, setDiffToVersion] = useState<string>('current');
  const [isSnapshotting, setIsSnapshotting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  const iframeRef = useRef<HTMLIFrameElement | null>(null);

  useEffect(() => {
    if (manifest?.entry && !activeFile) {
      setActiveFile(manifest.entry);
    }
  }, [manifest?.entry, activeFile]);

  useEffect(() => {
    if (manifest?.versions && manifest.versions.length > 0 && !diffFromVersion) {
      setDiffFromVersion(manifest.versions[manifest.versions.length - 1].id);
    }
  }, [manifest?.versions, diffFromVersion]);

  const artifactRoot = manifest?.root ?? '';
  const currentRoot = artifactRoot ? `${artifactRoot.replace(/[\\/]$/, '')}/current` : '';
  const versionsRoot = artifactRoot ? `${artifactRoot.replace(/[\\/]$/, '')}/versions` : '';
  const effectiveWorkspacePath = workspacePath || currentWorkspacePath;
  const isAgentLocked = Boolean(
    manifest?.editing_lock && manifest.editing_lock.holder !== 'human'
  );
  const phaseLabel = useMemo(() => {
    if (!manifest?.current_version) return t('designCanvas.panel.phase.scaffold');
    if (isSnapshotting) return t('designCanvas.panel.phase.finishing');
    if (pickerActive) return t('designCanvas.panel.phase.sampling');
    return t('designCanvas.panel.phase.iterating');
  }, [manifest?.current_version, isSnapshotting, pickerActive, t]);

  // Lock staleness: even if backend still has the lock record, a UI that didn't
  // hear back within `LOCK_STALE_MS` should surface "expired" so the user knows they
  // can safely take it over.
  const lockIsStale = useMemo(() => {
    const since = manifest?.editing_lock?.since;
    if (!since) return false;
    const parsed = Date.parse(since);
    if (Number.isNaN(parsed)) return true;
    return Date.now() - parsed > LOCK_STALE_MS;
  }, [manifest?.editing_lock?.since]);

  const ensureFileLoaded = useCallback(
    async (relative: string) => {
      if (!manifest || !relative) return;
      if (filesCache[relative] !== undefined) return;
      const absolute = `${currentRoot.replace(/[\\/]$/, '')}/${relative}`.replace(
        /\\/g,
        '/'
      );
      setIsLoadingFile(true);
      try {
        const content = await workspaceAPI.readFileContent(absolute);
        setFileContent(manifest.id, relative, content ?? '');
      } catch (err) {
        log.warn('Failed to load design artifact file', { relative, err });
        setFileContent(manifest.id, relative, '');
      } finally {
        setIsLoadingFile(false);
      }
    },
    [currentRoot, filesCache, manifest, setFileContent]
  );

  // Lazy-load policy: load the entry HTML first, then parse it to find only
  // the CSS/JS that the entry actually references and load those. Everything
  // else is loaded on demand when the user clicks the corresponding tab.
  //
  // Previously every *.css/*.js/*.html/*.json in the manifest was eagerly
  // read on mount, which caused a thundering-herd of workspaceAPI reads on
  // large artifacts (and blocked the first preview paint on unrelated files).
  useEffect(() => {
    if (!manifest) return;
    ensureFileLoaded(manifest.entry);
  }, [manifest?.id, manifest?.entry, ensureFileLoaded]);

  useEffect(() => {
    if (!manifest) return;
    const entryHtml = filesCache[manifest.entry];
    if (!entryHtml) return;
    const referenced = new Set<string>();
    const addMatch = (re: RegExp) => {
      for (const m of entryHtml.matchAll(re)) {
        const raw = (m[1] || '').trim().replace(/^['"]|['"]$/g, '').replace(/^\.\//, '').replace(/^\//, '');
        if (raw && !/^[a-z]+:/i.test(raw) && !raw.startsWith('//') && !raw.startsWith('data:')) {
          referenced.add(raw);
        }
      }
    };
    addMatch(/<link[^>]*?href=["']([^"']+)["']/gi);
    addMatch(/<script[^>]*?src=["']([^"']+)["']/gi);
    // Only load assets that are actually files in the artifact manifest.
    const known = new Set(manifest.files.map((f) => f.path));
    for (const path of referenced) {
      if (known.has(path)) ensureFileLoaded(path);
    }
  }, [manifest, filesCache[manifest?.entry || ''], ensureFileLoaded]);

  // ---------- Picker + Inspector ----------

  const handleSelectElement = useCallback(
    (selection: SelectedElement) => {
      if (!manifest) return;
      setSelectedElement(manifest.id, selection);
      if (!isInspectorOpen) setIsInspectorOpen(true);
    },
    [manifest, setSelectedElement, isInspectorOpen]
  );

  const handleTokensExtracted = useCallback(
    (tokens: Record<string, string>) => {
      if (!manifest) return;
      setTokens(manifest.id, tokens);
    },
    [manifest, setTokens]
  );

  // ---------- Continue-with-Agent ----------

  const buildContinueContext = useCallback(() => {
    if (!manifest) return '';
    const selection = selectedElement;
    const parts: string[] = [];
    parts.push(`Continue working on design artifact \`${manifest.id}\` (${manifest.title}).`);
    if (manifest.current_version) {
      parts.push(`Current version: ${manifest.current_version}.`);
    }
    if (selection?.domPath) {
      const css = selection.computedStyle;
      const highlights: string[] = [];
      if (css) {
        for (const k of ['color', 'background-color', 'font-size', 'font-family']) {
          const v = css[k];
          if (v) highlights.push(`${k}:${v}`);
        }
      }
      parts.push(
        `Focus element: \`${selection.domPath}\`` +
          (selection.textExcerpt ? ` — "${selection.textExcerpt}"` : '') +
          (highlights.length ? `\nComputed: ${highlights.join('; ')}` : '')
      );
    }
    parts.push(
      'Update the artifact via DesignArtifact (update_file / snapshot, pass expected_version to avoid overwriting concurrent human edits).'
    );
    return parts.join('\n');
  }, [selectedElement, manifest]);

  const handleContinueWithAgent = useCallback(() => {
    const text = buildContinueContext();
    if (text) {
      globalEventBus.emit('fill-chat-input', { content: text }, 'DesignCanvasPanel');
    }
  }, [buildContinueContext]);

  const handleCopyContext = useCallback(() => {
    const text = buildContinueContext();
    if (!text) return;
    navigator.clipboard?.writeText(text).then(
      () => notificationService.success(t('designCanvas.panel.notifications.copyContextOk')),
      () => notificationService.error(t('designCanvas.panel.notifications.copyContextFail'))
    );
  }, [buildContinueContext, t]);

  // ---------- Open entry externally ----------

  const handleOpenExternal = useCallback(async () => {
    if (!manifest) return;
    const absolute = `${currentRoot.replace(/[\\/]$/, '')}/${manifest.entry}`.replace(
      /\\/g,
      '/'
    );
    try {
      await systemAPI.openPath(absolute);
    } catch (err) {
      log.warn('systemAPI.openPath failed, falling back to file URL', { absolute, err });
      try {
        const fileUrl = `file:///${absolute.replace(/^\//, '')}`;
        window.open(fileUrl, '_blank');
      } catch (fallbackErr) {
        log.warn('Failed to open delivery file externally', fallbackErr);
        notificationService.error(t('designCanvas.panel.notifications.openExternalFail'));
      }
    }
  }, [currentRoot, manifest, t]);

  // ---------- Save from Monaco (lock-aware) ----------

  const handleCodeSave = useCallback(
    async (content: string) => {
      if (!manifest) return;
      if (isAgentLocked && !lockIsStale) {
        notificationService.warning(
          t('designCanvas.panel.notifications.agentEditingBlocked', {
            holder: String(manifest.editing_lock?.holder ?? ''),
          })
        );
        return;
      }
      setIsSaving(true);
      try {
        const res = await designArtifactAPI.updateFile(manifest.id, activeFile, content, {
          expectedVersion: manifest.current_version || undefined,
          as: 'human',
          workspacePath: effectiveWorkspacePath,
        });
        setFileContent(manifest.id, activeFile, content);
        if (res.manifest) {
          notificationService.success(t('designCanvas.panel.notifications.saved'));
        }
      } catch (err: any) {
        const msg = String(err?.message || err);
        if (msg.includes('VERSION_CONFLICT')) {
          notificationService.error(t('designCanvas.panel.notifications.versionConflict'));
          try {
            const list = await designArtifactAPI.list(workspacePath);
            const fresh = list.find((m) => m.id === manifest.id);
            if (fresh) upsertManifest(fresh, 'manifest-updated');
          } catch {
            /* no-op */
          }
        } else if (msg.includes('EDIT_LOCKED')) {
          notificationService.error(t('designCanvas.panel.notifications.editLocked'));
        } else {
          log.error('Save failed', err);
          notificationService.error(
            t('designCanvas.panel.notifications.saveFailed', { message: msg })
          );
        }
      } finally {
        setIsSaving(false);
      }
    },
    [
      manifest,
      isAgentLocked,
      lockIsStale,
      activeFile,
      effectiveWorkspacePath,
      setFileContent,
      upsertManifest,
      workspacePath,
      t,
    ]
  );

  // Debounced auto-snapshot: if the user makes a run of saves, queue a
  // snapshot 45s after the last edit so the version history captures it
  // without requiring them to remember to press the Snapshot button.
  // Skipped when the lock is held by the agent (they will snapshot at their
  // own milestones) or while a manual snapshot is in-flight.
  const autoSnapshotTimer = useRef<number | null>(null);
  useEffect(() => {
    if (!manifest || isAgentLocked || isSnapshotting) return;
    if (autoSnapshotTimer.current) {
      window.clearTimeout(autoSnapshotTimer.current);
    }
    autoSnapshotTimer.current = window.setTimeout(async () => {
      try {
        await designArtifactAPI.snapshot(manifest.id, {
          summary: t('designCanvas.panel.snapshot.autoSummary'),
          author: 'human',
          workspacePath: effectiveWorkspacePath,
        });
      } catch (err) {
        log.warn('Auto-snapshot failed', err);
      }
    }, 45_000);
    return () => {
      if (autoSnapshotTimer.current) window.clearTimeout(autoSnapshotTimer.current);
    };
  }, [manifest?.updated_at, manifest?.id, isAgentLocked, isSnapshotting, effectiveWorkspacePath, t]);

  // ---------- Snapshot ----------

  const handleSnapshot = useCallback(async () => {
    if (!manifest) return;
    setIsSnapshotting(true);
    try {
      // Lightweight prompt. A richer in-app dialog would be nicer, but
      // `window.prompt` remains acceptable because it's synchronous and keeps
      // the "take a snapshot" flow a single click away.
      const summary = window.prompt(
        t('designCanvas.panel.snapshot.promptMessage'),
        t('designCanvas.panel.snapshot.promptDefault')
      );
      if (summary === null) {
        setIsSnapshotting(false);
        return;
      }
      const snapshotResult = await designArtifactAPI.snapshot(manifest.id, {
        summary: summary || t('designCanvas.panel.snapshot.manualSummaryDefault'),
        author: 'human',
        workspacePath: effectiveWorkspacePath,
      });
      const snapshotPath = getSnapshotVersionPath(snapshotResult.manifest ?? manifest);
      if (snapshotPath) {
        notifyPathSuccess(t('designCanvas.panel.notifications.snapshotPathPrefix'), snapshotPath);
      } else {
        notificationService.success(t('designCanvas.panel.notifications.snapshotOk'));
      }
      setIsSnapshotting(false);

    } catch (err: any) {
      log.error('Snapshot failed', err);
      notificationService.error(
        t('designCanvas.panel.notifications.snapshotFailed', {
          message: String(err?.message || err),
        })
      );
      setIsSnapshotting(false);
    } finally {
      // Success clears the busy state before the thumbnail refresh continues in
      // the background; this fallback covers early exits and unexpected paths.
      setIsSnapshotting(false);
    }
  }, [manifest, effectiveWorkspacePath, t]);

  // ---------- Export menu ----------

  const handleDownloadEntryHtml = useCallback(async () => {
    if (!manifest) return;
    const loading = notificationService.loading({
      title: t('designCanvas.panel.notifications.exportingTitle'),
      message: t('designCanvas.panel.notifications.exportHtmlMessage'),
    });
    await waitForNextPaint();
    try {
      let content = filesCache[manifest.entry];
      if (!content) {
        await ensureFileLoaded(manifest.entry);
        content = useDesignArtifactStore.getState().artifacts[manifest.id]?.fileCache[manifest.entry];
      }
      if (!content) return;
      const blob = new Blob([content], { type: 'text/html;charset=utf-8' });
      const filePath = await saveBlobToDownloads(
        blob,
        `${manifest.id}-${buildTimestamp()}.html`
      );
      loading.cancel();
      notifyPathSuccess(t('designCanvas.panel.notifications.exportToPrefix'), filePath);
    } catch (err: any) {
      log.error('HTML export failed', err);
      loading.fail(
        t('designCanvas.panel.notifications.exportFailed', { message: String(err?.message || err) })
      );
    }
  }, [manifest, filesCache, ensureFileLoaded, t]);

  const handleZipExport = useCallback(async () => {
    if (!manifest) return;
    const loading = notificationService.loading({
      title: t('designCanvas.panel.notifications.exportingTitle'),
      message: t('designCanvas.panel.notifications.exportZipMessage'),
    });
    await waitForNextPaint();
    try {
      const res = await designArtifactAPI.zipExport(manifest.id, effectiveWorkspacePath);
      const exportPath = (res.export_path as string) || '';
      loading.cancel();
      if (exportPath) {
        notifyPathSuccess(t('designCanvas.panel.notifications.exportToPrefix'), exportPath);
      } else {
        notificationService.success(t('designCanvas.panel.notifications.exportDesignFolder'));
      }
    } catch (err: any) {
      log.error('Zip export failed', err);
      loading.fail(
        t('designCanvas.panel.notifications.exportFailed', { message: String(err?.message || err) })
      );
    }
  }, [manifest, effectiveWorkspacePath, t]);

  const handleScreenshot = useCallback(async () => {
    notificationService.info(t('designCanvas.panel.notifications.screenshotInfo'));
  }, [t]);

  const handleSkillExport = useCallback(
    (format: 'pdf' | 'pptx') => {
      if (!manifest) return;
      const entryPath = `${currentRoot.replace(/[\\/]$/, '')}/${manifest.entry}`.replace(
        /\\/g,
        '/'
      );
      const skill = format === 'pdf' ? 'pdf' : 'slides';
      const prompt =
        `Use the ${skill} skill to convert design artifact \`${manifest.id}\` into ${format.toUpperCase()}.\n` +
        `Source HTML: ${entryPath}\n` +
        `Write the output next to the artifact root directory: ${artifactRoot}\n` +
        `Report the output path when done.`;
      globalEventBus.emit('fill-chat-input', { content: prompt }, 'DesignCanvasPanel');
      notificationService.success(
        t('designCanvas.panel.notifications.skillExportInserted', { format: format.toUpperCase() })
      );
    },
    [manifest, currentRoot, artifactRoot, t]
  );

  // ---------- Edit lock toggle (manual from UI) ----------

  const handleToggleLock = useCallback(async () => {
    if (!manifest) return;
    try {
      if (manifest.editing_lock) {
        await designArtifactAPI.releaseLock(manifest.id, workspacePath);
        notificationService.success(t('designCanvas.panel.notifications.lockReleased'));
      } else {
        await designArtifactAPI.acquireLock(manifest.id, {
          holder: 'human',
          note: t('designCanvas.panel.lock.manualAcquireNote'),
          workspacePath: effectiveWorkspacePath,
        });
        notificationService.success(t('designCanvas.panel.notifications.lockAcquired'));
      }
    } catch (err: any) {
      log.warn('Toggle lock failed', err);
      notificationService.error(
        t('designCanvas.panel.notifications.lockToggleFailed', { message: String(err?.message || err) })
      );
    }
  }, [manifest, workspacePath, effectiveWorkspacePath, t]);

  // ---------- Render ----------

  if (!manifest) {
    return (
      <div className="design-canvas-panel design-canvas-panel--empty">
        <div className="design-canvas-panel__empty">
          <Wand2 size={24} />
          <div className="design-canvas-panel__empty-title">{t('designCanvas.panel.emptyTitle')}</div>
          <div className="design-canvas-panel__empty-subtitle">
            {t('designCanvas.panel.emptySubtitle')}
          </div>
        </div>
      </div>
    );
  }

  const activeFilePath = activeFile || manifest.entry;
  const activeFileAbsolute = `${currentRoot.replace(/[\\/]$/, '')}/${activeFilePath}`.replace(
    /\\/g,
    '/'
  );

  const preview = (
    <DesignArtifactFrame
      artifactId={manifest.id}
      entry={manifest.entry}
      files={filesCache}
      viewport={viewport}
      pickerActive={pickerActive}
      onSelectElement={handleSelectElement}
      onTokens={handleTokensExtracted}
      frameRef={iframeRef}
    />
  );

  const codeView = (
    <div className="design-canvas-panel__code" key={activeFileAbsolute}>
      <div className="design-canvas-panel__code-tabs">
        {manifest.files.map((f) => (
          <button
            key={f.path}
            type="button"
            className={`design-canvas-panel__code-tab${
              f.path === activeFilePath ? ' design-canvas-panel__code-tab--active' : ''
            }`}
            onClick={() => {
              setActiveFile(f.path);
              ensureFileLoaded(f.path);
            }}
          >
            {f.path}
          </button>
        ))}
      </div>
      <div className="design-canvas-panel__code-body">
        {isLoadingFile ? (
          <div className="design-canvas-panel__loading">
            <Loader2 size={16} className="spin" /> {t('designCanvas.panel.loadingFile')}
          </div>
        ) : (
          <CodeEditor
            filePath={activeFileAbsolute}
            fileName={activeFilePath.split('/').pop()}
            workspacePath={effectiveWorkspacePath}
            readOnly={isAgentLocked}
            showLineNumbers
            showMinimap={false}
            theme="vs-dark"
            onSave={(content) => handleCodeSave(content)}
            onContentChange={(content, hasChanges) => {
              if (!hasChanges) return;
              setFileContent(manifest.id, activeFilePath, content);
            }}
          />
        )}
      </div>
    </div>
  );

  const diffView = (() => {
    const fromIsBase = diffFromVersion && manifest.versions.find((v) => v.id === diffFromVersion);
    const toIsCurrent = diffToVersion === 'current';
    const fromManifest = fromIsBase || null;
    const toManifest = toIsCurrent
      ? null
      : manifest.versions.find((v) => v.id === diffToVersion) || null;
    const fromLabel = fromManifest ? fromManifest.id.slice(0, 8) : '—';
    const toLabel = toIsCurrent
      ? t('designCanvas.panel.diffLabelCurrent')
      : toManifest?.id.slice(0, 8) || '—';
    const fromPath = fromManifest
      ? `${versionsRoot.replace(/[\\/]$/, '')}/${fromManifest.id}/${activeFilePath}`.replace(
          /\\/g,
          '/'
        )
      : '';
    const toPath = toIsCurrent
      ? activeFileAbsolute
      : toManifest
        ? `${versionsRoot.replace(/[\\/]$/, '')}/${toManifest.id}/${activeFilePath}`.replace(
            /\\/g,
            '/'
          )
        : '';

    return (
      <div className="design-canvas-panel__diff">
        <div className="design-canvas-panel__diff-toolbar">
          <div className="design-canvas-panel__diff-selector">
            <label>{t('designCanvas.panel.diffFrom')}</label>
            <select
              value={diffFromVersion}
              onChange={(e) => setDiffFromVersion(e.target.value)}
            >
              {manifest.versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id.slice(0, 8)} · {v.summary}
                </option>
              ))}
            </select>
          </div>
          <div className="design-canvas-panel__diff-selector">
            <label>{t('designCanvas.panel.diffTo')}</label>
            <select
              value={diffToVersion}
              onChange={(e) => setDiffToVersion(e.target.value)}
            >
              <option value="current">{t('designCanvas.panel.diffCurrentWorkingCopy')}</option>
              {manifest.versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id.slice(0, 8)} · {v.summary}
                </option>
              ))}
            </select>
          </div>
          <div className="design-canvas-panel__diff-file">{activeFilePath}</div>
        </div>
        <div className="design-canvas-panel__diff-body">
          {manifest.versions.length === 0 ? (
            <div className="design-canvas-panel__history-empty">
              {t('designCanvas.panel.diffNoSnapshots')}
            </div>
          ) : (
            <DesignDiffLoader
              fromPath={fromPath}
              toPath={toPath}
              fromLabel={fromLabel}
              toLabel={toLabel}
            />
          )}
        </div>
      </div>
    );
  })();

  const historyView = (
    <div className="design-canvas-panel__history">
      <h4>{t('designCanvas.panel.historyTitle')}</h4>
      {manifest.versions.length === 0 ? (
        <div className="design-canvas-panel__history-empty">
          {t('designCanvas.panel.historyEmpty')}
        </div>
      ) : (
        <ul className="design-canvas-panel__history-list">
          {[...manifest.versions].reverse().map((v) => (
            <li
              key={v.id}
              className={`design-canvas-panel__history-item${
                v.id === manifest.current_version
                  ? ' design-canvas-panel__history-item--current'
                  : ''
              }`}
            >
              <div className="design-canvas-panel__history-head">
                <span className="design-canvas-panel__history-id">{v.id.slice(0, 12)}</span>
                <span className="design-canvas-panel__history-author">{v.author}</span>
                <span className="design-canvas-panel__history-time">
                  {formatRelativeTime(v.created_at, t)}
                </span>
              </div>
              <div className="design-canvas-panel__history-summary">{v.summary}</div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );

  return (
    <div className="design-canvas-panel">
      {isAgentLocked && (
        <div className="design-canvas-panel__lock-banner" role="status">
          <Lock size={12} />
          <span>
            {lockIsStale
              ? t('designCanvas.panel.lockBanner.stale')
              : t('designCanvas.panel.lockBanner.agentEditing')}
            {t('designCanvas.panel.lockBanner.holderLead')}
            {manifest.editing_lock?.holder}
            {manifest.editing_lock?.since
              ? t('designCanvas.panel.lockBanner.heldFor', {
                  duration: formatLockAge(manifest.editing_lock.since, t),
                })
              : ''}
            {lockIsStale
              ? t('designCanvas.panel.lockBanner.suffixStale')
              : t('designCanvas.panel.lockBanner.suffixReadOnly')}
          </span>
          {lockIsStale && (
            <button
              type="button"
              className="design-canvas-panel__lock-takeover"
              onClick={async () => {
                try {
                  await designArtifactAPI.acquireLock(manifest.id, {
                    holder: 'human',
                    note: t('designCanvas.panel.lockBanner.takeOverNote'),
                    force: true,
                    workspacePath: effectiveWorkspacePath,
                  });
                  notificationService.success(t('designCanvas.panel.notifications.takeoverLockOk'));
                } catch (err: any) {
                  notificationService.error(
                    t('designCanvas.panel.notifications.takeoverLockFail', {
                      message: String(err?.message || err),
                    })
                  );
                }
              }}
            >
              <Unlock size={11} /> {t('designCanvas.panel.lockBanner.takeOver')}
            </button>
          )}
        </div>
      )}

      <div className="design-canvas-panel__toolbar">
        <div className="design-canvas-panel__toolbar-left">
          <span className="design-canvas-panel__title">{manifest.title}</span>
          <span className="design-canvas-panel__kind">{manifest.kind}</span>
          <span className="design-canvas-panel__phase">{phaseLabel}</span>
          {manifest.current_version && (
            <span className="design-canvas-panel__version">
              v{manifest.current_version.slice(0, 8)}
            </span>
          )}
          {isSaving && (
            <span className="design-canvas-panel__hint">
              <Loader2 size={11} className="spin" />{' '}
              {t('designCanvas.panel.savingHint', { path: activeFilePath })}
            </span>
          )}
        </div>
        <div className="design-canvas-panel__toolbar-center">
          {(['preview', 'code', 'split', 'diff', 'history'] as ViewMode[]).map((mode) => (
            <button
              key={mode}
              type="button"
              className={`design-canvas-panel__mode-btn${
                viewMode === mode ? ' design-canvas-panel__mode-btn--active' : ''
              }`}
              onClick={() => setViewMode(mode)}
              title={t(`designCanvas.panel.mode.${mode}`)}
            >
              {mode === 'preview' && <Eye size={14} />}
              {mode === 'code' && <CodeIcon size={14} />}
              {mode === 'split' && <Columns size={14} />}
              {mode === 'diff' && <GitCompare size={14} />}
              {mode === 'history' && <History size={14} />}
              <span>
                {t(`designCanvas.panel.mode.${mode}`)}
              </span>
            </button>
          ))}
        </div>
        <div className="design-canvas-panel__toolbar-right">
          {(['desktop', 'tablet', 'mobile'] as Viewport[]).map((vp) => (
            <button
              key={vp}
              type="button"
              className={`design-canvas-panel__viewport-btn${
                viewport === vp ? ' design-canvas-panel__viewport-btn--active' : ''
              }`}
              onClick={() => setViewport(vp)}
              title={vp}
            >
              {VIEWPORT_ICONS[vp]}
            </button>
          ))}
          <button
            type="button"
            className={`design-canvas-panel__picker-btn${
              pickerActive ? ' design-canvas-panel__picker-btn--active' : ''
            }`}
            onClick={() => setPickerActive((v) => !v)}
            title={t('designCanvas.panel.pickerTitle')}
          >
            <MousePointer2 size={14} />
          </button>
          <button
            type="button"
            className={`design-canvas-panel__picker-btn${
              isInspectorOpen ? ' design-canvas-panel__picker-btn--active' : ''
            }`}
            onClick={() => setIsInspectorOpen((v) => !v)}
            title={t('designCanvas.panel.inspectorTitle')}
          >
            <FileText size={14} />
          </button>
          <button
            type="button"
            className="design-canvas-panel__action-btn"
            onClick={handleSnapshot}
            disabled={isSnapshotting}
            title={t('designCanvas.panel.snapshotTitle')}
          >
            {isSnapshotting ? <Loader2 size={14} className="spin" /> : <Camera size={14} />}
            <span>{t('designCanvas.panel.snapshotLabel')}</span>
          </button>
          <button
            type="button"
            className="design-canvas-panel__action-btn"
            onClick={handleContinueWithAgent}
            title={t('designCanvas.panel.continueTitle')}
          >
            <Wand2 size={14} />
            <span>{t('designCanvas.panel.continueLabel')}</span>
          </button>
          <button
            type="button"
            className={`design-canvas-panel__action-btn${
              manifest.editing_lock ? ' design-canvas-panel__action-btn--locked' : ''
            }`}
            onClick={handleToggleLock}
            title={
              manifest.editing_lock
                ? t('designCanvas.panel.releaseLockTitle', {
                    holder: manifest.editing_lock.holder,
                  })
                : t('designCanvas.panel.acquireLockTitle')
            }
          >
            {manifest.editing_lock ? <Lock size={14} /> : <Unlock size={14} />}
          </button>
          <div className="design-canvas-panel__export-wrap">
            <button
              type="button"
              className="design-canvas-panel__action-btn"
              onClick={() => setIsExportMenuOpen((v) => !v)}
              title={t('designCanvas.panel.exportTitle')}
            >
              <Download size={14} />
              <ChevronDown size={12} />
            </button>
            {isExportMenuOpen && (
              <ul className="design-canvas-panel__export-menu">
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setIsExportMenuOpen(false);
                      handleDownloadEntryHtml();
                    }}
                  >
                    <FileText size={13} /> {t('designCanvas.panel.exportEntryHtml')}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setIsExportMenuOpen(false);
                      handleZipExport();
                    }}
                  >
                    <FileArchive size={13} /> {t('designCanvas.panel.exportZip')}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setIsExportMenuOpen(false);
                      handleScreenshot();
                    }}
                  >
                    <Camera size={13} /> {t('designCanvas.panel.exportScreenshotPng')}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setIsExportMenuOpen(false);
                      handleSkillExport('pdf');
                    }}
                  >
                    <FileText size={13} /> {t('designCanvas.panel.exportPdf')}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setIsExportMenuOpen(false);
                      handleSkillExport('pptx');
                    }}
                  >
                    <FileText size={13} /> {t('designCanvas.panel.exportPptx')}
                  </button>
                </li>
              </ul>
            )}
          </div>
          <button
            type="button"
            className="design-canvas-panel__action-btn"
            onClick={handleOpenExternal}
            title={t('designCanvas.panel.openExternalTitle')}
          >
            <ExternalLink size={14} />
          </button>
        </div>
      </div>

      <div className={`design-canvas-panel__body design-canvas-panel__body--${viewMode}`}>
        {viewMode === 'preview' && <div className="design-canvas-panel__pane">{preview}</div>}
        {viewMode === 'code' && <div className="design-canvas-panel__pane">{codeView}</div>}
        {viewMode === 'split' && (
          <>
            <div className="design-canvas-panel__pane design-canvas-panel__pane--code">
              {codeView}
            </div>
            <div className="design-canvas-panel__pane design-canvas-panel__pane--preview">
              {preview}
            </div>
          </>
        )}
        {viewMode === 'diff' && <div className="design-canvas-panel__pane">{diffView}</div>}
        {viewMode === 'history' && <div className="design-canvas-panel__pane">{historyView}</div>}

        {isInspectorOpen && (
          <DesignInspector
            manifest={manifest}
            selectedElement={selectedElement}
            tokens={extractedTokens}
            onOpenFile={(path) => {
              setActiveFile(path);
              ensureFileLoaded(path);
              if (viewMode === 'preview') setViewMode('split');
            }}
            onCopyContext={handleCopyContext}
          />
        )}
      </div>

      {selectedElement?.domPath && !isInspectorOpen && (
        <div className="design-canvas-panel__inspector">
          <div className="design-canvas-panel__inspector-label">{t('designCanvas.panel.selectedLabel')}</div>
          <code className="design-canvas-panel__inspector-path">
            {selectedElement.domPath}
          </code>
          {selectedElement.textExcerpt && (
            <span className="design-canvas-panel__inspector-text">
              “{selectedElement.textExcerpt}”
            </span>
          )}
        </div>
      )}
    </div>
  );
};

// -------- Internal: lazy-load two artifact files for Diff --------

interface DesignDiffLoaderProps {
  fromPath: string;
  toPath: string;
  fromLabel: string;
  toLabel: string;
}

const DesignDiffLoader: React.FC<DesignDiffLoaderProps> = ({
  fromPath,
  toPath,
  fromLabel,
  toLabel,
}) => {
  const { t } = useTranslation('flow-chat');
  const [original, setOriginal] = useState('');
  const [modified, setModified] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all([
      fromPath ? workspaceAPI.readFileContent(fromPath).catch(() => '') : Promise.resolve(''),
      toPath ? workspaceAPI.readFileContent(toPath).catch(() => '') : Promise.resolve(''),
    ])
      .then(([from, to]) => {
        if (cancelled) return;
        setOriginal(from ?? '');
        setModified(to ?? '');
      })
      .catch((err) => {
        if (cancelled) return;
        setError(String(err?.message || err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [fromPath, toPath]);

  if (loading) {
    return (
      <div className="design-canvas-panel__loading">
        <Loader2 size={16} className="spin" /> {t('designCanvas.panel.diffLoading')}
      </div>
    );
  }
  if (error) {
    return (
      <div className="design-canvas-panel__history-empty">
        {t('designCanvas.panel.diffLoadFailed', { message: error })}
      </div>
    );
  }

  return (
    <div className="design-canvas-panel__diff-editor">
      <div className="design-canvas-panel__diff-caption">
        <span>{fromLabel}</span>
        <span>→</span>
        <span>{toLabel}</span>
      </div>
      <DiffEditor
        originalContent={original}
        modifiedContent={modified}
        readOnly
        renderSideBySide
      />
    </div>
  );
};

export default DesignCanvasPanel;
