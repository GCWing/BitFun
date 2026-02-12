 
import { i18nService } from '@/infrastructure/i18n';

const t = (key: string, options?: Record<string, unknown>) => i18nService.t(key, options);
export interface GlobalConfig {
  app: AppConfig;
  editor: EditorConfig;
  terminal: TerminalConfig;
  workspace: WorkspaceConfig;
  ai: AIConfig;
  version: string;
  last_modified: number; 
}

export interface AppConfig {
  language: string;
  auto_update: boolean;
  telemetry: boolean;
  startup_behavior: string;
  confirm_on_exit: boolean;
  restore_windows: boolean;
  zoom_level: number;
  logging: AppLoggingConfig;
  sidebar: SidebarConfig;
  right_panel: RightPanelConfig;
  notifications: NotificationConfig;
  ai_experience: AIExperienceConfig;
}

export type BackendLogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'off';

export interface AppLoggingConfig {
  level: BackendLogLevel;
}

export interface SidebarConfig {
  width: number;
  collapsed: boolean;
}

export interface RightPanelConfig {
  width: number;
  collapsed: boolean;
}

export interface NotificationConfig {
  enabled: boolean;
  position: string;
  duration: number;
}

export interface AIExperienceConfig {
   
  enable_session_title_generation: boolean;
   
  enable_welcome_panel_ai_analysis: boolean;

  /** Whether to enable visual mode (use Mermaid diagrams to illustrate complex logic and flows). */
  enable_visual_mode: boolean;
}



export type ModelCapability =
  | 'text_chat'
  | 'image_understanding'
  | 'image_generation'
  | 'search'
  | 'function_calling'
  | 'speech_recognition';

export type ModelCategory = 
  | 'general_chat'
  | 'multimodal'
  | 'image_generation'
  | 'search_enhanced'
  | 'speech_recognition';

export interface ModelMetadata {
  category: ModelCategory;
  capabilities: ModelCapability[];
  recommendedFor?: string[];
  strengths?: string[];
}


export const CAPABILITY_LABELS: Record<ModelCapability, string> = {
  text_chat: t('settings/ai-model:capabilities.text_chat'),
  image_understanding: t('settings/ai-model:capabilities.image_understanding'),
  image_generation: t('settings/ai-model:capabilities.image_generation'),
  search: t('settings/ai-model:capabilities.search'),
  function_calling: t('settings/ai-model:capabilities.function_calling'),
  speech_recognition: t('settings/ai-model:capabilities.speech_recognition')
};


export const CATEGORY_LABELS: Record<ModelCategory, string> = {
  general_chat: t('settings/ai-model:category.general_chat'),
  multimodal: t('settings/ai-model:category.multimodal'),
  image_generation: t('settings/ai-model:category.image_generation'),
  search_enhanced: t('settings/ai-model:category.search_enhanced'),
  speech_recognition: t('settings/ai-model:category.speech_recognition')
};


export const CATEGORY_ICONS: Record<ModelCategory, string> = {
  general_chat: t('settings/ai-model:categoryIcons.general_chat'),
  multimodal: t('settings/ai-model:categoryIcons.multimodal'),
  image_generation: t('settings/ai-model:categoryIcons.image_generation'),
  search_enhanced: t('settings/ai-model:categoryIcons.search_enhanced'),
  speech_recognition: t('settings/ai-model:categoryIcons.speech_recognition')
};


export type CustomHeadersMode = 'replace' | 'merge';


export interface AIModelConfig {
  id?: string;
  name: string;
  provider: string;
  api_key?: string;        
  base_url: string;        
  model_name: string;      
  description?: string;    
  context_window?: number; 
  max_tokens?: number;     
  temperature?: number;
  top_p?: number;          
  frequency_penalty?: number; 
  presence_penalty?: number;  
  enabled: boolean;
  is_default?: boolean;    
  custom_headers?: Record<string, string>; 
  custom_headers_mode?: CustomHeadersMode; 
  skip_ssl_verify?: boolean; 
  custom_request_body?: string; 
  timeout?: number;

  
  category: ModelCategory;
  capabilities: ModelCapability[];
  recommended_for?: string[];
  metadata?: Record<string, any>;

  
  enable_thinking_process?: boolean;

  
  support_preserved_thinking?: boolean;
}

export interface ProxyConfig {
  enabled: boolean;
  url: string;
  username?: string;
  password?: string;
}

 
export interface DefaultModelsConfig {
   
  primary?: string | null;
   
  fast?: string | null;
   
  search?: string | null;
   
  image_understanding?: string | null;
   
  speech_recognition?: string | null;
}

export interface AIConfig {
  models: AIModelConfig[];  
  default_models: DefaultModelsConfig;  
  agent_models: Record<string, string>;  
  func_agent_models: Record<string, string>;  
  mode_configs: Record<string, ModeConfigItem>;  
  subagent_configs: Record<string, SubAgentConfigItem>;  
  proxy: ProxyConfig;  
  debug_mode_config: DebugModeConfig;  
  request_timeout: number;
  max_retries: number;
  temperature: number;
  max_tokens: number;
  streaming: boolean;
  auto_save_conversations: boolean;
  conversation_history_limit: number;
  tool_execution_timeout_secs?: number | null;
  tool_confirmation_timeout_secs?: number | null;
  skip_tool_confirmation?: boolean;
}



export interface ModeConfigItem {
  mode_id: string;  
  available_tools: string[];  
  enabled: boolean;  
  default_tools: string[];  
}


export interface SubAgentConfigItem {
  enabled: boolean;  
}



export type SkillLevel = 'user' | 'project';


export interface SkillInfo {
  name: string;         
  description: string;  
  path: string;         
  level: SkillLevel;
  enabled: boolean;
}



 
export interface DebugModeConfig {
   
  log_path: string;
   
  ingest_port: number;
   
  enabled_languages: string[];
   
  language_templates: Record<string, LanguageDebugTemplate>;
}

 
export interface LanguageDebugTemplate {
   
  language: string;
   
  display_name: string;
   
  enabled: boolean;
   
  instrumentation_template: string;
   
  region_start: string;
   
  region_end: string;
   
  notes: string[];
}

 
export const DEFAULT_DEBUG_MODE_CONFIG: DebugModeConfig = {
  log_path: '.bitfun/debug.log',
  ingest_port: 7242,
  enabled_languages: [],
  language_templates: {}
};

 
export const LANGUAGE_TEMPLATE_LABELS: Record<string, string> = {
  javascript: t('settings/debug:languageLabels.javascript'),
  python: t('settings/debug:languageLabels.python'),
  rust: t('settings/debug:languageLabels.rust'),
  go: t('settings/debug:languageLabels.go'),
  java: t('settings/debug:languageLabels.java')
};

 
export const ALL_LANGUAGES = ['javascript', 'python', 'rust', 'go', 'java'] as const;

 
export const DEFAULT_LANGUAGE_TEMPLATES: Record<string, LanguageDebugTemplate> = {
  javascript: {
    language: 'javascript',
    display_name: t('settings/debug:languageLabels.javascript'),
    enabled: false,  
    instrumentation_template: `fetch('http://127.0.0.1:{PORT}/ingest/{SESSION_ID}',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({location:'{LOCATION}',message:'{MESSAGE}',data:{DATA},timestamp:Date.now(),sessionId:'{SESSION_ID}',hypothesisId:'{HYPOTHESIS_ID}',runId:'{RUN_ID}'})}).catch(()=>{});`,
    region_start: '// #region agent log',
    region_end: '// #endregion',
    notes: [
      t('settings/debug:templates.noteItems.javascript.postToIngest'),
      t('settings/debug:templates.noteItems.javascript.replaceData'),
    ],
  },
  python: {
    language: 'python',
    display_name: t('settings/debug:languageLabels.python'),
    enabled: false,
    instrumentation_template: `import json, time, os
with open(os.path.join(os.getcwd(), '{LOG_PATH}'), 'a', encoding='utf-8') as _f:
    _f.write(json.dumps({"location": "{LOCATION}", "message": "{MESSAGE}", "data": {DATA}, "timestamp": int(time.time()*1000), "sessionId": "{SESSION_ID}", "hypothesisId": "{HYPOTHESIS_ID}", "runId": "{RUN_ID}"}, ensure_ascii=False) + '\\n')`,
    region_start: '# region agent log',
    region_end: '# endregion',
    notes: [
      t('settings/debug:templates.noteItems.python.appendNdjson'),
      t('settings/debug:templates.noteItems.python.ensureAscii'),
      t('settings/debug:templates.noteItems.python.replaceData'),
      t('settings/debug:templates.noteItems.python.importOnce'),
    ],
  },
  rust: {
    language: 'rust',
    display_name: t('settings/debug:languageLabels.rust'),
    enabled: false,
    instrumentation_template: `{
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    if let Ok(mut _f) = OpenOptions::new().create(true).append(true).open("{LOG_PATH}") {
        let _ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let _ = writeln!(_f, r#"{{"location":"{LOCATION}","message":"{MESSAGE}","data":{},"timestamp":{},"sessionId":"{SESSION_ID}","hypothesisId":"{HYPOTHESIS_ID}","runId":"{RUN_ID}"}}"#, serde_json::json!({DATA}), _ts);
    }
}`,
    region_start: '// #region agent log',
    region_end: '// #endregion',
    notes: [
      t('settings/debug:templates.noteItems.rust.appendNdjson'),
      t('settings/debug:templates.noteItems.rust.requireSerdeJson'),
      t('settings/debug:templates.noteItems.rust.replaceData'),
      t('settings/debug:templates.noteItems.rust.syncOnly'),
    ],
  },
  go: {
    language: 'go',
    display_name: t('settings/debug:languageLabels.go'),
    enabled: false,
    instrumentation_template: `func() {
	f, err := os.OpenFile("{LOG_PATH}", os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err == nil {
		defer f.Close()
		data, _ := json.Marshal(map[string]interface{}{"location": "{LOCATION}", "message": "{MESSAGE}", "data": {DATA}, "timestamp": time.Now().UnixMilli(), "sessionId": "{SESSION_ID}", "hypothesisId": "{HYPOTHESIS_ID}", "runId": "{RUN_ID}"})
		f.Write(append(data, '\\n'))
	}
}()`,
    region_start: '// #region agent log',
    region_end: '// #endregion',
    notes: [
      t('settings/debug:templates.noteItems.go.iife'),
      t('settings/debug:templates.noteItems.go.appendNdjson'),
      t('settings/debug:templates.noteItems.go.imports'),
      t('settings/debug:templates.noteItems.go.replaceData'),
    ],
  },
  java: {
    language: 'java',
    display_name: t('settings/debug:languageLabels.java'),
    enabled: false,
    instrumentation_template: `try {
    java.nio.file.Files.writeString(
        java.nio.file.Path.of("{LOG_PATH}"),
        String.format("{\\"location\\":\\"{LOCATION}\\",\\"message\\":\\"{MESSAGE}\\",\\"data\\":%s,\\"timestamp\\":%d,\\"sessionId\\":\\"{SESSION_ID}\\",\\"hypothesisId\\":\\"{HYPOTHESIS_ID}\\",\\"runId\\":\\"{RUN_ID}\\"}%n",
            new com.google.gson.Gson().toJson({DATA}), System.currentTimeMillis()),
        java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
} catch (Exception _e) { /* debug log */ }`,
    region_start: '// #region agent log',
    region_end: '// #endregion',
    notes: [
      t('settings/debug:templates.noteItems.java.appendNdjson'),
      t('settings/debug:templates.noteItems.java.requireGson'),
      t('settings/debug:templates.noteItems.java.replaceData'),
      t('settings/debug:templates.noteItems.java.writeString'),
    ],
  },
};


export interface SkillValidationResult {
  valid: boolean;
  name?: string;
  description?: string;
  error?: string;
}


export interface EditorConfig {
  font_size: number;        
  font_family: string;      
  font_weight?: 'normal' | 'bold'; 
  line_height: number;      
  tab_size: number;         
  insert_spaces: boolean;   
  word_wrap: string;        
  line_numbers: string;     
  minimap: MinimapConfig;
  theme: string;
  auto_save: string;        
  auto_save_delay: number;  
  format_on_save: boolean;  
  format_on_paste: boolean; 
  trim_auto_whitespace: boolean; 
  
  cursor_style?: string;           
  cursor_blinking?: string;        
  render_whitespace?: string;      
  render_line_highlight?: string;  
  
  smooth_scrolling?: boolean;      
  scroll_beyond_last_line?: boolean; 
  
  semantic_highlighting?: boolean;   
  bracket_pair_colorization?: boolean; 
}

export interface MinimapConfig {
  enabled: boolean;
  side?: string;
  size?: string;
}


export interface TerminalConfig {
  default_shell: string;        
  font_size: number;            
  font_family: string;          
  cursor_style: string;         
  cursor_blink: boolean;        
  scrollback_lines: number;     
  theme: string;
  transparency: number;
  bell_style: string;           
  copy_on_select: boolean;      
  paste_on_right_click: boolean; 
  confirm_on_exit: boolean;     
  startup_command: string;      
  env_vars: Record<string, string>; 
}


export interface WorkspaceConfig {
  recent_workspaces: string[];
  max_recent_workspaces: number;
  auto_open_last_workspace: boolean;
  workspace_settings: Record<string, any>;
  exclude_patterns: string[];
  include_patterns: string[];
  file_associations: Record<string, string>;
  search_exclude_patterns: string[];
}



export interface IConfigManager {
  
  getConfig<T = any>(path?: string): Promise<T>;
  setConfig<T = any>(path: string, value: T): Promise<void>;
  resetConfig(path?: string): Promise<void>;
  
  
  validateConfig(): Promise<ConfigValidationResult>;
  exportConfig(): Promise<ConfigExport>;
  importConfig(config: ConfigExport): Promise<void>;
  
  
  onConfigChange(callback: (path: string, oldValue: any, newValue: any) => void): () => void;
  
  
  refreshCache(): Promise<void>;
  clearCache(): void;
}

export interface ConfigValidationResult {
  valid: boolean;
  errors: ConfigValidationError[];
  warnings: ConfigValidationWarning[];
}

export interface ConfigValidationError {
  path: string;
  message: string;
  code: string;
}

export interface ConfigValidationWarning {
  path: string;
  message: string;
  code: string;
}

export interface ConfigExport {
  config: GlobalConfig;
  metadata: {
    version: string;
    exported_at: number;
    exported_by: string;
  };
}



export interface ConfigChangeEvent {
  path: string;
  old_value: any;
  new_value: any;
  timestamp: number;
}



export interface UseConfigReturn<T = any> {
  data: T | null;
  loading: boolean;
  error: string | null;
  setConfig: (value: T) => Promise<void>;
  resetConfig: () => Promise<void>;
  refreshConfig: () => Promise<void>;
}



export type ConfigPath = 
  | 'app'
  | 'app.language'
  | 'app.auto_update'
  | 'app.telemetry'
  | 'app.sidebar'
  | 'app.sidebar.width'
  | 'app.sidebar.collapsed'
  | 'editor'
  | 'editor.font_size'
  | 'editor.theme'
  | 'terminal'
  | 'terminal.default_shell'
  | 'workspace'
  | 'ai'
  | 'ai.default_model'
  | 'ai.models'
  | 'agents'
  | string; 



export interface ConfigPanelProps {
  section?: keyof GlobalConfig;
  onClose?: () => void;
  onSave?: (config: Partial<GlobalConfig>) => void;
  readOnly?: boolean;
}

export interface RuntimeLoggingInfo {
  effectiveLevel: BackendLogLevel;
  sessionLogDir: string;
  appLogPath: string;
  aiLogPath: string;
  webviewLogPath: string;
}



 
export interface DefaultModels {
   
  primary: string | null;
   
  fast: string | null;
}

 
export interface OptionalCapabilityModels {
   
  image_understanding?: string;
   
  image_generation?: string;
   
  search?: string;
   
  speech_recognition?: string;
}

 
export type OptionalCapabilityType =
  | 'image_understanding'
  | 'image_generation'
  | 'search'
  | 'speech_recognition';
