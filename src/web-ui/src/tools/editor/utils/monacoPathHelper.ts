/**
 * Monaco Editor Path Helper
 * 
 * Handles path differences between development and production (Tauri) environments.
 * @module utils/monacoPathHelper
 */

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MonacoPathHelper');

/**
 * Get Monaco Editor base path.
 * @returns Monaco VS path
 */
export function getMonacoPath(): string {
  const isDev = import.meta.env.DEV;
  
  if (isDev) {
    // In dev mode, use node_modules path served by Vite
    return '/node_modules/monaco-editor/min/vs';
  } else {
    return './monaco-editor/vs';
  }
}

/**
 * Get Monaco Worker path.
 * @param workerFile Worker file relative path (relative to vs directory)
 * @returns Full worker path
 */
export function getMonacoWorkerPath(workerFile: string): string {
  const basePath = getMonacoPath();
  return `${basePath}/${workerFile}`;
}

/**
 * Get Monaco CSS path.
 * @returns Monaco CSS file path
 */
export function getMonacoCssPath(): string {
  return getMonacoWorkerPath('editor/editor.main.css');
}

/**
 * Check if Monaco resources are accessible.
 * Used for debugging resource loading issues in production.
 */
export async function checkMonacoResources(): Promise<{
  loaderAvailable: boolean;
  cssAvailable: boolean;
  workerAvailable: boolean;
  errors: string[];
}> {
  const errors: string[] = [];
  const results = {
    loaderAvailable: false,
    cssAvailable: false,
    workerAvailable: false,
    errors
  };

  try {
    const loaderPath = getMonacoWorkerPath('loader.js');
    const loaderResponse = await fetch(loaderPath);
    results.loaderAvailable = loaderResponse.ok;
    if (!loaderResponse.ok) {
      errors.push(`Loader not found: ${loaderPath} (${loaderResponse.status})`);
    }
  } catch (error) {
    errors.push(`Loader fetch error: ${error}`);
  }

  try {
    const cssPath = getMonacoCssPath();
    const cssResponse = await fetch(cssPath);
    results.cssAvailable = cssResponse.ok;
    if (!cssResponse.ok) {
      errors.push(`CSS not found: ${cssPath} (${cssResponse.status})`);
    }
  } catch (error) {
    errors.push(`CSS fetch error: ${error}`);
  }

  try {
    const workerPath = getMonacoWorkerPath('base/worker/workerMain.js');
    const workerResponse = await fetch(workerPath);
    results.workerAvailable = workerResponse.ok;
    if (!workerResponse.ok) {
      errors.push(`Worker not found: ${workerPath} (${workerResponse.status})`);
    }
  } catch (error) {
    errors.push(`Worker fetch error: ${error}`);
  }

  return results;
}

/** Log Monaco resource check results (for debugging) */
export async function logMonacoResourceCheck(): Promise<void> {
  const results = await checkMonacoResources();
  
  if (results.errors.length > 0) {
    log.error('Monaco resource check failed', {
      environment: import.meta.env.DEV ? 'Development' : 'Production',
      basePath: getMonacoPath(),
      errors: results.errors
    });
  } else {
    log.debug('Monaco resource check passed', {
      environment: import.meta.env.DEV ? 'Development' : 'Production',
      basePath: getMonacoPath(),
      results
    });
  }
}

