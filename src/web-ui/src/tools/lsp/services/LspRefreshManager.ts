/**
 * Centralized refresh scheduler for LSP-related Monaco features.
 *
 * Keeps refreshes debounced/throttled to avoid redundant work.
 */

import * as monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('LspRefreshManager');

interface RefreshTimers {
  semanticTokens?: NodeJS.Timeout;
  inlayHints?: NodeJS.Timeout;
  diagnostics?: NodeJS.Timeout;
}

interface RefreshConfig {
  /** didChange debounce delay (ms). */
  didChange: number;
  /** Extra delay for semantic tokens after didChange (ms). */
  semanticTokens: number;
  /** Inlay hints refresh delay (ms). */
  inlayHints: number;
  /** Diagnostics refresh delay (ms). */
  diagnostics: number;
}

/** Default refresh timings. */
const DEFAULT_REFRESH_CONFIG: RefreshConfig = {
  didChange: 300,
  semanticTokens: 200,
  inlayHints: 400,
  diagnostics: 100,
};

/** LSP refresh manager (singleton). */
export class LspRefreshManager {
  private static instance: LspRefreshManager;
  
  /** Refresh timers per document URI. */
  private refreshTimers = new Map<string, RefreshTimers>();
  
  /** Refresh config. */
  private config: RefreshConfig;
  
  /** Cancelled request IDs (used to skip stale work). */
  private cancelledRequests = new Set<string>();
  
  private constructor(config?: Partial<RefreshConfig>) {
    this.config = { ...DEFAULT_REFRESH_CONFIG, ...config };
  }
  
  public static getInstance(config?: Partial<RefreshConfig>): LspRefreshManager {
    if (!LspRefreshManager.instance) {
      LspRefreshManager.instance = new LspRefreshManager(config);
    }
    return LspRefreshManager.instance;
  }
  
  /**
   * Main entrypoint to be called after document content changes.
   */
  public onDocumentChange(
    uri: string,
    editor: monaco.editor.IStandaloneCodeEditor | null,
    options?: {
      /** Refresh semantic tokens. */
      refreshSemanticTokens?: boolean;
      /** Refresh inlay hints. */
      refreshInlayHints?: boolean;
      /** Refresh diagnostics. */
      refreshDiagnostics?: boolean;
    }
  ): void {
    const opts = {
      refreshSemanticTokens: true,
      refreshInlayHints: true,
      // Diagnostics are typically pushed by the LSP server.
      refreshDiagnostics: false,
      ...options
    };
    
    if (!editor) {
      return;
    }
    
    const timers = this.refreshTimers.get(uri) || {};
    
    if (opts.refreshSemanticTokens) {
      if (timers.semanticTokens) {
        clearTimeout(timers.semanticTokens);
      }
      
      timers.semanticTokens = setTimeout(() => {
        this.refreshSemanticTokens(editor);
        delete timers.semanticTokens;
      }, this.config.semanticTokens);
    }
    
    if (opts.refreshInlayHints) {
      if (timers.inlayHints) {
        clearTimeout(timers.inlayHints);
      }
      
      timers.inlayHints = setTimeout(() => {
        this.refreshInlayHints(editor);
        delete timers.inlayHints;
      }, this.config.inlayHints);
    }
    
    if (opts.refreshDiagnostics) {
      if (timers.diagnostics) {
        clearTimeout(timers.diagnostics);
      }
      
      timers.diagnostics = setTimeout(() => {
        this.refreshDiagnostics(editor);
        delete timers.diagnostics;
      }, this.config.diagnostics);
    }
    
    this.refreshTimers.set(uri, timers);
  }
  
  /**
   * Refresh semantic tokens.
   *
   * Uses a few internal fallbacks because Monaco doesn't expose a stable public API
   * to flush semantic token caches in all versions.
   */
  private refreshSemanticTokens(editor: monaco.editor.IStandaloneCodeEditor): void {
    try {
      const model = editor.getModel();
      if (!model) {
        return;
      }
      
      const editorAny = editor as any;
      const modelAny = model as any;
      
      if (editorAny._modelData?.model?.tokenization) {
        try {
          const tokenization = editorAny._modelData.model.tokenization;
          if (tokenization.flushTokens) {
            tokenization.flushTokens();
            return;
          }
        } catch (_error) {
          // silent
        }
      }
      
      if (modelAny._tokenization?.resetTokenization) {
        modelAny._tokenization.resetTokenization();
        return;
      }
      
      if (modelAny._resetTokenization) {
        modelAny._resetTokenization();
        return;
      }
      
    } catch (error) {
      log.error('Failed to refresh semantic tokens', { error });
    }
  }
  
  private refreshInlayHints(editor: monaco.editor.IStandaloneCodeEditor): void {
    try {
      const action = editor.getAction('editor.action.inlayHints.refresh');
      if (action) {
        action.run();
      }
    } catch (error) {
      log.error('Failed to refresh inlay hints', { error });
    }
  }
  
  private refreshDiagnostics(editor: monaco.editor.IStandaloneCodeEditor): void {
    try {
      const model = editor.getModel();
      if (!model) return;
      
      const monaco = (window as any).monaco;
      if (monaco?.editor?.setModelMarkers) {
        const uri = model.uri;
        const currentMarkers = monaco.editor.getModelMarkers({ resource: uri });
        monaco.editor.setModelMarkers(model, 'lsp', currentMarkers);
      }
    } catch (_error) {
      // silent
    }
  }
  
  /** Refresh semantic tokens and inlay hints immediately (no debounce). */
  public forceRefresh(
    _uri: string,
    editor: monaco.editor.IStandaloneCodeEditor | null
  ): void {
    if (!editor) return;
    
    this.refreshSemanticTokens(editor);
    this.refreshInlayHints(editor);
  }
  
  /** Cancel all scheduled refresh work for a document. */
  public cancelRefresh(documentUri: string): void {
    const timers = this.refreshTimers.get(documentUri);
    if (!timers) return;
    
    if (timers.semanticTokens) {
      clearTimeout(timers.semanticTokens);
      delete timers.semanticTokens;
    }
    
    if (timers.inlayHints) {
      clearTimeout(timers.inlayHints);
      delete timers.inlayHints;
    }
    
    if (timers.diagnostics) {
      clearTimeout(timers.diagnostics);
      delete timers.diagnostics;
    }
    
    this.refreshTimers.delete(documentUri);
  }
  
  public dispose(): void {
    for (const [_uri, timers] of this.refreshTimers.entries()) {
      if (timers.semanticTokens) clearTimeout(timers.semanticTokens);
      if (timers.inlayHints) clearTimeout(timers.inlayHints);
      if (timers.diagnostics) clearTimeout(timers.diagnostics);
    }
    
    this.refreshTimers.clear();
    this.cancelledRequests.clear();
  }
  
  public updateConfig(config: Partial<RefreshConfig>): void {
    this.config = { ...this.config, ...config };
  }
  
  public getConfig(): Readonly<RefreshConfig> {
    return { ...this.config };
  }
  
  public getDebugInfo(): {
    activeTimers: number;
    timersByUri: Record<string, string[]>;
    config: RefreshConfig;
  } {
    const timersByUri: Record<string, string[]> = {};
    let activeTimers = 0;
    
    for (const [uri, timers] of this.refreshTimers.entries()) {
      const active: string[] = [];
      if (timers.semanticTokens) active.push('semanticTokens');
      if (timers.inlayHints) active.push('inlayHints');
      if (timers.diagnostics) active.push('diagnostics');
      
      if (active.length > 0) {
        timersByUri[uri] = active;
        activeTimers += active.length;
      }
    }
    
    return {
      activeTimers,
      timersByUri,
      config: this.getConfig()
    };
  }
}

// Singleton export
export const lspRefreshManager = LspRefreshManager.getInstance();

// Global debug helper
if (typeof window !== 'undefined') {
  (window as any).lspRefreshManager = lspRefreshManager;
}
