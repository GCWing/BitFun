/**
 * Editor core layer - base editor components
 */

export type {
  MonacoEditorCoreProps,
  MonacoDiffCoreProps,
  EditorReadyState,
  DiffEditorReadyState,
} from './types';

export { MonacoEditorCore, type MonacoEditorCoreRef } from './MonacoEditorCore';
export { MonacoDiffCore, type MonacoDiffCoreRef } from './MonacoDiffCore';
export { default as MonacoEditorCoreDefault } from './MonacoEditorCore';
export { default as MonacoDiffCoreDefault } from './MonacoDiffCore';
