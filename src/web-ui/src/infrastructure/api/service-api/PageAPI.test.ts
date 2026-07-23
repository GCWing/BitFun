import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({ api: { invoke } }));
vi.mock('../errors/TauriCommandError', () => ({
  createTauriCommandError: (_command: string, error: unknown) => error,
}));

import { pageAPI } from './PageAPI';

describe('PageAPI', () => {
  beforeEach(() => invoke.mockReset());

  it('includes the Page generation in version reads and secure open-link requests', async () => {
    const generation = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    invoke.mockResolvedValueOnce([]);
    invoke.mockResolvedValue({
      open_url: 'https://relay.test/api/page-open/ticket',
      page_url: 'https://relay.test/p/alice/demo/@v/v1',
      expires_in_seconds: 60,
    });

    await pageAPI.listVersions('demo', generation);
    await pageAPI.createOpenLink('demo', generation, 'v1');

    expect(invoke).toHaveBeenNthCalledWith(1, 'page_list_versions', {
      request: { slug: 'demo', generation },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'page_create_open_link', {
      request: { slug: 'demo', generation, version_id: 'v1' },
    });
  });

  it('uses an explicit null version for a production open link', async () => {
    const generation = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    invoke.mockResolvedValue({
      open_url: 'https://relay.test/api/page-open/ticket',
      page_url: 'https://relay.test/p/alice/demo',
      expires_in_seconds: 60,
    });

    await pageAPI.createOpenLink('demo', generation);

    expect(invoke).toHaveBeenCalledWith('page_create_open_link', {
      request: { slug: 'demo', generation, version_id: null },
    });
  });

  it('generation-fences every Page mutation and keeps unpublish separate from deletion', async () => {
    const generation = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    invoke.mockResolvedValue(undefined);

    await pageAPI.deploy('demo', generation, 'v1');
    await pageAPI.update('demo', generation, { title: 'Renamed', visibility: 'relay' });
    await pageAPI.deleteVersion('demo', generation, 'v0');
    await pageAPI.unpublish('demo', generation);
    await pageAPI.deletePage('demo', generation);

    expect(invoke).toHaveBeenNthCalledWith(1, 'page_deploy', {
      request: { slug: 'demo', generation, version_id: 'v1' },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'page_update', {
      request: { slug: 'demo', generation, title: 'Renamed', visibility: 'relay' },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'page_delete_version', {
      request: { slug: 'demo', generation, version_id: 'v0' },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, 'page_unpublish', {
      request: { slug: 'demo', generation },
    });
    expect(invoke).toHaveBeenNthCalledWith(5, 'page_delete', {
      request: { slug: 'demo', generation },
    });
  });
});
