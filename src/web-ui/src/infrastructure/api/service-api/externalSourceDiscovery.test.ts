import { describe, expect, it } from 'vitest';
import { externalSourceDiscoveryPollDelay } from './externalSourceDiscovery';

describe('external source discovery polling', () => {
  it('uses one shared bounded backoff schedule', () => {
    expect([0, 1, 2, 3, 8].map(externalSourceDiscoveryPollDelay))
      .toEqual([750, 1_500, 3_000, 5_000, 5_000]);
  });
});
