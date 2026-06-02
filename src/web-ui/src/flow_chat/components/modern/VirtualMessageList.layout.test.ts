import { describe, expect, it } from 'vitest';
import { getVirtualMessageDefaultItemHeight } from './virtualMessageListLayout';

describe('getVirtualMessageDefaultItemHeight', () => {
  it('keeps compact historical projections on the small row estimate', () => {
    expect(getVirtualMessageDefaultItemHeight({
      isHistorical: false,
      hasCompactHistoricalProjection: true,
      hasInitialHistoryModelRoundProjection: true,
    })).toBe(72);
  });

  it('uses a taller initial estimate for partial historical model rounds', () => {
    expect(getVirtualMessageDefaultItemHeight({
      isHistorical: false,
      hasCompactHistoricalProjection: false,
      hasInitialHistoryModelRoundProjection: true,
    })).toBeGreaterThan(200);
  });

  it('keeps live sessions on the legacy estimate', () => {
    expect(getVirtualMessageDefaultItemHeight({
      isHistorical: false,
      hasCompactHistoricalProjection: false,
      hasInitialHistoryModelRoundProjection: false,
    })).toBe(200);
  });
});
