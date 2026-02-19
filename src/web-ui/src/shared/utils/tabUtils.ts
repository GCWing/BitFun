 

import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TabUtils');

export interface TabCreationOptions {
  type: string;
  title: string;
  data: any;
  metadata?: Record<string, any>;
  checkDuplicate?: boolean;
  duplicateCheckKey?: string;
  replaceExisting?: boolean;
  mode?: 'agent' | 'project'; 
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

  const eventName = mode === 'project' ? 'project-create-tab' : 'agent-create-tab';
  
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
  mode: 'agent' | 'project' = 'agent',
  repositoryPath?: string, 
  revealLine?: number, 
  replaceExisting?: boolean 
): void {
  
  const duplicateKey = repositoryPath 
    ? `git-diff:${repositoryPath}:${filePath}` 
    : `fix-diff:${filePath}`;
  
  createTab({
    type: 'diff-code-editor',
    title: `${fileName} - ${repositoryPath ? i18nService.getT()('common:tabs.gitDiff') : i18nService.getT()('common:tabs.fixPreview')}`,
    data: {
      fileName,
      filePath,
      language: 'typescript',
      originalCode,
      modifiedCode,
      readOnly,
      repositoryPath, 
      revealLine
    },
    metadata: { filePath, repositoryPath, duplicateCheckKey: duplicateKey },
    checkDuplicate: true,
    duplicateCheckKey: duplicateKey,
    replaceExisting: replaceExisting ?? false, 
    mode
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
  initialTab: 'models' | 'ai-rules' | 'agents' = 'models',
  mode: 'agent' | 'project' = 'agent'
): void {
  
  import('@/app/components/panels/content-canvas/stores/canvasStore').then(({ useCanvasStore }) => {
    const store = useCanvasStore.getState();

    
    const existingTab = store.findTabByMetadata({ isConfigCenter: true });

    if (existingTab) {
      
      const groupState = existingTab.groupId === 'primary' 
        ? store.primaryGroup 
        : existingTab.groupId === 'secondary' 
          ? store.secondaryGroup 
          : store.tertiaryGroup;
      
      const isActiveTab = groupState.activeTabId === existingTab.tab.id 
        && store.activeGroupId === existingTab.groupId;
      
      if (isActiveTab) {
        
        store.closeTab(existingTab.tab.id, existingTab.groupId);
      } else {
        
        store.switchToTab(existingTab.tab.id, existingTab.groupId);
      }
    } else {
      
      createTab({
        type: 'config-center',
        title: i18nService.getT()('common:tabs.configCenter'),
        data: { initialTab },
        metadata: { isConfigCenter: true },
        checkDuplicate: true,
        duplicateCheckKey: 'config-center',
        replaceExisting: false,
        mode
      });
    }

    
    
  });
}

 
export function createWorkflowEditorTab(
  workflowId?: string,
  workflowName?: string,
  mode: 'agent' | 'project' = 'agent'
): void {
  const title = workflowId
    ? `${workflowName || workflowId}`
    : i18nService.getT()('panels/workflows:editor.createTitle');
  const duplicateKey = workflowId
    ? `workflow-editor:${workflowId}`
    : `workflow-editor:new-${Date.now()}`;

  createTab({
    type: 'workflow-editor',
    title,
    data: { workflowId },
    metadata: { isWorkflowEditor: true, workflowId },
    checkDuplicate: !!workflowId,
    duplicateCheckKey: duplicateKey,
    replaceExisting: true,
    mode
  });
}

export function createTerminalTab(
  sessionId: string,
  sessionName: string,
  mode: 'agent' | 'project' = 'agent'
): void {
  const title = sessionName.length > 20 
    ? `${sessionName.slice(0, 20)}...` 
    : sessionName;
  
  createTab({
    type: 'terminal',
    title: `${title}`,
    data: { sessionId, sessionName },
    metadata: { 
      isTerminal: true,
      sessionId,
      duplicateCheckKey: `terminal-${sessionId}`
    },
    checkDuplicate: true,
    duplicateCheckKey: `terminal-${sessionId}`,
    replaceExisting: false,
    mode
  });
}