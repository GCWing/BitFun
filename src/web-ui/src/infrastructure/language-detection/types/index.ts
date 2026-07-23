 

// ============================================================================

// ============================================================================

 
export type LanguageCategory = 
  | 'programming'     
  | 'markup'          
  | 'stylesheet'      
  | 'data'            
  | 'config'          
  | 'documentation'   
  | 'script'          
  | 'binary'          
  | 'media'           
  | 'other';          

 
export interface Language {
   
  id: string;
  
   
  name: string;
  
   
  category: LanguageCategory;
  
   
  extensions: string[];
  
   
  filenames?: string[];
  
   
  firstLineMatch?: RegExp;
  
   
  monacoId: string;
  
   
  prismId?: string;
  
   
  lspId?: string;
  
   
  textmateScope?: string;
  
   
  iconType: string;
  
   
  color?: string;
  
   
  aliases?: string[];
  
   
  parent?: string;
  
   
  supportsComments?: boolean;
  
   
  lineCommentPrefix?: string;
  
   
  blockComment?: { start: string; end: string };
  
   
  metadata?: Record<string, unknown>;
}

// ============================================================================

// ============================================================================

 
export type ProjectType =
  
  | 'react'
  | 'vue'
  | 'angular'
  | 'svelte'
  | 'nextjs'
  | 'nuxt'
  | 'vite'
  | 'webpack'
  
  | 'nodejs'
  | 'express'
  | 'nestjs'
  | 'fastify'
  
  | 'tauri'
  | 'electron'
  
  | 'react-native'
  | 'flutter'
  
  | 'rust'
  | 'cargo'
  | 'go'
  | 'python'
  | 'django'
  | 'flask'
  | 'java'
  | 'maven'
  | 'gradle'
  | 'dotnet'
  
  | 'monorepo'
  | 'library'
  | 'cli'
  | 'unknown';

 
export interface FrameworkInfo {
   
  name: string;
  
   
  version?: string;
  
   
  source?: string;
}

 
export interface ProjectInfo {
   
  type: ProjectType;
  
   
  primaryLanguage: string;
  
   
  languages: string[];
  
   
  languageStats: Record<string, number>;
  
   
  frameworks: FrameworkInfo[];
  
   
  packageManager?: 'npm' | 'yarn' | 'pnpm' | 'bun' | 'cargo' | 'pip' | 'poetry' | 'maven' | 'gradle' | 'go';
  
   
  buildTools?: string[];
  
   
  rootPath: string;
  
   
  configFiles: string[];
  
   
  isMonorepo: boolean;
  
   
  subProjects?: ProjectInfo[];
  
   
  confidence: number;
  
   
  metadata?: Record<string, unknown>;
}

// ============================================================================

// ============================================================================

 
export interface DetectionContext {
   
  workspacePath?: string;
  
   
  filePath?: string;
  
   
  fileContent?: string;
  
   
  projectInfo?: ProjectInfo;
  
   
  hints?: {
    language?: string;
    projectType?: ProjectType;
  };
}

 
export interface FileDetectionResult {
   
  language: Language;
  
   
  confidence: number;
  
   
  method: 'extension' | 'filename' | 'firstLine' | 'content' | 'hint' | 'default';
  
   
  candidates?: Language[];
}

// ============================================================================

// ============================================================================

 
export interface ProjectDetectionPlugin {
   
  id: string;
  
   
  name: string;
  
   
  supportedTypes: ProjectType[];
  
   
  priority: number;
  
   
  detect(context: DetectionContext): Promise<ProjectInfo | null>;
  
   
  getConfigFiles(): string[];
}

 
export interface LanguagePlugin {
   
  id: string;
  
   
  name: string;
  
   
  getLanguages(): Language[];
  
   
  detect?(context: DetectionContext): FileDetectionResult | null;
}

// ============================================================================

// ============================================================================

 
export interface LspConfig {
   
  serverId: string;
  
   
  enabled: boolean;
  
   
  initOptions?: Record<string, unknown>;
  
   
  workspaceConfig?: Record<string, unknown>;
}

// ============================================================================

// ============================================================================

 
export interface LanguageDetectionEvent {
  type: 'project-detected' | 'language-detected' | 'plugin-registered';
  payload: unknown;
}
