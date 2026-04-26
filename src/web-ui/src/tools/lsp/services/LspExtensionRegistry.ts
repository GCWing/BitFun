/**
 * Registry of file extensions and languages supported by installed LSP plugins.
 *
 * Loaded once and cached to avoid querying the backend on every file open.
 */

import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';
import { measureAsync } from '@/shared/utils/timing';

const log = createLogger('LspExtensionRegistry');

export interface SupportedExtensionsResponse {
  /** Extension-to-language map (e.g. ".ts" -> "typescript"). */
  extensionToLanguage: Record<string, string>;
  /** All supported language IDs. */
  supportedLanguages: string[];
}

class LspExtensionRegistry {
  private static instance: LspExtensionRegistry;
  private extensionToLanguage: Map<string, string> = new Map();
  private supportedLanguages: Set<string> = new Set();
  private initialized = false;
  private initializing = false;
  private initPromise: Promise<void> | null = null;

  private constructor() {}

  static getInstance(): LspExtensionRegistry {
    if (!LspExtensionRegistry.instance) {
      LspExtensionRegistry.instance = new LspExtensionRegistry();
    }
    return LspExtensionRegistry.instance;
  }

  /** Initialize from backend once. */
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }

    if (this.initializing && this.initPromise) {
      return this.initPromise;
    }

    this.initializing = true;
    this.initPromise = this._doInitialize();
    
    try {
      await this.initPromise;
    } finally {
      this.initializing = false;
      this.initPromise = null;
    }
  }

  private async _doInitialize(): Promise<void> {
    try {
      const result = await measureAsync(() =>
        invoke<SupportedExtensionsResponse>('lsp_get_supported_extensions')
      );
      const response = result.value;
      
      this.extensionToLanguage.clear();
      this.supportedLanguages.clear();

      for (const [ext, lang] of Object.entries(response.extensionToLanguage)) {
        const normalizedExt = ext.startsWith('.') ? ext : `.${ext}`;
        this.extensionToLanguage.set(normalizedExt.toLowerCase(), lang);
      }

      for (const lang of response.supportedLanguages) {
        this.supportedLanguages.add(lang.toLowerCase());
      }

      this.initialized = true;
      log.debug('Initialized successfully', { 
        durationMs: result.durationMs,
        extensionCount: this.extensionToLanguage.size,
        languageCount: this.supportedLanguages.size
      });
    } catch (error) {
      log.error('Failed to initialize', { error });
      this.initialized = true;
    }
  }

  /** Whether a file path is supported by any installed LSP plugin. */
  isFileSupported(filePath: string): boolean {
    if (!this.initialized) {
      return false;
    }

    const ext = this.getFileExtension(filePath);
    if (!ext) {
      return false;
    }

    return this.extensionToLanguage.has(ext);
  }

  /** Whether a language ID is supported by any installed LSP plugin. */
  isLanguageSupported(language: string): boolean {
    if (!this.initialized) {
      return false;
    }

    return this.supportedLanguages.has(language.toLowerCase());
  }

  /** Map a file path to a language ID (or null if not supported). */
  getLanguageByFilePath(filePath: string): string | null {
    if (!this.initialized) {
      return null;
    }

    const ext = this.getFileExtension(filePath);
    if (!ext) {
      return null;
    }

    return this.extensionToLanguage.get(ext) || null;
  }

  /** Get all supported extensions. */
  getSupportedExtensions(): string[] {
    return Array.from(this.extensionToLanguage.keys());
  }

  /** Get all supported languages. */
  getSupportedLanguages(): string[] {
    return Array.from(this.supportedLanguages);
  }

  /** Extract a lowercase extension with dot (e.g. ".ts"). */
  private getFileExtension(filePath: string): string | null {
    const lastDot = filePath.lastIndexOf('.');
    const lastSlash = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'));
    
    // No extension, or dot is in a folder segment.
    if (lastDot === -1 || lastDot < lastSlash) {
      return null;
    }

    return filePath.substring(lastDot).toLowerCase();
  }

  /** Reload (e.g. after installing a new plugin). */
  async reload(): Promise<void> {
    this.initialized = false;
    await this.initialize();
  }

  isInitialized(): boolean {
    return this.initialized;
  }
}

// Singleton export
export const lspExtensionRegistry = LspExtensionRegistry.getInstance();

