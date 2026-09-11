/** @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAccountLoginState } from './useAccountLoginState';

const mocks = vi.hoisted(() => ({
  identity: { status: 'signed-in', me: { user: { githubId: 1 } } },
  getDeviceInfo: vi.fn(),
}));
vi.mock('@/infrastructure/account-identity', () => ({ useAccountIdentity: () => mocks.identity }));
vi.mock('@/infrastructure/api/service-api/RemoteConnectAPI', () => ({
  remoteConnectAPI: { getDeviceInfo: mocks.getDeviceInfo },
}));
function Harness() {
  const state = useAccountLoginState();
  return <div data-logged-in={String(state.loggedIn)} data-device-name={state.deviceName ?? ''} />;
}
let container: HTMLDivElement;
let root: Root;
const render = () => act(async () => { root.render(<Harness />); });
const attr = (name: string) => container.firstElementChild?.getAttribute(name);
beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  mocks.identity = { status: 'signed-in', me: { user: { githubId: 1 } } };
  mocks.getDeviceInfo.mockReset().mockRejectedValue(new Error('Relay unavailable'));
  container = document.createElement('div'); document.body.append(container); root = createRoot(container);
});
afterEach(() => { act(() => root.unmount()); container.remove(); });
describe('shared GitHub identity', () => {
  it('remains signed in when Relay device information is unavailable', async () => {
    await render();
    expect(attr('data-logged-in')).toBe('true');
    expect(attr('data-device-name')).toBe('');
  });
  it('ignores a pending device reply after logout', async () => {
    let resolve!: (value: { device_name: string }) => void;
    mocks.getDeviceInfo.mockReturnValue(new Promise(res => { resolve = res; }));
    await render();
    mocks.identity.status = 'signed-out'; await render();
    await act(async () => { resolve({ device_name: 'Previous device' }); });
    expect(attr('data-logged-in')).toBe('false');
    expect(attr('data-device-name')).toBe('');
  });
  it('does not show the previous identity device name after switching identities', async () => {
    mocks.getDeviceInfo.mockResolvedValueOnce({ device_name: 'First device' });
    await render(); expect(attr('data-device-name')).toBe('First device');
    mocks.identity.me.user.githubId = 2; await render();
    expect(attr('data-logged-in')).toBe('true');
    expect(attr('data-device-name')).toBe('');
  });
});
