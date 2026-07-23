 

import type { Language, FileDetectionResult, DetectionContext } from '../types';
import { languageDetector } from '../core/LanguageDetector';

 
export function detectLanguage(
  filePathOrName: string,
  context?: Partial<DetectionContext>
): FileDetectionResult {
  return languageDetector.detectFile(filePathOrName, context);
}

 
export function getMonacoLanguage(filePathOrName: string): string {
  return languageDetector.getMonacoLanguage(filePathOrName);
}

 
export function getFileIconType(filePathOrName: string): string {
  return languageDetector.getIconType(filePathOrName);
}

 
export function getFileColor(filePathOrName: string): string | undefined {
  return languageDetector.getColor(filePathOrName);
}

 
export function hasLspSupport(filePathOrName: string): boolean {
  return languageDetector.hasLspSupport(filePathOrName);
}

 
export function getLanguageById(languageId: string): Language | undefined {
  return languageDetector.getLanguage(languageId);
}

 
export function isImageFile(filePathOrName: string): boolean {
  const result = detectLanguage(filePathOrName);
  return result.language.iconType === 'image';
}

 
export function isCodeFile(filePathOrName: string): boolean {
  const result = detectLanguage(filePathOrName);
  return result.language.category === 'programming';
}

 
export function isConfigFile(filePathOrName: string): boolean {
  const result = detectLanguage(filePathOrName);
  return result.language.category === 'config' || result.language.category === 'data';
}

 
export function isDocumentationFile(filePathOrName: string): boolean {
  const result = detectLanguage(filePathOrName);
  return result.language.category === 'documentation';
}

 
export function getCommentPrefix(languageId: string): string | undefined {
  const lang = languageDetector.getLanguage(languageId);
  return lang?.lineCommentPrefix;
}

 
export function getPrismLanguage(filePathOrName: string): string {
  const result = detectLanguage(filePathOrName);
  const lang = result.language;
  
  
  if (lang.prismId) {
    return lang.prismId;
  }
  
  
  const prismFallbackMap: Record<string, string> = {
    'csharp': 'csharp',
    'cpp': 'cpp',
    'c': 'c',
    'plaintext': 'text',
    'shell': 'bash',
    'bat': 'batch',
    'dockerfile': 'docker',
  };
  
  return prismFallbackMap[lang.monacoId] || lang.monacoId;
}

 
export function getPrismLanguageFromAlias(alias: string): string {
  if (!alias) return 'text';
  
  const lowerAlias = alias.toLowerCase();
  
  
  const lang = languageDetector.getLanguage(lowerAlias);
  if (lang) {
    return lang.prismId || lang.monacoId;
  }
  
  
  return lowerAlias;
}

 
export function getEditorType(filePathOrName: string): 'code-editor' | 'markdown-editor' | 'image-viewer' | 'plan-viewer' {
  const result = detectLanguage(filePathOrName);
  const iconType = result.language.iconType;
  
  
  const fileName = filePathOrName.split(/[/\\]/).pop()?.toLowerCase() || '';
  if (fileName.endsWith('.plan.md')) {
    return 'plan-viewer';
  }
  
  
  if (iconType === 'image') {
    return 'image-viewer';
  }
  
  
  if (result.language.id === 'markdown') {
    return 'markdown-editor';
  }
  
  
  return 'code-editor';
}
