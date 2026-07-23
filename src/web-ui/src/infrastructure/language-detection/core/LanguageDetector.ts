 

import type { 
  Language, 
  DetectionContext, 
  FileDetectionResult,
  LanguagePlugin,
  LspConfig 
} from '../types';
import { languageRegistry } from './LanguageRegistry';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('LanguageDetector');

// ============================================================================

// ============================================================================

 
const LSP_CONFIG_MAP: Record<string, LspConfig> = {
  typescript: {
    serverId: 'typescript-language-server',
    enabled: true,
  },
  javascript: {
    serverId: 'typescript-language-server',
    enabled: true,
  },
  python: {
    serverId: 'pylsp',
    enabled: true,
  },
  rust: {
    serverId: 'rust-analyzer',
    enabled: true,
  },
  go: {
    serverId: 'gopls',
    enabled: true,
  },
  java: {
    serverId: 'jdtls',
    enabled: true,
  },
  html: {
    serverId: 'vscode-html-language-server',
    enabled: true,
  },
  css: {
    serverId: 'vscode-css-language-server',
    enabled: true,
  },
  json: {
    serverId: 'vscode-json-language-server',
    enabled: true,
  },
  yaml: {
    serverId: 'yaml-language-server',
    enabled: true,
  },
  vue: {
    serverId: 'vue-language-server',
    enabled: true,
  },
  svelte: {
    serverId: 'svelte-language-server',
    enabled: true,
  },
};

// ============================================================================

// ============================================================================

 
class LanguageDetector {
  private static instance: LanguageDetector;
  
   
  private cache = new Map<string, FileDetectionResult>();
  
   
  private plugins: LanguagePlugin[] = [];
  
  private constructor() {}
  
   
  public static getInstance(): LanguageDetector {
    if (!LanguageDetector.instance) {
      LanguageDetector.instance = new LanguageDetector();
    }
    return LanguageDetector.instance;
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  public detectFile(
    filePathOrName: string, 
    context?: Partial<DetectionContext>
  ): FileDetectionResult {
    
    const normalizedPath = this.normalizePath(filePathOrName);
    
    
    const cacheKey = this.getCacheKey(normalizedPath, context);
    const cached = this.cache.get(cacheKey);
    if (cached) {
      return cached;
    }
    
    
    const fullContext: DetectionContext = {
      filePath: normalizedPath,
      ...context,
    };
    
    
    const result = this.doDetect(fullContext);
    
    
    this.cache.set(cacheKey, result);
    
    return result;
  }
  
   
  private doDetect(context: DetectionContext): FileDetectionResult {
    const { filePath, fileContent, hints } = context;
    
    
    if (hints?.language) {
      const hintLang = languageRegistry.getById(hints.language);
      if (hintLang) {
        return {
          language: hintLang,
          confidence: 1.0,
          method: 'hint',
        };
      }
    }
    
    
    for (const plugin of this.plugins) {
      if (plugin.detect) {
        const result = plugin.detect(context);
        if (result) {
          return result;
        }
      }
    }
    
    
    const fileName = this.extractFileName(filePath || '');
    const extension = this.extractExtension(fileName);
    
    
    const byFilename = languageRegistry.getByFilename(fileName);
    if (byFilename) {
      return {
        language: byFilename,
        confidence: 0.95,
        method: 'filename',
      };
    }
    
    
    if (extension) {
      const byExtension = languageRegistry.getByExtension(extension);
      if (byExtension.length === 1) {
        return {
          language: byExtension[0],
          confidence: 0.9,
          method: 'extension',
        };
      }
      if (byExtension.length > 1) {
        
        const primary = this.resolvePrimaryLanguage(byExtension, context);
        return {
          language: primary,
          confidence: 0.8,
          method: 'extension',
          candidates: byExtension,
        };
      }
    }
    
    
    if (fileContent) {
      const firstLine = fileContent.split('\n')[0];
      const byFirstLine = this.detectByFirstLine(firstLine);
      if (byFirstLine) {
        return {
          language: byFirstLine,
          confidence: 0.85,
          method: 'firstLine',
        };
      }
    }
    
    
    return {
      language: languageRegistry.getDefault(),
      confidence: 0.1,
      method: 'default',
    };
  }
  
   
  private detectByFirstLine(firstLine: string): Language | null {
    const allLanguages = languageRegistry.getAll();
    
    for (const lang of allLanguages) {
      if (lang.firstLineMatch && lang.firstLineMatch.test(firstLine)) {
        return lang;
      }
    }
    
    return null;
  }
  
   
  private resolvePrimaryLanguage(candidates: Language[], context: DetectionContext): Language {
    
    if (context.projectInfo) {
      const projectLang = context.projectInfo.primaryLanguage;
      const matching = candidates.find(c => c.id === projectLang || c.parent === projectLang);
      if (matching) {
        return matching;
      }
    }
    
    
    const primary = candidates.find(c => !c.parent);
    if (primary) {
      return primary;
    }
    
    
    return candidates[0];
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  public getMonacoLanguage(filePathOrName: string): string {
    return this.detectFile(filePathOrName).language.monacoId;
  }
  
   
  public getIconType(filePathOrName: string): string {
    return this.detectFile(filePathOrName).language.iconType;
  }
  
   
  public getColor(filePathOrName: string): string | undefined {
    return this.detectFile(filePathOrName).language.color;
  }
  
   
  public hasLspSupport(filePathOrName: string): boolean {
    const lang = this.detectFile(filePathOrName).language;
    return !!lang.lspId && !!LSP_CONFIG_MAP[lang.id];
  }
  
   
  public getLspConfig(languageId: string): LspConfig | undefined {
    return LSP_CONFIG_MAP[languageId];
  }
  
   
  public getLanguage(id: string): Language | undefined {
    return languageRegistry.getById(id);
  }
  
   
  public getAllLanguages(): Language[] {
    return languageRegistry.getAll();
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  public registerPlugin(plugin: LanguagePlugin): void {
    this.plugins.push(plugin);
    languageRegistry.registerPlugin(plugin);
    
    
    this.cache.clear();
    
    log.debug('Plugin registered', { pluginName: plugin.name });
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  public clearCache(): void {
    this.cache.clear();
  }
  
   
  public invalidateCache(filePath: string): void {
    const normalizedPath = this.normalizePath(filePath);
    
    
    for (const key of this.cache.keys()) {
      if (key.startsWith(normalizedPath)) {
        this.cache.delete(key);
      }
    }
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  private normalizePath(path: string): string {
    return path.replace(/\\/g, '/').toLowerCase();
  }
  
   
  private extractFileName(path: string): string {
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || '';
  }
  
   
  private extractExtension(fileName: string): string {
    const dotIndex = fileName.lastIndexOf('.');
    if (dotIndex === -1 || dotIndex === 0) {
      return '';
    }
    return fileName.substring(dotIndex + 1);
  }
  
   
  private getCacheKey(path: string, context?: Partial<DetectionContext>): string {
    let key = path;
    if (context?.hints?.language) {
      key += `|hint:${context.hints.language}`;
    }
    if (context?.projectInfo?.type) {
      key += `|project:${context.projectInfo.type}`;
    }
    return key;
  }
  
   
  public getStats(): {
    cacheSize: number;
    pluginCount: number;
    languageCount: number;
  } {
    return {
      cacheSize: this.cache.size,
      pluginCount: this.plugins.length,
      languageCount: languageRegistry.getAll().length,
    };
  }
}


export const languageDetector = LanguageDetector.getInstance();
export default LanguageDetector;
