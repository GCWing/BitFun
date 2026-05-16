import {
  backgroundTaskScheduler,
  type BackgroundTaskHandle,
} from '@/shared/utils/backgroundTaskScheduler';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MonacoStartupWarmup');

interface SchedulerLike {
  schedule<T>(
    task: (signal: AbortSignal) => Promise<T> | T,
    options: {
      idle: boolean;
      inFlightKey: string;
      priority: 'low';
    }
  ): BackgroundTaskHandle<T>;
}

interface MonacoStartupWarmupOptions {
  scheduler?: SchedulerLike;
  initializeMonaco?: () => Promise<void>;
  initializeThemeSync?: () => Promise<void>;
}

async function defaultInitializeMonaco(): Promise<void> {
  const { MonacoManager } = await import('./MonacoInitManager');
  await MonacoManager.initialize();
}

async function defaultInitializeThemeSync(): Promise<void> {
  const { monacoThemeSync } = await import('@/infrastructure/theme/integrations/MonacoThemeSync');
  await monacoThemeSync.initialize();
}

export function scheduleMonacoStartupWarmup(
  options: MonacoStartupWarmupOptions = {}
): BackgroundTaskHandle<void> {
  const scheduler = options.scheduler ?? backgroundTaskScheduler;
  const initializeMonaco = options.initializeMonaco ?? defaultInitializeMonaco;
  const initializeThemeSync = options.initializeThemeSync ?? defaultInitializeThemeSync;

  return scheduler.schedule(async (signal) => {
    try {
      await initializeMonaco();
      if (signal.aborted) {
        return;
      }
      await initializeThemeSync();
      log.info('Monaco startup warmup completed');
    } catch (error) {
      if (signal.aborted) {
        return;
      }
      log.warn('Monaco startup warmup failed', error);
      throw error;
    }
  }, {
    idle: true,
    inFlightKey: 'startup:monaco-warmup',
    priority: 'low',
  });
}
