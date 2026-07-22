/**
 * @vitest-environment jsdom
 */

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAccountLoginState } from './useAccountLoginState';

const mocks = vi.hoisted(() => ({
  accountStatus: vi.fn(),
  getDeviceInfo: vi.fn(),
  unlisten: vi.fn(),
  loginStateListener: null as null | ((payload: { logged_in: boolean }) => void),
}));

vi.mock('@/infrastructure/api/service-api/RemoteConnectAPI', () => ({
  remoteConnectAPI: {
    accountStatus: mocks.accountStatus,
    getDeviceInfo: mocks.getDeviceInfo,
  },
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: {
    listen: vi.fn((_event: string, listener: (payload: { logged_in: boolean }) => void) => {
      mocks.loginStateListener = listener;
      return mocks.unlisten;
    }),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ warn: vi.fn() }),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => { resolve = res; });
  return { promise, resolve };
}

function AccountStateHarness(): React.ReactElement {
  const state = useAccountLoginState();
  return (
    <div
      data-logged-in={String(state.loggedIn)}
      data-device-name={state.deviceName ?? ''}
    />
  );
}

describe('useAccountLoginState request ownership', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.accountStatus.mockReset();
    mocks.getDeviceInfo.mockReset();
    mocks.unlisten.mockReset();
    mocks.loginStateListener = null;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('does not let an older logged-in probe overwrite a newer logout event', async () => {
    const oldDeviceInfo = deferred<{ device_name: string }>();
    mocks.accountStatus
      .mockResolvedValueOnce({ logged_in: true, user_id: 'user-a' })
      .mockResolvedValueOnce({ logged_in: false, user_id: null });
    mocks.getDeviceInfo.mockImplementationOnce(() => oldDeviceInfo.promise);

    await act(async () => {
      root.render(<AccountStateHarness />);
    });
    await vi.waitFor(() => expect(mocks.getDeviceInfo).toHaveBeenCalledTimes(1));

    await act(async () => {
      mocks.loginStateListener?.({ logged_in: false });
    });
    await vi.waitFor(() => {
      expect(mocks.accountStatus).toHaveBeenCalledTimes(2);
      expect(container.firstElementChild?.getAttribute('data-logged-in')).toBe('false');
    });

    await act(async () => {
      oldDeviceInfo.resolve({ device_name: 'Old account device' });
      await oldDeviceInfo.promise;
    });

    expect(container.firstElementChild?.getAttribute('data-logged-in')).toBe('false');
    expect(container.firstElementChild?.getAttribute('data-device-name')).toBe('');
  });
});
