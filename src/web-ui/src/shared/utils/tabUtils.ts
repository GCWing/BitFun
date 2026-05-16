 

import { i18nService } from '@/infrastructure/i18n';
import { fileTabManager } from '@/shared/services/FileTabManager';
import type { FileTabOptions } from '@/shared/services/FileTabManager';
import { enqueuePendingTab } from '@/shared/services/pendingTabQueue';
import { resolveAndFocusOpenTarget } from '@/shared/services/sceneOpenTargetResolver';
import type { OpenSource } from '@/shared/services/sceneOpenTargetResolver';
import { TAB_EVENTS } from '@/app/components/panels/content-canvas/types';
export type TabTargetMode = 'agent' | 'project' | 'git';

export interface TabCreationOptions {
  type: string;
  title: string;
  data: any;
  metadata?: Record<string, any>;
  checkDuplicate?: boolean;
  duplicateCheckKey?: string;
  replaceExisting?: boolean;
  /** Target canvas: agent (AuxPane), project (FileViewer), git (Git scene diff area) */
  mode?: TabTargetMode;
}

interface CreateTerminalTabOptions {
  sceneJustOpened?: boolean;
}

function isRightPanelCollapsed(): boolean {
  try {
    const layoutState = (window as any).__BITFUN_LAYOUT_STATE__;
    return layoutState?.rightPanelCollapsed ?? false;
  } catch {
    return false;
  }
}

 
export function createTab(options: TabCreationOptions): void {
  const {
    type,
    title,
    data,
    metadata = {},
    checkDuplicate = false,
    duplicateCheckKey,
    replaceExisting = false,
    mode = 'agent' 
  } = options;

  const eventName =
    mode === 'project' ? 'project-create-tab' : mode === 'git' ? 'git-create-tab' : 'agent-create-tab';

  const createTabEvent = new CustomEvent(eventName, {
    detail: {
      type,
      title,
      data,
      metadata,
      checkDuplicate,
      duplicateCheckKey,
      replaceExisting
    }
  });

  window.dispatchEvent(createTabEvent);
}

 
export function createFileViewerTab(
  filePath: string, 
  fileName: string, 
  content: string,
  mode: 'agent' | 'project' = 'project'
): void {
  createTab({
    type: 'file-viewer',
    title: fileName,
    data: content,
    metadata: { filePath, fileName },
    checkDuplicate: true,
    duplicateCheckKey: filePath,
    replaceExisting: false,
    mode
  });
}

 
export function createCodeEditorTab(
  filePath: string,
  fileName: string,
  options?: {
    language?: string;
    readOnly?: boolean;
    showLineNumbers?: boolean;
    showMinimap?: boolean;
    theme?: 'vs-dark' | 'vs-light' | 'hc-black';
    jumpToLine?: number;
    jumpToColumn?: number;
  },
  mode: 'agent' | 'project' = 'agent'
): void {
  createTab({
    type: 'code-editor',
    title: fileName,
    data: {
      filePath,
      fileName,
      language: options?.language,
      readOnly: options?.readOnly ?? false,
      showLineNumbers: options?.showLineNumbers ?? true,
      showMinimap: options?.showMinimap ?? true,
      theme: options?.theme ?? 'vs-dark',
      jumpToLine: options?.jumpToLine,
      jumpToColumn: options?.jumpToColumn
    },
    metadata: { filePath, fileName },
    checkDuplicate: true,
    duplicateCheckKey: `code-editor:${filePath}`,
    replaceExisting: true,
    mode
  });
}

export function createDiffEditorTab(
  filePath: string,
  fileName: string,
  originalCode: string,
  modifiedCode: string,
  readOnly: boolean = false,
  mode: TabTargetMode = 'agent',
  repositoryPath?: string,
  revealLine?: number,
  replaceExisting?: boolean,
  options?: {
    titleKind?: 'git-diff' | 'diff' | 'fix-preview';
    duplicateKeyPrefix?: 'git-diff' | 'diff' | 'fix-diff';
  }
): void {
  const titleKind = options?.titleKind ?? (repositoryPath ? 'git-diff' : 'fix-preview');
  const duplicateKeyPrefix = options?.duplicateKeyPrefix ?? (repositoryPath ? 'git-diff' : 'fix-diff');
  const duplicateKey = repositoryPath
    ? `${duplicateKeyPrefix}:${repositoryPath}:${filePath}`
    : `${duplicateKeyPrefix}:${filePath}`;
  const titleSuffix =
    titleKind === 'git-diff'
      ? i18nService.getT()('common:tabs.gitDiff')
      : titleKind === 'diff'
        ? i18nService.getT()('common:tabs.diff')
        : i18nService.getT()('common:tabs.fixPreview');

  createTab({
    type: 'diff-code-editor',
    title: `${fileName} - ${titleSuffix}`,
    data: {
      fileName,
      filePath,
      language: 'typescript',
      originalCode,
      modifiedCode,
      readOnly,
      repositoryPath,
      revealLine,
    },
    metadata: { filePath, repositoryPath, duplicateCheckKey: duplicateKey },
    checkDuplicate: true,
    duplicateCheckKey: duplicateKey,
    replaceExisting: replaceExisting ?? false,
    mode,
  });
}

/**
 * Open a Git diff tab in the Git scene canvas (mode 'git').
 * Use from Git scene only; keeps diff editing inside the Git context.
 */
export function createGitDiffEditorTab(
  filePath: string,
  fileName: string,
  originalCode: string,
  modifiedCode: string,
  repositoryPath: string,
  readOnly: boolean = false,
  replaceExisting?: boolean
): void {
  createDiffEditorTab(
    filePath,
    fileName,
    originalCode,
    modifiedCode,
    readOnly,
    'git',
    repositoryPath,
    undefined,
    replaceExisting
  );
}

/**
 * Open a code editor tab in the Git scene canvas (e.g. for untracked files).
 */
export function createGitCodeEditorTab(
  filePath: string,
  fileName: string,
  options?: Parameters<typeof createCodeEditorTab>[2]
): void {
  createTab({
    type: 'code-editor',
    title: fileName,
    data: {
      filePath,
      fileName,
      language: options?.language,
      readOnly: options?.readOnly ?? false,
      showLineNumbers: options?.showLineNumbers ?? true,
      showMinimap: options?.showMinimap ?? true,
      theme: options?.theme ?? 'vs-dark',
      jumpToLine: options?.jumpToLine,
      jumpToColumn: options?.jumpToColumn,
    },
    metadata: { filePath, fileName },
    checkDuplicate: true,
    duplicateCheckKey: `code-editor:${filePath}`,
    replaceExisting: true,
    mode: 'git',
  });
}

 
export function createMarkdownEditorTab(
  title: string,
  initialContent: string,
  filePath?: string,
  workspacePath?: string,
  mode: 'agent' | 'project' = 'agent'
): void {
  const timestamp = Date.now();
  const duplicateKey = filePath || `markdown-editor-${timestamp}`;
  
  createTab({
    type: 'markdown-editor',
    title,
    data: {
      initialContent,
      filePath,
      fileName: title,
      workspacePath,
      readOnly: false
    },
    metadata: {
      duplicateCheckKey: duplicateKey,
      timestamp
    },
    checkDuplicate: !filePath, 
    duplicateCheckKey: duplicateKey,
    replaceExisting: false,
    mode
  });
}

 
export function createConfigCenterTab(
  _initialTab: 'models' | 'agents' = 'models',
  _mode: 'agent' | 'project' = 'agent'
): void {
  // Settings is now an independent scene — open via event bus.
  window.dispatchEvent(new CustomEvent('scene:open', { detail: { sceneId: 'settings' } }));
}

export function createTerminalTab(
  sessionId: string,
  sessionName: string,
  mode: 'agent' | 'project' = 'agent',
  options: CreateTerminalTabOptions = {}
): void {
  const title = sessionName.length > 20 
    ? `${sessionName.slice(0, 20)}...` 
    : sessionName;

  const detail = {
    type: 'terminal',
    title: `${title}`,
    data: { sessionId, sessionName },
    metadata: {
      isTerminal: true,
      sessionId,
      duplicateCheckKey: `terminal-${sessionId}`,
    },
    checkDuplicate: true,
    duplicateCheckKey: `terminal-${sessionId}`,
    replaceExisting: false,
  };

  if (mode === 'agent') {
    window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_RIGHT_PANEL));

    if (options.sceneJustOpened) {
      enqueuePendingTab('agent', detail);
      return;
    }

    if (isRightPanelCollapsed()) {
      window.setTimeout(() => {
        window.dispatchEvent(new CustomEvent(TAB_EVENTS.AGENT_CREATE_TAB, { detail }));
      }, 300);
      return;
    }

    window.dispatchEvent(new CustomEvent(TAB_EVENTS.AGENT_CREATE_TAB, { detail }));
    return;
  }

  createTab({
    ...detail,
    mode,
  });
}

type OpenFileInBestTargetOptions = Omit<FileTabOptions, 'mode'>;
interface OpenFileTargetContext {
  source?: OpenSource;
}

/**
 * Open a file to the best target:
 * - active scene is session: open in agent AuxPane tabs
 * - otherwise: open in file-viewer scene project tabs
 *
 * This avoids unexpected focus stealing when session is merely opened but
 * not the currently active scene.
 */
export function openFileInBestTarget(
  options: OpenFileInBestTargetOptions,
  context: OpenFileTargetContext = {}
): void {
  const { mode, sceneJustOpened } = resolveAndFocusOpenTarget('file', { source: context.source ?? 'default' });

  fileTabManager.openFile({
    ...options,
    mode,
    sceneJustOpened,
  });
}
