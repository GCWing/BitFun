// Token usage API client

import { invoke } from '@tauri-apps/api/core';

export interface TokenUsageRecord {
  model_id: string;
  session_id: string;
  turn_id: string;
  timestamp: string;
  input_tokens: number;
  output_tokens: number;
  cached_tokens: number;
  total_tokens: number;
}

export interface ModelTokenStats {
  model_id: string;
  total_input: number;
  total_output: number;
  total_cached: number;
  total_tokens: number;
  session_count: number;
  request_count: number;
  first_used: string | null;
  last_used: string | null;
}

export interface SessionTokenStats {
  session_id: string;
  model_id: string;
  total_input: number;
  total_output: number;
  total_cached: number;
  total_tokens: number;
  request_count: number;
  created_at: string;
  last_updated: string;
}

export type TimeRange = 
  | 'Today'
  | 'ThisWeek'
  | 'ThisMonth'
  | 'All'
  | { Custom: { start: string; end: string } };

export type StatsTimeRange = 'Last7Days' | 'Last30Days' | 'All';

export interface TokenUsageSummary {
  total_input: number;
  total_output: number;
  total_cached: number;
  total_tokens: number;
  by_model: Record<string, ModelTokenStats>;
  by_session: Record<string, SessionTokenStats>;
  record_count: number;
}

export const tokenUsageApi = {
  /**
   * Convert StatsTimeRange to a TimeRange with custom date calculation
   */
  _toTimeRange(range: StatsTimeRange): TimeRange | undefined {
    if (range === 'All') return undefined;
    const now = new Date();
    const start = new Date();
    if (range === 'Last7Days') {
      start.setDate(now.getDate() - 7);
    } else if (range === 'Last30Days') {
      start.setDate(now.getDate() - 30);
    }
    return { Custom: { start: start.toISOString(), end: now.toISOString() } };
  },

  /**
   * Get token statistics for a specific model
   */
  async getModelStats(
    modelId: string,
    statsTimeRange?: StatsTimeRange,
    includeSubagent?: boolean
  ): Promise<ModelTokenStats | null> {
    const timeRange = statsTimeRange ? this._toTimeRange(statsTimeRange) : undefined;
    const needFiltered = timeRange !== undefined || includeSubagent;
    return invoke('get_model_token_stats', {
      request: {
        model_id: modelId,
        time_range: needFiltered ? (timeRange ?? 'All') : undefined,
        include_subagent: includeSubagent ?? false,
      }
    });
  },

  /**
   * Get token statistics for all models
   */
  async getAllModelStats(): Promise<Record<string, ModelTokenStats>> {
    const response = await invoke<{ stats: Record<string, ModelTokenStats> }>('get_all_model_token_stats', {});
    return response.stats;
  },

  /**
   * Get token statistics for a specific session
   */
  async getSessionStats(sessionId: string): Promise<SessionTokenStats | null> {
    return invoke('get_session_token_stats', {
      request: { session_id: sessionId }
    });
  },

  /**
   * Query token usage with filters
   */
  async queryTokenUsage(
    modelId?: string,
    sessionId?: string,
    timeRange: TimeRange = 'All',
    limit?: number,
    offset?: number,
    includeSubagent?: boolean
  ): Promise<TokenUsageSummary> {
    return invoke('query_token_usage', {
      request: {
        model_id: modelId,
        session_id: sessionId,
        time_range: timeRange,
        limit,
        offset,
        include_subagent: includeSubagent ?? false,
      }
    });
  },

  /**
   * Clear token statistics for a specific model
   */
  async clearModelStats(modelId: string): Promise<void> {
    return invoke('clear_model_token_stats', {
      request: { model_id: modelId }
    });
  },

  /**
   * Clear all token statistics
   */
  async clearAllStats(): Promise<void> {
    return invoke('clear_all_token_stats', {});
  }
};

