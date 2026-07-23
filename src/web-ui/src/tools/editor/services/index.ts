/**
 * Editor services
 */

export { monacoInitManager, MonacoManager } from './MonacoInitManager';
export { default as MonacoInitManager } from './MonacoInitManager';

export {
  monacoModelManager,
  monacoGlobalManager,
  MonacoGlobalManager,
  type ModelMetadata,
  type ModelCreatedEvent,
  type ModelContentChangedEvent,
  type ModelDisposedEvent,
  type ModelContentReadyEvent,
} from './MonacoModelManager';
export { default as MonacoModelManager } from './MonacoModelManager';

export { themeManager, type ThemeChangeEvent } from './ThemeManager';
export { default as ThemeManager } from './ThemeManager';

export {
  buildEditorOptions,
  buildDiffEditorOptions,
  buildUpdateOptions,
  type EditorOptionsInput,
  type EditorOptionsOverrides,
} from './EditorOptionsBuilder';

export {
  editorExtensionManager,
  type EditorExtension,
  type EditorExtensionContext,
} from './EditorExtensionManager';
export { default as EditorExtensionManager } from './EditorExtensionManager';

export * from './EditorReadyManager';

export {
  DiffService,
  diffService,
  computeDiff,
  computeCharDiff,
  acceptHunk,
  rejectHunk,
  getDiffStats,
  type DiffOptions,
  type DiffLine,
  type CharDiffSegment,
  type CharDiffResult,
  type DiffHunkData,
  type DiffStats,
  type DiffComputeResult,
} from './DiffService';

export {
  activeEditTargetService,
  createMonacoEditTarget,
  type EditTarget,
  type EditMenuAction,
  type EditTargetKind,
  type MacosEditMenuMode,
} from './ActiveEditTargetService';
