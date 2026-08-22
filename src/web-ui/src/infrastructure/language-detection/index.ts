/**
 * Language detection.
 */

// Core services
export { languageDetector } from './core/LanguageDetector';
export { languageRegistry } from './core/LanguageRegistry';

// Types
export type {
  // Language
  Language,
  LanguageCategory,
  FileDetectionResult,
  
  // Context
  DetectionContext,
  
  // Plugins
  LanguagePlugin,
  
  // Events
  LanguageDetectionEvent,
} from './types';

// Convenience exports
export {
  detectLanguage,
  getMonacoLanguage,
  getFileIconType,
  getFileColor,
  getPrismLanguage,
  getPrismLanguageFromAlias,
  getEditorType,
  isImageFile,
  isCodeFile,
  isConfigFile,
  isDocumentationFile,
  getCommentPrefix,
  getLanguageById,
} from './utils/helpers';
