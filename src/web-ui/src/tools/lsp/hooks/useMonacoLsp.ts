/**
 * Monaco editor LSP hook.
 *
 * Design:
 * - One adapter per model (long-lived).
 * - Editors are registered/unregistered to the adapter (short-lived).
 * - Switching tabs should not destroy the adapter, so LSP state can persist.
 */

import { useEffect, useRef } from 'react';
import type * as monaco from 'monaco-editor';
import { lspAdapterManager } from '../services/LspAdapterManager';
import { lspExtensionRegistry } from '../services/LspExtensionRegistry';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useMonacoLsp');

export function useMonacoLsp(
  editor: monaco.editor.IStandaloneCodeEditor | null,
  language: string,
  filePath: string,
  enabled: boolean = true,
  workspacePath?: string
) {
  const isRegisteredRef = useRef(false);

  useEffect(() => {
    if (!editor || !enabled) {
      return;
    }

    const model = editor.getModel();
    if (!model) {
      return;
    }

    if (!workspacePath) {
      log.warn('No workspace path provided, LSP will not be enabled');
      return;
    }

    const monacoBuiltinLanguages = [
      'typescript',
      'javascript',
      'typescriptreact',
      'javascriptreact',
      'json',
      'html',
      'css',
      'scss',
      'less',
    ];
    if (monacoBuiltinLanguages.includes(language.toLowerCase())) {
      return;
    }

    if (!lspExtensionRegistry.isInitialized()) {
      log.warn('LSP extension registry not initialized yet');
      return;
    }

    if (!lspExtensionRegistry.isLanguageSupported(language)) {
      return;
    }

    lspAdapterManager.getOrCreateAdapter(
      model,
      language,
      filePath,
      workspacePath
    );
    
    lspAdapterManager.registerEditor(model, editor);
    isRegisteredRef.current = true;

    return () => {
      if (isRegisteredRef.current) {
        lspAdapterManager.unregisterEditor(model, editor);
        isRegisteredRef.current = false;
      }
    };
  }, [editor, language, filePath, enabled, workspacePath]);
}
