/**
 * Editor feature exports
 */

export type {
  EditorConfig,
  EditorConfigPartial,
  EditorPresetName,
  EditorPresetConfig,
  MinimapConfig,
  GuidesConfig,
  ScrollbarConfig,
  HoverConfig,
  SuggestConfig,
  QuickSuggestionsConfig,
  InlayHintsConfig,
} from './config';

export {
  DEFAULT_EDITOR_CONFIG,
  EDITOR_PRESETS,
  getPreset,
  getPresetNames,
  mergeConfig,
  getFullConfig,
} from './config';

export {
  monacoInitManager,
  MonacoManager,
  monacoModelManager,
  monacoGlobalManager,
  themeManager,
  editorExtensionManager,
  buildEditorOptions,
  buildDiffEditorOptions,
} from './services';

export type {
  ThemeChangeEvent,
  ModelMetadata,
  ModelCreatedEvent,
  ModelContentChangedEvent,
  ModelDisposedEvent,
  ModelContentReadyEvent,
  EditorExtension,
  EditorExtensionContext,
  EditorOptionsInput,
  EditorOptionsOverrides,
} from './services';

export { EditorManager, editorManager } from './services/EditorManager';
export { editorReadyManager } from './services/EditorReadyManager';
export { default as MonacoInitManager } from './services/MonacoInitManager';

export {
  MonacoEditorCore,
  MonacoDiffCore,
} from './core';

export type {
  MonacoEditorCoreProps,
  MonacoDiffCoreProps,
  MonacoEditorCoreRef,
  MonacoDiffCoreRef,
} from './core';

export {
  ExtensionPriority,
  createLspExtension,
  lspExtension,
} from './extensions';

export type {
  LspExtensionConfig,
  AiCompletionExtensionConfig,
  TabCompletionExtensionConfig,
} from './extensions';

export {
  useEditorConfig,
  useEditorTheme,
  useEditorOptions,
  useSimpleEditorOptions,
  useDiffEditorOptions,
} from './hooks';

export { useLspInitialization } from './hooks/useLspIntegration';

export { default as CodeEditor } from './components/CodeEditor';
export type { CodeEditorProps } from './components/CodeEditor';

export { DiffEditor } from './components/DiffEditor';
export type { DiffEditorProps } from './components/DiffEditor';

export { ReadOnlyCodeBlock } from './components/ReadOnlyCodeBlock';
export type { ReadOnlyCodeBlockProps } from './components/ReadOnlyCodeBlock';

export { default as MarkdownEditor } from './components/MarkdownEditor';
export type { MarkdownEditorProps } from './components/MarkdownEditor';

export { default as ImageViewer } from './components/ImageViewer';
export type { ImageViewerProps } from './components/ImageViewer';

export { default as PlanViewer } from './components/PlanViewer';
export type { PlanViewerProps } from './components/PlanViewer';

export { EditorBreadcrumb } from './components/EditorBreadcrumb';
export type { EditorBreadcrumbProps } from './components/EditorBreadcrumb';
export { EditorStatusBar } from './components/EditorStatusBar';
export type { EditorStatusBarProps } from './components/EditorStatusBar';

export { BitFunDarkTheme, BitFunDarkThemeMetadata } from './themes/bitfun-dark.theme';

import { createLogger } from '@/shared/utils/logger';

export async function initializeEditorFeature(): Promise<void> {
  const log = createLogger('Editor');
  
  try {
    const { monacoInitManager } = await import('./services/MonacoInitManager');
    await monacoInitManager.initialize();
    
    const { editorExtensionManager } = await import('./services/EditorExtensionManager');
    const { lspExtension } = await import('./extensions/LspExtension');
    editorExtensionManager.register(lspExtension);
  } catch (error) {
    log.error('Failed to initialize editor feature', error);
    throw error;
  }
}

export const EditorFeatureMetadata = {
  name: 'Editor',
  version: '2.0.0',
  description: 'Code and markdown editing with Monaco and LSP support',
  dependencies: ['core'],
  capabilities: [
    'code-editing',
    'markdown-editing',
    'diff-editing',
    'syntax-highlighting',
    'code-completion',
    'lsp-integration',
    'multi-file-editing',
    'search-replace',
    'format-document',
    'readonly-code-block',
    'extensible-plugins',
  ]
} as const;
