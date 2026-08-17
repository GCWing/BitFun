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
  /** IANA time zone used for local-calendar ranges and trend buckets. */
  timeZone?: string;
  includeSubagent?: boolean;
}

export interface UsageStatisticsEntry {
  name: string;
  requests: number;
  tokens: number;
  /** Cache hit ratio (0.0..=1.0) when any request reported cache telemetry. */
  cacheHitRate: number | null;
}

export interface UsageTrendPoint {
  /** Bucket start as an ISO timestamp; alignment uses the requested time zone. */
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
  /** Prompt input tokens from requests that reported cache telemetry. */
  totalCacheReportedInputTokens: number;
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
