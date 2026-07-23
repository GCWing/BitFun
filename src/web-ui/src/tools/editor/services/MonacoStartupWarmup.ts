import {
  backgroundTaskScheduler,
  type BackgroundTaskHandle,
} from '@/shared/utils/backgroundTaskScheduler';
import { createLogger } from '@/shared/utils/logger';
import { startupTrace } from '@/shared/utils/startupTrace';

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
  preloadEditorSurfaceStages?: EditorWarmupStage[];
  waitForIdle?: (signal: AbortSignal) => Promise<void>;
  trace?: {
    markPhase: (phase: string, data?: Record<string, unknown>) => void;
  };
}

interface EditorWarmupStage {
  name: string;
  run: () => Promise<void>;
}

async function defaultInitializeMonaco(): Promise<void> {
  const { MonacoManager } = await import('./MonacoInitManager');
  await MonacoManager.initialize();
}

async function defaultInitializeThemeSync(): Promise<void> {
  const { monacoThemeSync } = await import('@/infrastructure/theme/integrations/MonacoThemeSync');
  await monacoThemeSync.initialize();
}

const defaultEditorSurfaceStages: EditorWarmupStage[] = [
  {
    name: 'code_editor',
    run: async () => {
      await import('@/tools/editor/components/CodeEditor');
    },
  },
  {
    name: 'diff_editor',
    run: async () => {
      await import('@/tools/editor/components/DiffEditor');
    },
  },
  {
    name: 'git_diff_editor',
    run: async () => {
      await import('@/tools/git/components/GitDiffEditor/GitDiffEditor');
    },
  },
];

function defaultWaitForIdle(signal: AbortSignal): Promise<void> {
  if (signal.aborted) {
    return Promise.resolve();
  }

  return new Promise(resolve => {
    let settled = false;
    let cancelScheduled: (() => void) | null = null;

    const finish = () => {
      if (settled) {
        return;
      }
      settled = true;
      signal.removeEventListener('abort', finish);
      cancelScheduled?.();
      resolve();
    };

    signal.addEventListener('abort', finish, { once: true });

    const requestIdleCallback = (globalThis as {
      requestIdleCallback?: (callback: () => void, options?: { timeout?: number }) => number;
    }).requestIdleCallback;
    const cancelIdleCallback = (globalThis as {
      cancelIdleCallback?: (handle: number) => void;
    }).cancelIdleCallback;

    if (typeof requestIdleCallback === 'function') {
      const idleHandle = requestIdleCallback(finish, { timeout: 1500 });
      cancelScheduled = () => cancelIdleCallback?.(idleHandle);
      return;
    }

    const timer = globalThis.setTimeout(finish, 16) as unknown as number;
    cancelScheduled = () => globalThis.clearTimeout(timer);
  });
}

export function scheduleMonacoStartupWarmup(
  options: MonacoStartupWarmupOptions = {}
): BackgroundTaskHandle<void> {
  const scheduler = options.scheduler ?? backgroundTaskScheduler;
  const initializeMonaco = options.initializeMonaco ?? defaultInitializeMonaco;
  const initializeThemeSync = options.initializeThemeSync ?? defaultInitializeThemeSync;
  const preloadEditorSurfaceStages = options.preloadEditorSurfaceStages ?? defaultEditorSurfaceStages;
  const waitForIdle = options.waitForIdle ?? defaultWaitForIdle;
  const trace = options.trace ?? startupTrace;

  trace.markPhase('editor_startup_warmup_scheduled', {
    idle: true,
    priority: 'low',
  });

  return scheduler.schedule(async (signal) => {
    try {
      if (signal.aborted) {
        return;
      }
      trace.markPhase('editor_startup_warmup_start');
      for (const stage of preloadEditorSurfaceStages) {
        trace.markPhase('editor_startup_warmup_stage_start', { stage: stage.name });
        await stage.run();
        if (signal.aborted) {
          return;
        }
        trace.markPhase('editor_startup_warmup_stage_end', { stage: stage.name });
        await waitForIdle(signal);
        if (signal.aborted) {
          return;
        }
      }
      if (signal.aborted) {
        return;
      }
      trace.markPhase('editor_startup_warmup_surfaces_loaded');
      await initializeMonaco();
      if (signal.aborted) {
        return;
      }
      trace.markPhase('editor_startup_warmup_monaco_ready');
      await waitForIdle(signal);
      if (signal.aborted) {
        return;
      }
      await initializeThemeSync();
      if (signal.aborted) {
        return;
      }
      trace.markPhase('editor_startup_warmup_end');
      log.info('Monaco startup warmup completed');
    } catch (error) {
      if (signal.aborted) {
        return;
      }
      trace.markPhase('editor_startup_warmup_failed', {
        error: error instanceof Error ? error.message : String(error),
      });
      log.warn('Monaco startup warmup failed', error);
      throw error;
    }
  }, {
    idle: true,
    inFlightKey: 'startup:monaco-warmup',
    priority: 'low',
  });
}
