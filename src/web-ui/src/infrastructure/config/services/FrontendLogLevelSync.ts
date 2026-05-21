import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import {
  LogLevel,
  createLogger,
  logger,
  setIncludeSensitiveDiagnostics,
} from '@/shared/utils/logger';
import type { BackendLogLevel } from '../types';

const log = createLogger('FrontendLogLevelSync');
const LOGGING_LEVEL_PATH = 'app.logging.level';
const LOGGING_INCLUDE_SENSITIVE_PATH = 'app.logging.include_sensitive_diagnostics';

let initialSettingsLoaded = false;
let configWatcherInstalled = false;

function toFrontendLogLevel(level: string | null | undefined): LogLevel | null {
  switch (level?.trim().toLowerCase()) {
    case 'trace':
      return LogLevel.TRACE;
    case 'debug':
      return LogLevel.DEBUG;
    case 'info':
      return LogLevel.INFO;
    case 'warn':
      return LogLevel.WARN;
    case 'error':
      return LogLevel.ERROR;
    case 'off':
      return LogLevel.NONE;
    default:
      return null;
  }
}

function toBackendLogLevel(level: LogLevel): BackendLogLevel {
  switch (level) {
    case LogLevel.TRACE:
      return 'trace';
    case LogLevel.DEBUG:
      return 'debug';
    case LogLevel.INFO:
      return 'info';
    case LogLevel.WARN:
      return 'warn';
    case LogLevel.ERROR:
      return 'error';
    case LogLevel.NONE:
      return 'off';
  }
}

function applyFrontendLogLevel(level: string | null | undefined, source: string): void {
  const nextLevel = toFrontendLogLevel(level);
  if (nextLevel === null) {
    if (level) {
      log.warn('Ignoring invalid frontend log level', { level, source });
    }
    return;
  }

  const previousLevel = logger.getLevel();
  if (previousLevel === nextLevel) {
    return;
  }

  logger.setLevel(nextLevel);
  log.info('Frontend log level updated', {
    oldLevel: toBackendLogLevel(previousLevel),
    newLevel: toBackendLogLevel(nextLevel),
    source,
  });
}

async function resolveInitialLogLevel(): Promise<string | undefined> {
  const [savedLevelResult, runtimeInfoResult] = await Promise.allSettled([
    configAPI.getConfig(LOGGING_LEVEL_PATH) as Promise<BackendLogLevel>,
    configAPI.getRuntimeLoggingInfo(),
  ]);

  if (savedLevelResult.status === 'fulfilled' && toFrontendLogLevel(savedLevelResult.value) !== null) {
    return savedLevelResult.value;
  }

  if (runtimeInfoResult.status === 'fulfilled') {
    const runtimeLevel = runtimeInfoResult.value?.effectiveLevel;
    if (toFrontendLogLevel(runtimeLevel) !== null) {
      return runtimeLevel;
    }
  }

  return undefined;
}

async function resolveInitialSensitiveDiagnosticsPreference(): Promise<boolean> {
  const value = await configAPI.getConfig(LOGGING_INCLUDE_SENSITIVE_PATH) as boolean | undefined;
  return value ?? true;
}

export async function initializeFrontendLogLevelSync(): Promise<void> {
  if (initialSettingsLoaded) {
    return;
  }

  initialSettingsLoaded = true;

  try {
    const [initialLevel, includeSensitiveDiagnostics] = await Promise.all([
      resolveInitialLogLevel(),
      resolveInitialSensitiveDiagnosticsPreference(),
    ]);
    applyFrontendLogLevel(initialLevel, 'startup');
    setIncludeSensitiveDiagnostics(includeSensitiveDiagnostics);
  } catch (error) {
    log.error('Failed to initialize frontend log level sync', error);
  }
}

export async function installFrontendLogLevelConfigWatcher(): Promise<void> {
  if (configWatcherInstalled) {
    return;
  }

  configWatcherInstalled = true;

  try {
    const { configManager } = await import('./ConfigManager');
    configManager.onConfigChange((path, _oldValue, newValue) => {
      if (path === LOGGING_LEVEL_PATH) {
        applyFrontendLogLevel(typeof newValue === 'string' ? newValue : undefined, 'config_change');
        return;
      }

      if (path === LOGGING_INCLUDE_SENSITIVE_PATH) {
        setIncludeSensitiveDiagnostics(typeof newValue === 'boolean' ? newValue : true);
      }
    });
  } catch (error) {
    configWatcherInstalled = false;
    log.error('Failed to install frontend log level config watcher', error);
  }
}
