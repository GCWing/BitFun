import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./service-api/ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

import {
  TokenUsageStatisticsUnavailableError,
  tokenUsageStatisticsApi,
  type TokenUsageStatisticsRequest,
  type UsageStatistics,
} from './tokenUsageStatisticsApi';

const REQUEST: TokenUsageStatisticsRequest = {
  timeRange: 'last24Hours',
  granularity: 'hour',
  timeZone: 'Asia/Shanghai',
};

const STATISTICS: UsageStatistics = {
  totalRequests: 0,
  totalTokens: 0,
  totalInputTokens: 0,
  totalOutputTokens: 0,
  totalCachedTokens: 0,
  totalCacheWriteTokens: 0,
  totalCacheReportedInputTokens: 0,
  byModel: [],
  byGroup: [],
  byEndpoint: [],
  trend: [],
  granularity: 'hour',
};

describe('tokenUsageStatisticsApi', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('uses the shared ApiClient so the active surface transport is honored', async () => {
    invokeMock.mockResolvedValue(STATISTICS);

    await expect(tokenUsageStatisticsApi.getStatistics(REQUEST)).resolves.toBe(STATISTICS);

    expect(invokeMock).toHaveBeenCalledWith('get_token_usage_statistics', {
      request: REQUEST,
    });
  });

  it('classifies an older Peer host as explicitly unavailable', async () => {
    invokeMock.mockRejectedValue(new Error(
      'token_usage_statistics_unsupported: The connected Peer host does not support usage statistics',
    ));

    await expect(tokenUsageStatisticsApi.getStatistics(REQUEST)).rejects.toBeInstanceOf(
      TokenUsageStatisticsUnavailableError,
    );
  });
});
