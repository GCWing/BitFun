/**
 * LSP Extension
 * 
 * Encapsulates LSP functionality as an editor extension.
 * Auto-registers LSP adapter when editor is created.
 * @module LspExtension
 */

import type * as monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';
import type { EditorExtension, EditorExtensionContext } from '../services/EditorExtensionManager';
import { ExtensionPriority, type LspExtensionConfig } from './types';

const log = createLogger('LspExtension');

/** Languages with Monaco built-in services - skip external LSP for these */
const MONACO_BUILTIN_LANGUAGES = [
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

/** Create LSP extension instance */
export function createLspExtension(config: LspExtensionConfig = { enabled: true }): EditorExtension {
  const skipLanguages = new Set([
    ...MONACO_BUILTIN_LANGUAGES,
    ...(config.skipLanguages || []),
  ]);
  
  return {
    id: 'bitfun.lsp',
    name: 'LSP Extension',
    priority: ExtensionPriority.HIGH,
    
    onEditorCreated(
      editor: monaco.editor.IStandaloneCodeEditor,
      model: monaco.editor.ITextModel,
      context: EditorExtensionContext
    ): void {
      if (!config.enabled || !context.enableLsp) {
        return;
      }
      
      if (skipLanguages.has(context.language)) {
        return;
      }
      
      const workspacePath = context.workspacePath || config.workspacePath;
      if (!workspacePath) {
        log.warn('No workspace path provided, LSP will not be enabled');
        return;
      }
      
      void (async () => {
        try {
          // Dynamic import to avoid circular dependencies
          const { lspAdapterManager } = await import('@/tools/lsp/services/LspAdapterManager');
          const { lspExtensionRegistry } = await import('@/tools/lsp/services/LspExtensionRegistry');

          if (!lspExtensionRegistry.isInitialized()) {
            log.warn('LSP extension registry not initialized yet');
            return;
          }

          if (!lspExtensionRegistry.isLanguageSupported(context.language)) {
            return;
          }

          lspAdapterManager.getOrCreateAdapter(
            model,
            context.language,
            context.filePath,
            workspacePath
          );

          lspAdapterManager.registerEditor(model, editor);
        } catch (error) {
          log.error('Failed to initialize LSP', error);
        }
      })();
    },
    
    onEditorWillDispose(
      editor: monaco.editor.IStandaloneCodeEditor,
      model: monaco.editor.ITextModel,
      _context: EditorExtensionContext
    ): void {
      void import('@/tools/lsp/services/LspAdapterManager')
        .then(({ lspAdapterManager }) => {
          lspAdapterManager.unregisterEditor(model, editor);
        })
        .catch(error => {
          log.error('Failed to dispose LSP editor binding', error);
        });
    },
  };
}

/** Default LSP extension instance */
export const lspExtension = createLspExtension();

export default lspExtension;
