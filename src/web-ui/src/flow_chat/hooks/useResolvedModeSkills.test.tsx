/** @vitest-environment jsdom */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModeSkillInfo } from '@/infrastructure/config/types';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { useResolvedModeSkills } from './useResolvedModeSkills';

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: { getModeSkillScanReport: vi.fn() },
}));
vi.mock('@/shared/utils/logger', () => ({ createLogger: () => ({ error: vi.fn() }) }));

const skills = [{ name: 'review', key: 'review' }] as ModeSkillInfo[];
function deferred() {
  let resolve!: (value: ModeSkillInfo[]) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<ModeSkillInfo[]>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

describe('useResolvedModeSkills', () => {
  let root: Root;
  let container: HTMLDivElement;
  let latest: ReturnType<typeof useResolvedModeSkills>;
  let props: Parameters<typeof useResolvedModeSkills>[0];
  const requests: ReturnType<typeof deferred>[] = [];
  function Probe() {
    latest = useResolvedModeSkills(props);
    return null;
  }
  async function render(update = {}) {
    props = { ...props, ...update };
    await act(async () => root.render(<Probe />));
  }
  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    root = createRoot(container);
    props = { enabled: true, surfaceEpoch: 1, modeId: 'agent', workspacePath: '/project' };
    requests.length = 0;
    vi.mocked(configAPI.getModeSkillScanReport).mockReset().mockImplementation(() => {
      const request = deferred();
      requests.push(request);
      return request.promise.then(skills => ({ skills, diagnostics: [], diagnosticsAvailable: true }));
    });
  });
  afterEach(async () => { await act(async () => root.unmount()); });

  it('retains a pending request across close/reopen and accepts completion while closed', async () => {
    await render();
    expect(latest.loading).toBe(true);
    await render({ enabled: false });
    await render({ enabled: true });
    expect(requests).toHaveLength(1);
    await render({ enabled: false });
    await act(async () => requests[0].resolve(skills));
    expect(latest.skills).toEqual(skills);
    await render({ enabled: true });
    expect(requests).toHaveLength(2);
    expect(latest.skills).toEqual(skills);
    expect(latest.hasLoaded).toBe(true);
    expect(latest.loading).toBe(true);
    await act(async () => requests[1].resolve([]));
    expect(latest.skills).toEqual([]);
    expect(latest.hasLoaded).toBe(true);
  });

  it.each([
    { workspacePath: '/different' },
    { modeId: 'plan' },
    { connectionId: 'other-ssh-host' },
    { surfaceEpoch: 2 },
  ])('isolates scope changes %j and ignores late responses', async update => {
    await render();
    await render(update);
    expect(requests).toHaveLength(2);
    await act(async () => requests[0].resolve(skills));
    expect(latest.skills).toEqual([]);
    expect(latest.hasLoaded).toBe(false);
    await act(async () => requests[1].resolve([]));
    expect(latest.skills).toEqual([]);
    expect(latest.hasLoaded).toBe(true);
  });

  it('hides cached data on a scope switch while closed', async () => {
    await render();
    await act(async () => requests[0].resolve(skills));
    await render({ enabled: false, surfaceEpoch: 2 });
    expect(latest.skills).toEqual([]);
    expect(requests).toHaveLength(1);
  });

  it('does not fetch until enabled and retries errors without discarding a pending request', async () => {
    await render({ enabled: false });
    expect(requests).toHaveLength(0);
    await render({ enabled: true });
    await act(async () => latest.retry());
    expect(requests).toHaveLength(1);
    await act(async () => requests[0].reject(new Error('offline')));
    expect(latest.failed).toBe(true);
    expect(latest.loading).toBe(false);
    await act(async () => latest.retry());
    expect(requests).toHaveLength(2);
    await act(async () => requests[1].resolve(skills));
    expect(latest.failed).toBe(false);
    expect(latest.skills).toEqual(skills);
  });

  it('surfaces background refresh failure instead of retaining stale policy indefinitely', async () => {
    await render();
    await act(async () => requests[0].resolve(skills));
    await render({ enabled: false });
    await render({ enabled: true });
    await act(async () => requests[1].reject(new Error('offline')));
    expect(latest.skills).toEqual([]);
    expect(latest.failed).toBe(true);
  });
});
