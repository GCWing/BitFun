import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProductControlAPI } from './ProductControlAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: { invoke: invokeMock },
}));

describe('ProductControlAPI', () => {
  beforeEach(() => invokeMock.mockReset());

  it('uses stable IDs and the structured Desktop command contract', async () => {
    invokeMock.mockResolvedValueOnce({ effectiveValue: true, revision: 8 });

    await new ProductControlAPI().configure(
      'setting.application.general',
      'prevent-sleep',
      true,
    );

    expect(invokeMock).toHaveBeenCalledWith('product_control_invoke', {
      request: {
        action: 'configure',
        capabilityId: 'setting.application.general',
        optionId: 'prevent-sleep',
        value: true,
      },
    });
  });

  it('does not expose an arbitrary config path or Tauri command escape hatch', () => {
    const api = new ProductControlAPI() as unknown as Record<string, unknown>;

    expect(api.setConfig).toBeUndefined();
    expect(api.invokeCommand).toBeUndefined();
  });
});
