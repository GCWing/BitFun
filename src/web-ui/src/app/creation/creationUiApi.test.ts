// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import React from 'react';
import { createRoot } from 'react-dom/client';
import { act } from 'react';
import { createCreationUiApi } from './creationUiApi';
import { setPeerDeviceModeActiveFlag } from '@/infrastructure/peer-device/peerModeFlag';

vi.mock('@/infrastructure/api/service-api/ProductControlAPI', () => ({ productControlAPI: { open: vi.fn(), get: vi.fn(), execute: vi.fn(), configure: vi.fn() } }));
vi.mock('@/infrastructure/api/service-api/MiniAppAPI', () => ({ miniAppAPI: { getMiniApp: vi.fn(async () => ({})) } }));
vi.mock('../stores/sceneStore', () => ({ useSceneStore: { getState: () => ({ activeTabId: 'session', openScene: vi.fn() }), subscribe: () => vi.fn() } }));

afterEach(() => { setPeerDeviceModeActiveFlag(false); document.body.innerHTML = ''; });

describe('Creation UI API', () => {
  it('keeps extension roots across React rerenders and removes them on dispose', async () => {
    const container = document.createElement('div'); document.body.append(container);
    const root = createRoot(container);
    const shell = (label: string) => React.createElement('div', null,
      React.createElement('span', null, label),
      React.createElement('div', { 'data-openbitfun-creation-slot': 'sidebar-footer' }));
    await act(async () => root.render(shell('before')));
    const creation = createCreationUiApi(new AbortController().signal);
    const mount = creation.api.mount('sidebar-footer'); mount.textContent = 'Custom action';
    await act(async () => root.render(shell('after')));
    expect(container.textContent).toBe('afterCustom action');
    creation.dispose(); expect(mount.isConnected).toBe(false);
    await act(async () => root.unmount());
  });

  it('rejects stale and Peer calls before they reach product APIs', async () => {
    const controller = new AbortController();
    const creation = createCreationUiApi(controller.signal);
    setPeerDeviceModeActiveFlag(true);
    expect(() => creation.api.control.open('feature.miniapps')).toThrow('Peer');
    await expect(creation.api.openMiniApp('app-1')).rejects.toThrow('Peer');
    setPeerDeviceModeActiveFlag(false); controller.abort();
    expect(() => creation.api.control.get('feature.miniapps')).toThrow();
  });
});
