import { invoke } from '@tauri-apps/api/core';

// ============ Types (strict 1:1 mirror of Rust types) ============

export type UsageTimeRange =
  | 'last24Hours'
  | 'today'
  | 'thisWeek'
  | 'thisMonth'
  | 'all'
  | 'custom';

export type UsageGranularity = 'hour' | 'day';

export interface TokenUsageStatisticsRequest {
  timeRange: UsageTimeRange;
  granularity: UsageGranularity;
  /** ISO timestamp; required when timeRange === 'custom'. */
  start?: string;
  end?: string;
  includeSubagent?: boolean;
}

export interface UsageStatisticsEntry {
  name: string;
  requests: number;
  tokens: number;
  /** Estimated cost in USD. */
  cost: number;
}

export interface UsageTrendPoint {
  /** Bucket start (ISO timestamp, UTC). */
  bucket: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  /** 0.0..=1.0 when the bucket has cache telemetry. */
  cacheHitRate: number | null;
}

export interface UsageStatistics {
  totalRequests: number;
  totalTokens: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCachedTokens: number;
  totalCacheWriteTokens: number;
  totalCost: number;
  byModel: UsageStatisticsEntry[];
  byGroup: UsageStatisticsEntry[];
  byEndpoint: UsageStatisticsEntry[];
  trend: UsageTrendPoint[];
  granularity: UsageGranularity;
}

// ============ API client ============

export const tokenUsageStatisticsApi = {
  async getStatistics(
    request: TokenUsageStatisticsRequest
  ): Promise<UsageStatistics> {
    return invoke('get_token_usage_statistics', { request });
  },
};
