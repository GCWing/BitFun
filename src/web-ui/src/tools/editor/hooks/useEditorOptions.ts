/**
 * Editor Options Hook
 * 
 * Responsibilities:
 * - Call EditorOptionsBuilder
 * - Return Monaco options object
 * - Handle option changes
 * @module useEditorOptions
 */

import { useMemo, useCallback } from 'react';
import type * as monaco from 'monaco-editor';
import type { EditorConfig, EditorConfigPartial, EditorPresetName } from '../config/types';
import {
  buildEditorOptions,
  buildDiffEditorOptions,
  buildUpdateOptions,
  type EditorOptionsInput,
  type EditorOptionsOverrides,
} from '../services/EditorOptionsBuilder';

interface UseEditorOptionsInput {
  /** Base configuration */
  config?: EditorConfig | EditorConfigPartial;
  /** Preset name */
  preset?: EditorPresetName;
  /** Runtime overrides */
  overrides?: EditorOptionsOverrides;
}

interface UseEditorOptionsResult {
  /** Editor creation options */
  editorOptions: monaco.editor.IStandaloneEditorConstructionOptions;
  /** Diff editor creation options */
  diffEditorOptions: monaco.editor.IStandaloneDiffEditorConstructionOptions;
  /** Get update options */
  getUpdateOptions: (configChanges: EditorConfigPartial) => monaco.editor.IEditorOptions;
}

/** Editor Options Hook */
export function useEditorOptions(input: UseEditorOptionsInput = {}): UseEditorOptionsResult {
  const { config, preset, overrides } = input;
  
  /** Build editor creation options */
  const editorOptions = useMemo(() => {
    const buildInput: EditorOptionsInput = {
      config: config as EditorConfigPartial,
      preset,
      overrides,
    };
    
    return buildEditorOptions(buildInput);
  }, [config, preset, overrides]);
  
  /** Build Diff editor creation options */
  const diffEditorOptions = useMemo(() => {
    const buildInput: EditorOptionsInput = {
      config: config as EditorConfigPartial,
      preset: preset || 'diff',
      overrides,
    };
    
    return buildDiffEditorOptions(buildInput);
  }, [config, preset, overrides]);
  
  /** Get update options for config changes */
  const getUpdateOptions = useCallback((configChanges: EditorConfigPartial) => {
    return buildUpdateOptions(configChanges);
  }, []);
  
  return {
    editorOptions,
    diffEditorOptions,
    getUpdateOptions,
  };
}

/** Simplified hook for standard editor */
export function useSimpleEditorOptions(
  preset: EditorPresetName = 'standard',
  overrides?: EditorOptionsOverrides
): monaco.editor.IStandaloneEditorConstructionOptions {
  return useMemo(() => {
    return buildEditorOptions({
      preset,
      overrides,
    });
  }, [preset, overrides]);
}

/** Simplified hook for Diff editor */
export function useDiffEditorOptions(
  overrides?: EditorOptionsOverrides
): monaco.editor.IStandaloneDiffEditorConstructionOptions {
  return useMemo(() => {
    return buildDiffEditorOptions({
      preset: 'diff',
      overrides,
    });
  }, [overrides]);
}

export default useEditorOptions;
