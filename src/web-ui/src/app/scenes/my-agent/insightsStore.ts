import { create } from 'zustand';
import { insightsApi, type InsightsReport, type InsightsReportMeta, type InsightsProgressEvent } from '@/infrastructure/api/insightsApi';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('InsightsStore');

const RETRY_STAGES = new Set(['facet_retry', 'analysis_retry']);
let nextGenerationRunId = 0;

export type InsightsView = 'list' | 'report';

interface InsightsProgress {
  stage: string;
  message: string;
  current: number;
  total: number;
  isRetrying: boolean;
}

interface InsightsState {
  view: InsightsView;
  reportMetas: InsightsReportMeta[];
  currentReport: InsightsReport | null;
  generating: boolean;
  progress: InsightsProgress;
  selectedDays: number;
  selectedModel: string;
  error: string;
  loadingMetas: boolean;
  activeGenerationRunId: number | null;

  setSelectedDays: (days: number) => void;
  setSelectedModel: (modelId: string) => void;
  fetchReportMetas: () => Promise<void>;
  loadReport: (meta: InsightsReportMeta) => Promise<void>;
  generateReport: () => Promise<void>;
  cancelGeneration: () => Promise<void>;
  backToList: () => void;
  clearError: () => void;
}

const defaultProgress: InsightsProgress = {
  stage: '',
  message: '',
  current: 0,
  total: 0,
  isRetrying: false,
};

export const useInsightsStore = create<InsightsState>((set, get) => ({
  view: 'list',
  reportMetas: [],
  currentReport: null,
  generating: false,
  progress: { ...defaultProgress },
  selectedDays: 30,
  selectedModel: 'auto',
  error: '',
  loadingMetas: false,
  activeGenerationRunId: null,

  setSelectedDays: (days) => set({ selectedDays: days }),
  setSelectedModel: (selectedModel) => set({ selectedModel }),

  fetchReportMetas: async () => {
    set({ loadingMetas: true });
    try {
      const metas = await insightsApi.getLatestInsights();
      set({ reportMetas: metas, loadingMetas: false });
    } catch (err) {
      log.error('Failed to fetch report metas', err);
      set({ loadingMetas: false });
    }
  },

  loadReport: async (meta) => {
    try {
      const report = await insightsApi.loadReport(meta.path);
      set({ currentReport: report, view: 'report', error: '' });
    } catch (err) {
      log.error('Failed to load report', err);
      set({ error: String(err) });
    }
  },

  generateReport: async () => {
    const { selectedDays, selectedModel, generating } = get();
    if (generating) return;
    const runId = ++nextGenerationRunId;

    set({
      generating: true,
      activeGenerationRunId: runId,
      error: '',
      progress: { ...defaultProgress, stage: 'starting' },
    });

    let unlisten: (() => void) | undefined;
    try {
      unlisten = await insightsApi.listenProgress((event: InsightsProgressEvent) => {
        if (get().activeGenerationRunId !== runId) return;
        const { message, stage, current, total } = event;
        set({
          progress: {
            stage,
            message,
            current,
            total,
            isRetrying: RETRY_STAGES.has(stage),
          },
        });
      });

      const report = await insightsApi.generateInsights(selectedDays, selectedModel);
      if (get().activeGenerationRunId !== runId) return;
      log.info('Insights report generated', {
        sessions: report.total_sessions,
        analyzed: report.analyzed_sessions,
      });
      set({
        currentReport: report,
        view: 'report',
        generating: false,
        activeGenerationRunId: null,
        progress: { ...defaultProgress },
      });
      get().fetchReportMetas();
    } catch (err) {
      if (get().activeGenerationRunId !== runId) return;
      log.error('Failed to generate insights', err);
      set({
        generating: false,
        activeGenerationRunId: null,
        view: 'list',
        error: String(err),
        progress: { ...defaultProgress },
      });
    } finally {
      unlisten?.();
    }
  },

  cancelGeneration: async () => {
    const runId = get().activeGenerationRunId;
    if (!get().generating || runId == null) return;
    try {
      await insightsApi.cancelGeneration();
    } catch (err) {
      log.error('Failed to cancel insights generation', err);
    }
    if (get().activeGenerationRunId !== runId) return;
    set({
      generating: false,
      activeGenerationRunId: null,
      progress: { ...defaultProgress },
    });
  },

  backToList: () => set({ view: 'list', currentReport: null }),

  clearError: () => set({ error: '' }),
}));
