import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({ api: { invoke } }));
vi.mock('../errors/TauriCommandError', () => ({
  createTauriCommandError: (_command: string, error: unknown) => error,
}));

import { pageAPI } from './PageAPI';

describe('PageAPI', () => {
  beforeEach(() => invoke.mockReset());

  it('requests a secure open link without putting credentials in arguments', async () => {
    invoke.mockResolvedValue({ open_url: 'https://relay.test/api/page-open/ticket', expires_in_seconds: 60 });

    await pageAPI.createOpenLink('demo', 'v1');

    expect(invoke).toHaveBeenCalledWith('page_create_open_link', {
      request: { slug: 'demo', version_id: 'v1' },
    });
  });

  it('keeps stopping production separate from destructive Page deletion', async () => {
    invoke.mockResolvedValue(undefined);

    await pageAPI.unpublish('demo');
    await pageAPI.deletePage('demo');

    expect(invoke).toHaveBeenNthCalledWith(1, 'page_unpublish', {
      request: { slug: 'demo' },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'page_delete', {
      request: { slug: 'demo' },
    });
  });
});
