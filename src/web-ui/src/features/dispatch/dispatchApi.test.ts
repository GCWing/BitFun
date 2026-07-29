import { beforeEach, describe, expect, it, vi } from 'vitest';
import { dispatchApi } from './dispatchApi';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: {
    invoke: mocks.invoke,
  },
}));

describe('dispatchApi', () => {
  beforeEach(() => {
    mocks.invoke.mockReset().mockResolvedValue([]);
  });

  it('wraps the target list command in the structured Tauri request contract', async () => {
    await dispatchApi.listTargets();

    expect(mocks.invoke).toHaveBeenCalledWith('dispatch_list_targets', {
      request: {},
    });
  });
});
