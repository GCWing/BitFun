import type { AppLanguage } from '../i18n/languages';

/** Installation step identifiers */
export type InstallStep = 'lang' | 'options' | 'model' | 'progress' | 'theme' | 'uninstall';

export interface LaunchContext {
  mode: 'install' | 'uninstall';
  uninstallPath: string | null;
  appLanguage?: AppLanguage | null;
}

export interface InstallPathValidation {
  installPath: string;
}

export type ThemeId =
  | 'bitfun-dark'
  | 'bitfun-light'
  | 'bitfun-midnight'
  | 'bitfun-china-style'
  | 'bitfun-china-night'
  | 'bitfun-cyber'
  | 'bitfun-slate';

/** Matches main app `themes.current` when following OS appearance. */
export const SYSTEM_THEME_ID = 'system' as const;

export type ThemePreferenceId = ThemeId | typeof SYSTEM_THEME_ID;

export interface ModelConfig {
  provider: string;
  apiKey: string;
  baseUrl: string;
  modelName: string;
  format: 'openai' | 'anthropic' | 'gemini' | 'responses';
  configName?: string;
  customRequestBody?: string;
  skipSslVerify?: boolean;
  customHeaders?: Record<string, string>;
  customHeadersMode?: 'merge' | 'replace';
  /** Aligns with main app model capabilities when testing image input. */
  capabilities?: string[];
  /** Aligns with main app model category (e.g. multimodal). */
  category?: string;
}

/** Matches backend `ConnectionTestMessageCode` (camelCase JSON). */
export type ConnectionTestMessageCode = 'toolCallsNotDetected' | 'imageInputCheckFailed';

export interface ConnectionTestResult {
  success: boolean;
  responseTimeMs: number;
  modelResponse?: string;
  messageCode?: ConnectionTestMessageCode;
  errorDetails?: string;
}

/** Remote model id from installer list_models command (settings-aligned shape). */
export interface RemoteModelInfo {
  id: string;
  displayName?: string;
}

/** Installation options sent to the Rust backend */
export interface InstallOptions {
  installPath: string;
  desktopShortcut: boolean;
  startMenu: boolean;
  contextMenu: boolean;
  addToPath: boolean;
  launchAfterInstall: boolean;
  appLanguage: AppLanguage;
  themePreference: ThemePreferenceId;
  modelConfig: ModelConfig | null;
}

/** Progress update received from the backend */
export interface InstallProgress {
  step: string;
  percent: number;
  message: string;
}

/** Disk space information */
export interface DiskSpaceInfo {
  total: number;
  available: number;
  required: number;
  sufficient: boolean;
}

/** Default installation options */
export const DEFAULT_OPTIONS: InstallOptions = {
  installPath: '',
  desktopShortcut: true,
  startMenu: true,
  contextMenu: true,
  addToPath: true,
  launchAfterInstall: true,
  appLanguage: 'zh-CN',
  themePreference: SYSTEM_THEME_ID,
  modelConfig: null,
};
