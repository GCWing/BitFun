import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MiniAppMarketAPI } from './MiniAppMarketAPI';

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock('./ApiClient', () => ({ api: mocks }));

describe('MiniAppMarketAPI account bridge', () => {
  beforeEach(() => {
    mocks.invoke.mockReset();
    mocks.listen.mockReset();
  });

  it('keeps the desktop transaction secret and OAuth tokens out of renderer calls', async () => {
    const market = new MiniAppMarketAPI();
    const transaction = {
      transactionId: 'transaction-1',
      authorizationUrl: 'https://github.com/login/oauth/authorize',
      expiresAt: 1234,
      pollIntervalSeconds: 3,
    };
    mocks.invoke
      .mockResolvedValueOnce(transaction)
      .mockResolvedValueOnce({
        status: 'authorized',
        accessToken: 'must-not-escape-native',
        refreshToken: 'must-not-escape-native',
      });

    const started = await market.authStart();
    const status = await market.authPoll(started);

    expect(started).not.toHaveProperty('transactionSecret');
    expect(status).toBe('authorized');
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, 'miniapp_market_auth_poll', {
      request: { transactionId: 'transaction-1' },
    });
  });

  it('subscribes every market surface to the native account-change event', () => {
    const market = new MiniAppMarketAPI();
    const handler = vi.fn();
    market.onAccountChanged(handler);
    expect(mocks.listen).toHaveBeenCalledWith('miniapp-market-account-changed', handler);
  });
});
