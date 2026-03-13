import { create } from 'zustand';
import { listen } from '@tauri-apps/api/event';
import { insightsApi, type InsightsReport, type InsightsReportMeta, type InsightsProgressEvent } from '@/infrastructure/api/insightsApi';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('InsightsStore');

const RETRY_STAGES = new Set(['facet_retry', 'recommendations_retry']);

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
  error: string;
  loadingMetas: boolean;

  setSelectedDays: (days: number) => void;
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
  error: '',
  loadingMetas: false,

  setSelectedDays: (days) => set({ selectedDays: days }),

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
    const { selectedDays, generating } = get();
    if (generating) return;

    set({
      generating: true,
      error: '',
      progress: { ...defaultProgress, message: 'Starting...' },
    });

    const unlisten = await listen<InsightsProgressEvent>('insights-progress', (event) => {
      const { message, stage, current, total } = event.payload;
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

    try {
      const report = await insightsApi.generateInsights(selectedDays);
      log.info('Insights report generated', {
        sessions: report.total_sessions,
        analyzed: report.analyzed_sessions,
      });
      set({
        currentReport: report,
        view: 'report',
        generating: false,
        progress: { ...defaultProgress },
      });
      get().fetchReportMetas();
    } catch (err) {
      log.error('Failed to generate insights', err);
      set({
        generating: false,
        view: 'list',
        error: String(err),
        progress: { ...defaultProgress },
      });
    } finally {
      unlisten();
    }
  },

  cancelGeneration: async () => {
    if (!get().generating) return;
    try {
      await insightsApi.cancelGeneration();
    } catch (err) {
      log.error('Failed to cancel insights generation', err);
    }
    set({
      generating: false,
      progress: { ...defaultProgress },
    });
  },

  backToList: () => set({ view: 'list', currentReport: null }),

  clearError: () => set({ error: '' }),
}));
