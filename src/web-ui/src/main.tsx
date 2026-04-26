import ReactDOM from "react-dom/client";
import App from "./app/App";
import AppErrorBoundary from "./app/components/AppErrorBoundary";
import { WorkspaceProvider } from "./infrastructure/contexts/WorkspaceProvider";
import "./app/styles/index.scss";

// Manually import Monaco Editor CSS.
// This ensures the CSS loads correctly in Tauri production.
import 'monaco-editor/min/vs/editor/editor.main.css';

// Font: Noto Sans SC is loaded via a <link> tag in index.html.
// File path: public/fonts/fonts.css, served as /fonts/fonts.css.

import { initializeAllTools } from "./tools";
import { initContextMenuSystem } from "./shared/context-menu-system";
import { loader } from '@monaco-editor/react';
import { getMonacoPath, getMonacoWorkerPath, logMonacoResourceCheck } from './tools/editor/utils/monacoPathHelper';
import { bootstrapLogger, createLogger, initLogger } from './shared/utils/logger';
import { elapsedMs, logElapsed, measureAsyncAndLog, nowMs } from './shared/utils/timing';
import {
  buildReactCrashLogPayload,
  isMinifiedReactErrorMessage,
} from './shared/utils/reactProductionError';

// Install console forwarding before app startup so early console output is persisted too.
bootstrapLogger();

const log = createLogger('App');

/** Dedupe only for white-screen heuristic (empty #root), not for Error Boundary logs. */
const WHITE_SCREEN_LOGGED_FLAG = '__bitfun_white_screen_crash_logged__';
function hasLoggedWhiteScreenCrash(): boolean {
  return Boolean((window as any)[WHITE_SCREEN_LOGGED_FLAG]);
}
function markWhiteScreenCrashLogged(): void {
  (window as any)[WHITE_SCREEN_LOGGED_FLAG] = true;
}

function serializeError(err: unknown): Record<string, unknown> {
  if (err instanceof Error) {
    return {
      name: err.name,
      message: err.message,
      stack: err.stack,
    };
  }
  return { value: String(err) };
}

function isRootEmpty(): boolean {
  const root = document.getElementById('root');
  if (!root) {
    return true;
  }
  return root.childElementCount === 0;
}

function registerGlobalErrorHandlers() {
  const flag = '__bitfun_global_error_handlers_registered__';
  const w = window as any;
  if (w[flag]) {
    return;
  }
  w[flag] = true;

  const scheduleCrashLog = (payload: { location: string; message: string; data?: Record<string, unknown> }) => {
    // Always persist uncaught errors so they appear in webview.log for diagnostics.
    // Mark white-screen crashes separately to allow callers to deduplicate.
    queueMicrotask(() => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          const isWhiteScreen = isRootEmpty();
          const crashType = isWhiteScreen ? 'white-screen' : 'page-error';
          // Deduplicate only white-screen crashes to avoid duplicate startup logs.
          if (isWhiteScreen && hasLoggedWhiteScreenCrash()) {
            return;
          }
          if (isWhiteScreen) {
            markWhiteScreenCrashLogged();
          }
          log.error(`[CRASH:${crashType}] Uncaught error`, {
            location: payload.location,
            message: payload.message,
            ...payload.data,
          });
        });
      });
    });
  };

  window.addEventListener(
    'error',
    (event: Event) => {
      if (event instanceof ErrorEvent) {
        const msg = event.message || '';
        // Minified React errors often reach window.error even when #root is not empty;
        // always persist so production builds get react.dev/errors/{code} in webview.log.
        if (isMinifiedReactErrorMessage(msg)) {
          const err =
            event.error instanceof Error ? event.error : new Error(msg);
          log.error('[CRASH] window:error (minified React)', {
            location: 'window:error',
            ...buildReactCrashLogPayload(err),
            filename: event.filename,
            lineno: event.lineno,
            colno: event.colno,
          });
        }
        scheduleCrashLog({
          location: 'window:error',
          message: msg || 'window error',
          data: {
            filename: event.filename,
            lineno: event.lineno,
            colno: event.colno,
            error: serializeError(event.error),
          },
        });
        return;
      }

    // Resource load errors rarely cause a white screen; log only if root is empty.
      const target = event.target as any;
      scheduleCrashLog({
        location: 'window:resource-error',
        message: 'resource load error',
        data: {
          tagName: target?.tagName,
          src: target?.src,
          href: target?.href,
        },
      });
    },
    true
  );

  window.addEventListener('unhandledrejection', (event: PromiseRejectionEvent) => {
    const reason = event.reason;
    const msg =
      reason instanceof Error
        ? reason.message
        : typeof reason === 'string'
          ? reason
          : '';
    if (isMinifiedReactErrorMessage(msg)) {
      const err = reason instanceof Error ? reason : new Error(msg);
      log.error('[CRASH] unhandledrejection (minified React)', {
        location: 'window:unhandledrejection',
        ...buildReactCrashLogPayload(err),
      });
    }
    scheduleCrashLog({
      location: 'window:unhandledrejection',
      message: 'unhandled rejection',
      data: {
        reason: serializeError(event.reason),
      },
    });
  });
}

registerGlobalErrorHandlers();

// Disable Tab-key focus traversal globally.
// Tab still works inside Monaco Editor and xterm terminal where it has semantic meaning.
document.addEventListener(
  'keydown',
  (e: KeyboardEvent) => {
    if (e.key !== 'Tab') return;
    const target = e.target as Element | null;
    if (target?.closest('.monaco-editor, .xterm')) return;
    e.preventDefault();
  },
  true
);

// Configure Monaco Editor loader - use local files (offline-ready).
const isDev = import.meta.env.DEV;
const monacoPath = getMonacoPath();

loader.config({
  paths: {
    vs: monacoPath
  }
});

// Debug: check resource availability in production.
if (!isDev) {
  // Delay checks to avoid blocking startup.
  setTimeout(() => {
    logMonacoResourceCheck().catch(err => {
      log.error('Monaco resource check failed', err);
    });
  }, 2000);
}

// Optimization: Monaco Editor worker mapping.
const MONACO_WORKER_MAP: Record<string, string> = {
  json: 'language/json/jsonWorker.js',
  css: 'language/css/cssWorker.js',
  scss: 'language/css/cssWorker.js',
  less: 'language/css/cssWorker.js',
  html: 'language/html/htmlWorker.js',
  handlebars: 'language/html/htmlWorker.js',
  razor: 'language/html/htmlWorker.js',
  typescript: 'language/typescript/tsWorker.js',
  javascript: 'language/typescript/tsWorker.js',
};

const DEFAULT_WORKER = 'base/worker/workerMain.js';

(window as any).MonacoEnvironment = {
  getWorker(_workerId: string, label: string) {
    const workerFile = MONACO_WORKER_MAP[label] || DEFAULT_WORKER;
    const workerPath = getMonacoWorkerPath(workerFile);
    
    return new Worker(workerPath, {
      type: 'classic',
      name: `monaco-${label}-worker`
    });
  }
};

/** Logger, theme, and minimal deps — must finish before first React paint (F5 / webview reload does not re-run Tauri init script). */
async function initializeBeforeRender(): Promise<void> {
  const phaseStartedAt = nowMs();
  await measureAsyncAndLog(log, 'Startup step completed', () => initLogger(), {
    data: { step: 'initLogger' },
  });

  await measureAsyncAndLog(log, 'Startup step completed', async () => {
    const { initializeFrontendLogLevelSync } = await import('./infrastructure/config/services/FrontendLogLevelSync');
    await initializeFrontendLogLevelSync();
  }, {
    data: { step: 'initializeFrontendLogLevelSync' },
  });

  log.debug('Monaco loader configured', { vs: monacoPath, isDev });
  log.info('Initializing BitFun');

  await measureAsyncAndLog(log, 'Startup step completed', async () => {
    const { registerDefaultContextTypes } = await import('./shared/context-system/core/registerDefaultTypes');
    registerDefaultContextTypes();
  }, {
    data: { step: 'registerDefaultContextTypes' },
  });

  await measureAsyncAndLog(log, 'Startup step completed', async () => {
    const { initRecommendationProviders } = await import('./flow_chat/components/smart-recommendations');
    initRecommendationProviders();
  }, {
    data: { step: 'initRecommendationProviders' },
  });

  await measureAsyncAndLog(log, 'Startup step completed', async () => {
    const { themeService } = await import('./infrastructure/theme');
    await themeService.initialize();
  }, {
    data: { step: 'themeService.initialize' },
  });
  log.info('Theme system initialized');
  logElapsed(log, 'Startup phase completed', phaseStartedAt, {
    data: { phase: 'initializeBeforeRender' },
  });
}

/** Rest of startup runs after the shell is visible so refresh latency stays reasonable. */
async function initializeAfterRender(): Promise<void> {
  const phaseStartedAt = nowMs();
  const { fontPreferenceService } = await import('./infrastructure/font-preference');
  await fontPreferenceService.initialize();
  log.info('Font preference initialized at startup');

  const { configManager } = await import('./infrastructure/config');
  await configManager.getConfig('editor');
  log.info('Editor configuration preloaded');

  const initResults = await Promise.allSettled([
    initializeAllTools(),
    (async () => {
      initContextMenuSystem({
        registerBuiltinCommands: true,
        registerBuiltinProviders: true,
        debug: false,
      });

      const { registerNotificationContextMenu } = await import('./shared/notification-system');
      registerNotificationContextMenu();
    })(),
    (async () => {
      const { MonacoManager } = await import('./tools/editor');
      await MonacoManager.initialize();

      const { monacoThemeSync } = await import('./infrastructure/theme/integrations/MonacoThemeSync');
      await monacoThemeSync.initialize();
      log.info('Monaco theme sync initialized');
    })(),
  ]);

  initResults.forEach((result, index) => {
    const names = ['Tools', 'ContextMenu', 'Editors'];
    if (result.status === 'rejected') {
      log.warn('Initialization failed', { module: names[index], error: result.reason });
    }
  });

  log.info('BitFun core systems initialized successfully');
  logElapsed(log, 'Startup phase completed', phaseStartedAt, {
    data: { phase: 'initializeAfterRender' },
  });
}

async function startApplication(): Promise<void> {
  const appStartedAt = nowMs();
  try {
    await initializeBeforeRender();
  } catch (error) {
    log.error('Failed to initialize BitFun (pre-render)', error);
  }

  // I18n Provider.
  const i18nProviderImportResult = await measureAsyncAndLog(
    log,
    'Startup step completed',
    () => import('./infrastructure/i18n'),
    { data: { step: 'loadI18nProvider' } }
  );
  const { I18nProvider } = i18nProviderImportResult.value;

  const renderStartedAt = nowMs();
  ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <AppErrorBoundary>
      <I18nProvider>
        <WorkspaceProvider>
          <App />
        </WorkspaceProvider>
      </I18nProvider>
    </AppErrorBoundary>
  );
  logElapsed(log, 'Startup step completed', renderStartedAt, {
    data: {
      step: 'scheduleInitialRender',
      sinceStartupMs: elapsedMs(appStartedAt),
    },
  });

  try {
    await initializeAfterRender();
  } catch (error) {
    log.error('Failed to complete post-render initialization', error);
  }

  logElapsed(log, 'Startup phase completed', appStartedAt, {
    data: { phase: 'startApplication' },
  });
}

void startApplication();
