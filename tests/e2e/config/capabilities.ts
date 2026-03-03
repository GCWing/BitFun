/**
 * Platform-specific WebDriver capabilities for Tauri E2E testing
 */

import * as path from 'path';
import * as os from 'os';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

// ESM-compatible __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Get the application path based on the current platform
 */
export function getApplicationPath(buildType: 'debug' | 'release' = 'release'): string {
  const isWindows = process.platform === 'win32';
  const isMac = process.platform === 'darwin';
  const isLinux = process.platform === 'linux';
  
  let appName: string;
  
  if (isWindows) {
    appName = 'bitfun-desktop.exe';
  } else if (isMac) {
    appName = 'BitFun.app/Contents/MacOS/BitFun';
  } else {
    appName = 'bitfun-desktop';
  }
  
  return path.resolve(__dirname, '..', '..', '..', 'target', buildType, appName);
}

/**
 * Windows-specific capabilities using Edge WebView2
 */
export const windowsCapabilities = {
  browserName: 'wry',
  'tauri:options': {
    application: getApplicationPath('release'),
  },
  // Edge WebDriver specific options if needed
  'ms:edgeOptions': {
    // Edge options for WebView2
  },
};

/**
 * Linux-specific capabilities using WebKitGTK
 */
export const linuxCapabilities = {
  browserName: 'wry',
  'tauri:options': {
    application: getApplicationPath('release'),
  },
  // WebKitWebDriver specific options if needed
  'webkit:browserOptions': {
    // WebKit options
  },
};

/**
 * macOS-specific capabilities (limited support)
 * Note: macOS WebDriver support for WKWebView is limited
 */
export const macOSCapabilities = {
  browserName: 'wry',
  'tauri:options': {
    application: getApplicationPath('release'),
  },
};

/**
 * Get capabilities for the current platform
 */
export function getPlatformCapabilities(): Record<string, unknown> {
  const platform = process.platform;
  
  switch (platform) {
    case 'win32':
      return windowsCapabilities;
    case 'linux':
      return linuxCapabilities;
    case 'darwin':
      console.warn('⚠️ macOS WebDriver support is limited for Tauri apps');
      return macOSCapabilities;
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }
}

/**
 * Environment-specific settings
 */
export const environmentSettings = {
  // Timeouts
  defaultTimeout: 10000,
  pageLoadTimeout: 30000,
  streamingResponseTimeout: 60000,
  animationTimeout: 2000,
  
  // Retry settings
  maxRetries: 3,
  retryDelay: 1000,
  
  // Screenshot settings
  screenshotOnFailure: true,
  screenshotPath: '../reports/screenshots',
  
  // Logging
  logLevel: process.env.E2E_LOG_LEVEL || 'info',
};

export default {
  getApplicationPath,
  getPlatformCapabilities,
  windowsCapabilities,
  linuxCapabilities,
  macOSCapabilities,
  environmentSettings,
};
