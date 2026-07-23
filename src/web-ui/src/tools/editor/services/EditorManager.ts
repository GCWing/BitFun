/** Manages multi-file editing, configuration, and editor state. */

import {
  IEditorManager,
  EditorConfig,
  FileContent,
  EditorEvent,
  SearchResult,
  SearchOptions,
  ReplaceOptions,
  LanguageDetectionResult
} from '../types';
import { globalEventBus } from '../../../infrastructure/event-bus';
import { getMonacoLanguage } from '@/infrastructure/language-detection';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('EditorManager');

/** Default editor config used when no user config exists. */
const DEFAULT_CONFIG: EditorConfig = {
  fontSize: 14,
  fontFamily: "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
  fontWeight: 'normal',
  lineHeight: 1.5,
  tabSize: 2,
  insertSpaces: true,
  wordWrap: 'off',
  lineNumbers: 'on',
  minimap: {
    enabled: true,
    side: 'right',
    size: 'proportional'
  },
  theme: 'vs-dark',
  cursorStyle: 'line',
  cursorBlinking: 'blink',
  renderWhitespace: 'selection',
  renderLineHighlight: 'all',
  autoSave: 'afterDelay',
  autoSaveDelay: 1000,
  scrollBeyondLastLine: false,
  smoothScrolling: true,
  formatOnSave: true,
  formatOnPaste: true,
  trimAutoWhitespace: true,
  semanticHighlighting: true,
  bracketPairColorization: true,
  guides: {
    indentation: true,
    bracketPairs: true,
    bracketPairsHorizontal: 'active',
    highlightActiveBracketPair: true,
    highlightActiveIndentation: true,
  },
  scrollbar: {
    vertical: 'auto',
    horizontal: 'auto',
    verticalScrollbarSize: 10,
    horizontalScrollbarSize: 10,
    useShadows: false,
  },
  hover: {
    enabled: true,
    delay: 100,
    sticky: true,
    above: false,
  },
  suggest: {
    showKeywords: true,
    showSnippets: true,
    preview: true,
    showInlineDetails: false,
  },
  quickSuggestions: {
    other: true,
    comments: false,
    strings: false,
  },
  inlayHints: {
    enabled: 'on',
    fontSize: 12,
    fontFamily: "'Fira Code', Consolas, 'Courier New', monospace",
    padding: false,
  },
};

export class EditorManager implements IEditorManager {
  private openFiles: FileContent[] = [];
  private activeFileIndex: number = -1;
  private config: EditorConfig;
  private listeners = new Set<(event: EditorEvent) => void>();
  private autoSaveTimers = new Map<number, ReturnType<typeof setTimeout>>();

  constructor(initialConfig?: Partial<EditorConfig>) {
    this.config = { ...DEFAULT_CONFIG, ...initialConfig };
    this.loadConfig();
  }

  async openFile(file: FileContent): Promise<number> {
    try {
      const existingIndex = this.openFiles.findIndex(f => f.name === file.name);
      if (existingIndex !== -1) {
        this.activeFileIndex = existingIndex;
        this.emitEvent({
          type: 'file:opened',
          payload: this.openFiles[existingIndex]
        });
        return existingIndex;
      }

      const detectedLanguage = this.detectLanguage(file.name, file.content);
      const fileWithLanguage: FileContent = {
        ...file,
        language: file.language || detectedLanguage.language,
        isDirty: false,
        lastModified: file.lastModified || new Date(),
        size: file.content.length
      };

      this.openFiles.push(fileWithLanguage);
      const newIndex = this.openFiles.length - 1;
      this.activeFileIndex = newIndex;

      this.setupAutoSave(newIndex);

      this.emitEvent({
        type: 'file:opened',
        payload: fileWithLanguage
      });

      return newIndex;
    } catch (error) {
      log.error('Failed to open file', error);
      throw error;
    }
  }

  async closeFile(index: number): Promise<void> {
    if (index < 0 || index >= this.openFiles.length) {
      return;
    }

    const file = this.openFiles[index];
    
    if (file.isDirty) {
      const shouldSave = await this.promptSave(file);
      if (shouldSave) {
        await this.saveFile(index);
      }
    }

    this.clearAutoSave(index);

    this.openFiles.splice(index, 1);

    if (this.activeFileIndex === index) {
      this.activeFileIndex = Math.min(this.activeFileIndex, this.openFiles.length - 1);
    } else if (this.activeFileIndex > index) {
      this.activeFileIndex--;
    }

    this.emitEvent({
      type: 'file:closed',
      payload: { index }
    });
  }

  async saveFile(index: number): Promise<void> {
    if (index < 0 || index >= this.openFiles.length) {
      return;
    }

    const file = this.openFiles[index];
    
    try {
      // TODO: Persist to workspace/file system (this manager currently only updates in-memory state).
      
      file.isDirty = false;
      file.lastModified = new Date();
      this.openFiles[index] = file;

      this.emitEvent({
        type: 'file:saved',
        payload: { index, content: file.content }
      });
    } catch (error) {
      log.error('Failed to save file', { fileName: file.name, error });
      throw error;
    }
  }

  async saveAllFiles(): Promise<void> {
    const savePromises = this.openFiles
      .map((file, index) => file.isDirty ? this.saveFile(index) : Promise.resolve())
      .filter(promise => promise !== Promise.resolve());

    await Promise.all(savePromises);
  }

  getActiveFile(): FileContent | null {
    return this.openFiles[this.activeFileIndex] || null;
  }

  getFile(index: number): FileContent | null {
    return this.openFiles[index] || null;
  }

  getAllFiles(): FileContent[] {
    return [...this.openFiles];
  }

  isFileDirty(index: number): boolean {
    const file = this.openFiles[index];
    return file ? file.isDirty || false : false;
  }

  updateFileContent(index: number, content: string): void {
    if (index < 0 || index >= this.openFiles.length) {
      return;
    }

    const file = this.openFiles[index];
    if (file.content !== content) {
      file.content = content;
      file.isDirty = true;
      file.size = content.length;
      this.openFiles[index] = file;

      this.resetAutoSave(index);

      this.emitEvent({
        type: 'file:changed',
        payload: { index, content }
      });
    }
  }

  search(query: string, options: SearchOptions = {}): SearchResult[] {
    const results: SearchResult[] = [];
    const searchRegex = this.createSearchRegex(query, options);

    const filesToSearch = options.fileIndex !== undefined 
      ? [this.openFiles[options.fileIndex]].filter(Boolean)
      : this.openFiles;

    filesToSearch.forEach((file, fileIndex) => {
      const actualFileIndex = options.fileIndex !== undefined ? options.fileIndex : fileIndex;
      const lines = file.content.split('\n');
      
      lines.forEach((line, lineIndex) => {
        let match;
        while ((match = searchRegex.exec(line)) !== null) {
          results.push({
            fileIndex: actualFileIndex,
            line: lineIndex + 1,
            column: match.index + 1,
            length: match[0].length,
            match: match[0],
            context: line
          });

          if (!searchRegex.global) break;
        }
      });
    });

    this.emitEvent({
      type: 'search:performed',
      payload: { query, results }
    });

    return results;
  }

  replace(query: string, replacement: string, options: ReplaceOptions = {}): number {
    const searchRegex = this.createSearchRegex(query, options);
    let replacements = 0;

    const filesToReplace = options.fileIndex !== undefined 
      ? [{ file: this.openFiles[options.fileIndex], index: options.fileIndex }].filter(item => item.file)
      : this.openFiles.map((file, index) => ({ file, index }));

    filesToReplace.forEach(({ file, index }) => {
      const originalContent = file.content;
      let newContent;

      if (options.replaceAll) {
        newContent = originalContent.replace(searchRegex, replacement);
        const matches = originalContent.match(searchRegex);
        replacements += matches ? matches.length : 0;
      } else {
        newContent = originalContent.replace(searchRegex, replacement);
        if (newContent !== originalContent) {
          replacements++;
        }
      }

      if (newContent !== originalContent) {
        this.updateFileContent(index, newContent);
      }
    });

    return replacements;
  }

  getConfig(): EditorConfig {
    return { ...this.config };
  }

  async updateConfig(newConfig: Partial<EditorConfig>): Promise<void> {
    this.config = { ...this.config, ...newConfig };
    
    this.saveConfig();

    this.emitEvent({
      type: 'config:changed',
      payload: newConfig
    });
  }

  addEventListener(listener: (event: EditorEvent) => void): () => void {
    this.listeners.add(listener);
    
    return () => {
      this.listeners.delete(listener);
    };
  }

  switchToFile(index: number): void {
    if (index >= 0 && index < this.openFiles.length) {
      this.activeFileIndex = index;
    }
  }

  getActiveFileIndex(): number {
    return this.activeFileIndex;
  }

  private detectLanguage(fileName: string, _content: string): LanguageDetectionResult {
    const detectedLanguage = getMonacoLanguage(fileName);
    const hasExtension = fileName.includes('.');
    
    return {
      language: detectedLanguage,
      confidence: hasExtension ? 0.9 : 0.1,
      detected: hasExtension
    };
  }

  private createSearchRegex(query: string, options: SearchOptions): RegExp {
    let flags = 'g'; // Global search
    
    if (!options.caseSensitive) {
      flags += 'i';
    }

    let pattern = query;
    
    if (!options.regex) {
      pattern = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    }

    if (options.wholeWord) {
      pattern = `\\b${pattern}\\b`;
    }

    return new RegExp(pattern, flags);
  }

  private setupAutoSave(index: number): void {
    if (this.config.autoSave === 'afterDelay') {
      this.resetAutoSave(index);
    }
  }

  private resetAutoSave(index: number): void {
    this.clearAutoSave(index);
    
    if (this.config.autoSave === 'afterDelay') {
      const timer = setTimeout(() => {
        if (this.isFileDirty(index)) {
          this.saveFile(index).catch(error => {
            log.error('Auto save failed', error);
          });
        }
      }, this.config.autoSaveDelay);
      
      this.autoSaveTimers.set(index, timer);
    }
  }

  private clearAutoSave(index: number): void {
    const timer = this.autoSaveTimers.get(index);
    if (timer) {
      clearTimeout(timer);
      this.autoSaveTimers.delete(index);
    }
  }

  private async promptSave(_file: FileContent): Promise<boolean> {
    // TODO: show save confirmation dialog
    return true;
  }

  private emitEvent(event: EditorEvent): void {
    this.listeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Error in event listener', error);
      }
    });

    globalEventBus.emit(`editor:${event.type}`, event.payload);
  }

  private loadConfig(): void {
    try {
      const savedConfig = localStorage.getItem('editor-config');
      if (savedConfig) {
        const parsed = JSON.parse(savedConfig);
        this.config = { ...DEFAULT_CONFIG, ...parsed };
      }
    } catch (error) {
      log.warn('Failed to load config', error);
    }
  }

  private saveConfig(): void {
    try {
      localStorage.setItem('editor-config', JSON.stringify(this.config));
    } catch (error) {
      log.warn('Failed to save config', error);
    }
  }

  destroy(): void {
    this.autoSaveTimers.forEach(timer => clearTimeout(timer));
    this.autoSaveTimers.clear();
    
    this.listeners.clear();
    
    this.openFiles = [];
    this.activeFileIndex = -1;
  }
}

/** Default editor manager instance. */
export const editorManager = new EditorManager();
