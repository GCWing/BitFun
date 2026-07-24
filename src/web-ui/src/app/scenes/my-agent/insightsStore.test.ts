import { beforeEach, describe, expect, it, vi } from 'vitest';

const apiMocks = vi.hoisted(() => ({
  generateInsights: vi.fn(),
  getLatestInsights: vi.fn().mockResolvedValue([]),
  loadReport: vi.fn(),
  hasInsightsData: vi.fn(),
  cancelGeneration: vi.fn().mockResolvedValue(undefined),
  listenProgress: vi.fn().mockResolvedValue(vi.fn()),
  openReport: vi.fn(),
}));

vi.mock('@/infrastructure/api/insightsApi', () => ({
  insightsApi: apiMocks,
}));

import type { InsightsProgressEvent, InsightsReport } from '@/infrastructure/api/insightsApi';
import { useInsightsStore } from './insightsStore';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function report(generatedAt: number): InsightsReport {
  return {
    generated_at: generatedAt,
    date_range: { start: '2026-07-01T00:00:00Z', end: '2026-07-02T00:00:00Z' },
    total_sessions: 1,
    analyzed_sessions: 1,
    total_messages: 2,
    days_covered: 2,
    session_usage: {
      input_tokens: 0,
      output_tokens: 0,
      total_tokens: 0,
      turns_with_usage: 0,
      output_reported_turns: 0,
      total_turns: 0,
    },
    generation_usage: {
      input_tokens: 0,
      output_tokens: 0,
      total_tokens: 0,
      reasoning_tokens: 0,
      cached_input_tokens: 0,
      cache_creation_input_tokens: 0,
      model_calls: 0,
      reported_model_calls: 0,
    },
    generation_models: [],
    stats: {
      total_hours: 1,
      msgs_per_day: 1,
      top_tools: [],
      top_goals: [],
      outcomes: {},
      satisfaction: {},
      session_types: {},
      languages: {},
      hour_counts: {},
      agent_types: {},
      response_time_buckets: {},
      median_response_time_secs: null,
      avg_response_time_secs: null,
      friction: {},
      success: {},
      tool_errors: {},
      total_lines_added: 0,
      total_lines_removed: 0,
      total_files_modified: 0,
    },
    at_a_glance: {
      whats_working: '',
      whats_hindering: '',
      quick_wins: '',
      looking_ahead: '',
    },
    interaction_style: { narrative: '', key_patterns: [] },
    project_areas: [],
    wins_intro: '',
    big_wins: [],
    friction_intro: '',
    friction_categories: [],
    suggestions: {
      bitfun_md_additions: [],
      features_to_try: [],
      usage_patterns: [],
    },
    horizon_intro: '',
    on_the_horizon: [],
    fun_ending: null,
    html_report_path: null,
  };
}

describe('insightsStore generation lifecycle', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    apiMocks.getLatestInsights.mockResolvedValue([]);
    apiMocks.cancelGeneration.mockResolvedValue(undefined);
    apiMocks.listenProgress.mockResolvedValue(vi.fn());
    useInsightsStore.setState({
      view: 'list',
      reportMetas: [],
      currentReport: null,
      generating: false,
      activeGenerationRunId: null,
      progress: { stage: '', message: '', current: 0, total: 0, isRetrying: false },
      selectedDays: 30,
      selectedModel: 'auto',
      error: '',
      loadingMetas: false,
    });
  });

  it('ignores completion from a cancelled generation after a new run starts', async () => {
    const first = deferred<InsightsReport>();
    const second = deferred<InsightsReport>();
    apiMocks.generateInsights
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    const firstRun = useInsightsStore.getState().generateReport();
    await vi.waitFor(() => expect(apiMocks.generateInsights).toHaveBeenCalledTimes(1));
    await useInsightsStore.getState().cancelGeneration();

    const secondRun = useInsightsStore.getState().generateReport();
    await vi.waitFor(() => expect(apiMocks.generateInsights).toHaveBeenCalledTimes(2));

    first.reject(new Error('cancelled'));
    await firstRun;
    expect(useInsightsStore.getState().generating).toBe(true);
    expect(useInsightsStore.getState().error).toBe('');

    second.resolve(report(2));
    await secondRun;
    expect(useInsightsStore.getState().generating).toBe(false);
    expect(useInsightsStore.getState().currentReport?.generated_at).toBe(2);
    expect(useInsightsStore.getState().view).toBe('report');
  });

  it('marks backend analysis retries as retrying progress', async () => {
    const pendingReport = deferred<InsightsReport>();
    let progressHandler: ((event: InsightsProgressEvent) => void) | undefined;
    apiMocks.generateInsights.mockImplementationOnce(() => pendingReport.promise);
    apiMocks.listenProgress.mockImplementationOnce(async (handler) => {
      progressHandler = handler;
      return vi.fn();
    });

    const run = useInsightsStore.getState().generateReport();
    await vi.waitFor(() => expect(progressHandler).toBeDefined());
    progressHandler?.({
      stage: 'analysis_retry',
      message: 'Retrying suggestions...',
      current: 0,
      total: 0,
    });

    expect(useInsightsStore.getState().progress.isRetrying).toBe(true);
    await useInsightsStore.getState().cancelGeneration();
    pendingReport.reject(new Error('cancelled'));
    await run;
  });

  it('passes the selected model to report generation', async () => {
    const pendingReport = deferred<InsightsReport>();
    apiMocks.generateInsights.mockImplementationOnce(() => pendingReport.promise);
    useInsightsStore.getState().setSelectedModel('model-insights');

    const run = useInsightsStore.getState().generateReport();
    await vi.waitFor(() => {
      expect(apiMocks.generateInsights).toHaveBeenCalledWith(30, 'model-insights');
    });

    await useInsightsStore.getState().cancelGeneration();
    pendingReport.reject(new Error('cancelled'));
    await run;
  });
});
