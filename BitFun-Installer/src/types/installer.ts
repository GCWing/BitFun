/** Installation step identifiers */
export type InstallStep = 'lang' | 'options' | 'model' | 'progress' | 'theme' | 'uninstall';

export interface LaunchContext {
  mode: 'install' | 'uninstall';
  uninstallPath: string | null;
}

export type ThemeId =
  | 'bitfun-dark'
  | 'bitfun-light'
  | 'bitfun-midnight'
  | 'bitfun-china-style'
  | 'bitfun-china-night'
  | 'bitfun-cyber'
  | 'bitfun-slate';

export interface ModelConfig {
  provider: string;
  apiKey: string;
  baseUrl: string;
  modelName: string;
  format: 'openai' | 'anthropic';
}

/** Installation options sent to the Rust backend */
export interface InstallOptions {
  installPath: string;
  desktopShortcut: boolean;
  startMenu: boolean;
  contextMenu: boolean;
  addToPath: boolean;
  launchAfterInstall: boolean;
  appLanguage: 'zh-CN' | 'en-US';
  themePreference: ThemeId;
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
  themePreference: 'bitfun-slate',
  modelConfig: null,
};
