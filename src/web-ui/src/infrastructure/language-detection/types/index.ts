 

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

 
export interface DetectionContext {
   
  workspacePath?: string;
  
   
  filePath?: string;
  
   
  fileContent?: string;
  
   
  hints?: {
    language?: string;
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

 
export interface LanguagePlugin {
   
  id: string;
  
   
  name: string;
  
   
  getLanguages(): Language[];
  
   
  detect?(context: DetectionContext): FileDetectionResult | null;
}

// ============================================================================

// ============================================================================

 
export interface LanguageDetectionEvent {
  type: 'language-detected' | 'plugin-registered';
  payload: unknown;
}
