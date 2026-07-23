/**
 * Language and project detection.
 */

// Core services
export { languageDetector } from './core/LanguageDetector';
export { languageRegistry } from './core/LanguageRegistry';
export { projectDetector } from './core/ProjectDetector';

// Types
export type {
  // Language
  Language,
  LanguageCategory,
  FileDetectionResult,
  
  // Project
  ProjectInfo,
  ProjectType,
  FrameworkInfo,
  
  // Context
  DetectionContext,
  
  // Plugins
  LanguagePlugin,
  ProjectDetectionPlugin,
  
  // LSP
  LspConfig,
  
  // Events
  LanguageDetectionEvent,
} from './types';

// Convenience exports
export {
  detectLanguage,
  getMonacoLanguage,
  getFileIconType,
  getFileColor,
  hasLspSupport,
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
