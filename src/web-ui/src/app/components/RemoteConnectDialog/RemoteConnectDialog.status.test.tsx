// @vitest-environment jsdom

import React, { act, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { DesignSystemProvider } from '@openbitfun/ui';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConnectionResult, RemoteConnectStatus } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { remoteConnectStatusSource } from '@/infrastructure/remote-connect/remoteConnectStatus';
import DeviceStatusControl from '../NavPanel/components/DeviceStatusControl';
import { RemoteConnectDialog } from './RemoteConnectDialog';
import { setRemoteConnectDisclaimerAgreed } from './remoteConnectDisclaimerStorage';

const boundary = vi.hoisted(() => ({
  backend: null as RemoteConnectStatus | null,
  hasWorkspace: true,
  getStatus: vi.fn(),
  startConnection: vi.fn(),
  stopConnection: vi.fn(),
  stopBot: vi.fn(),
  getFormState: vi.fn(),
  listeners: new Map<string, Set<(payload: unknown) => void>>(),
  jobs: {},
  copyText: vi.fn(),
  t: (key: string, values?: { count?: number; number?: number }) =>
    values?.count !== undefined ? `${key}:${values.count}` : values?.number !== undefined ? `${key}:${values.number}` : key,
}));

vi.mock('@/infrastructure/api/service-api/RemoteConnectAPI', async importOriginal => ({
  ...await importOriginal<typeof import('@/infrastructure/api/service-api/RemoteConnectAPI')>(),
  remoteConnectAPI: {
    getStatus: boundary.getStatus,
    startConnection: boundary.startConnection,
    stopConnection: boundary.stopConnection,
    stopBot: boundary.stopBot,
    getFormState: boundary.getFormState,
    setFormState: vi.fn().mockResolvedValue(undefined),
    getLanNetworkInfo: vi.fn().mockResolvedValue(null),
    getDeviceInfo: vi.fn().mockResolvedValue({ device_id: 'desktop', device_name: 'Workstation', mac_address: '' }),
    accountGetCredentialHint: vi.fn().mockResolvedValue({ username: 'sora', relay_url: 'https://relay.example.test/remote/a' }),
  },
}));
vi.mock('@/shared/utils/textSelection', () => ({ copyTextToClipboard: boundary.copyText }));
vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: {
    listen: (name: string, listener: (payload: unknown) => void) => {
      if (!boundary.listeners.has(name)) boundary.listeners.set(name, new Set());
      boundary.listeners.get(name)!.add(listener);
      return () => { boundary.listeners.get(name)?.delete(listener); };
    },
  },
}));
vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: boundary.t, currentLanguage: 'en-US', formatNumber: String }),
}));
vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({ t: boundary.t, currentLanguage: 'en-US', formatNumber: String }),
}));
vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({ useCurrentWorkspace: () => ({ hasWorkspace: boundary.hasWorkspace }) }));
vi.mock('@/infrastructure/account/useAccountLoginState', () => ({
  useAccountLoginState: () => ({ loggedIn: true, deviceName: 'Workstation' }),
}));
vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({ getAppearanceOverlayHost: () => document.body }));
vi.mock('@/infrastructure/peer-device/peerDeviceContextState', () => ({ usePeerDeviceModeOptional: () => null }));
vi.mock('@/features/dispatch/dispatchJobStore', () => ({ useDispatchJobStore: (select: (value: unknown) => unknown) => select({ jobs: boundary.jobs }) }));
vi.mock('@/shared/notification-system', () => ({ useNotification: () => ({ success: vi.fn(), warning: vi.fn(), error: vi.fn() }) }));
vi.mock('@/infrastructure/confirm-dialog', () => ({ confirmWarning: vi.fn().mockResolvedValue(true) }));
vi.mock('./AccountPanel', () => ({ AccountPanel: () => null }));
vi.mock('@/features/relay-deploy', () => ({ RelayDeployWizard: () => null }));

const relayA = 'https://relay.example.test/remote/a';
const relayB = 'https://relay.example.test/remote/b';

function status(overrides: Partial<RemoteConnectStatus> = {}): RemoteConnectStatus {
  return {
    is_connected: false, pairing_state: 'idle', active_method: null,
    peer_device_name: null, peer_user_id: null,
    bot_connected: 'Weixin (desktop-bot)', bot_verbose_mode: false,
    account_control_connected: false, account_control_relay_url: null,
    ...overrides,
  };
}

function invitation(relay = relayA): ConnectionResult {
  return {
    method: { custom_server: { url: relay } },
    qr_data: null, qr_svg: null,
    qr_url: `https://mobile.example.test/#/pair?relay=${encodeURIComponent(relay)}`,
    bot_pairing_code: null, bot_link: null, pairing_state: 'waiting_for_scan',
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(resolvePromise => { resolve = resolvePromise; });
  return { promise, resolve };
}

function Harness({ initialGroup }: { initialGroup?: 'network' | 'bot' }) {
  const [open, setOpen] = useState(true);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  return <DesignSystemProvider portalHost={document.body}>
    <button data-testid="reopen-remote-connect" onClick={() => setOpen(true)}>Open connections</button>
    <DeviceStatusControl open={sidebarOpen} onOpenChange={setSidebarOpen} onManageDevices={() => setOpen(true)} />
    <RemoteConnectDialog isOpen={open} onClose={() => setOpen(false)} initialGroup={initialGroup} />
  </DesignSystemProvider>;
}

let root: Root;
let container: HTMLDivElement;
let mounted: boolean;

function element(selector: string): HTMLElement {
  const result = document.querySelector<HTMLElement>(selector);
  expect(result, selector).not.toBeNull();
  return result!;
}
const dialog = () => element('[data-openbitfun-component="remote-connect-dialog"][data-openbitfun-part="root"]');
const overviewNetwork = () => element('[data-openbitfun-part="overviewAction"][data-openbitfun-group="network"]');
const cardStatus = () => (document.querySelector('[data-openbitfun-part="pairingCard"] [role="status"]') ?? element('[data-openbitfun-part="connections"] [role="status"]')).textContent;
const attachedMobile = () => document.querySelector('[data-testid="nav-footer-device-status"] [data-openbitfun-device-kind="mobile"]');
const attachedBot = () => document.querySelector('[data-testid="nav-footer-device-status"] [data-openbitfun-device-kind="message-app"]');

async function click(target: HTMLElement) { await act(async () => { target.click(); }); }
async function clickText(key: string) {
  const button = Array.from(dialog().querySelectorAll<HTMLButtonElement>('button')).find(candidate => candidate.textContent?.trim() === key);
  expect(button, key).toBeDefined();
  await click(button!);
}
async function tick(ms = 2000) { await act(async () => { await vi.advanceTimersByTimeAsync(ms); }); }
async function render(initialGroup?: 'network' | 'bot') { await act(async () => { root.render(<Harness initialGroup={initialGroup} />); }); }
async function openNetwork() { await click(overviewNetwork()); }
async function generateInvitation(relay = relayA) {
  await openNetwork();
  await click(element('#remote-connect-network-tab-custom_server'));
  if (relay !== relayA) {
    const input = element('input[placeholder="https://relay.example.com:9700"]') as HTMLInputElement;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, relay);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
  }
  await clickText('remoteConnect.showConnectionCode');
}
async function closeDialog() {
  const close = document.querySelector<HTMLElement>('.openbitfun-remote-connect-dialog__header button');
  expect(close).not.toBeNull();
  await click(close!);
  await tick(300);
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  boundary.listeners.clear();
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
  const storage = new Map<string, string>();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => { storage.set(key, value); },
    removeItem: (key: string) => { storage.delete(key); },
  });
  setRemoteConnectDisclaimerAgreed();
  boundary.hasWorkspace = true;
  boundary.copyText.mockResolvedValue(true);
  boundary.backend = status();
  boundary.getStatus.mockImplementation(async () => ({ ...boundary.backend! }));
  boundary.getFormState.mockResolvedValue({ custom_server_url: relayA });
  boundary.startConnection.mockImplementation(async (_method: string, relay: string) => {
    boundary.backend = { ...boundary.backend!, active_method: `CustomServer { url: "${relay}" }`, pairing_state: 'waiting_for_scan' };
    return invitation(relay);
  });
  boundary.stopConnection.mockImplementation(async () => {
    boundary.backend = { ...boundary.backend!, is_connected: false, pairing_state: 'idle', active_method: null, peer_device_name: null, peer_user_id: null };
  });
  boundary.stopBot.mockImplementation(async () => { boundary.backend = { ...boundary.backend!, bot_connected: null }; });
  remoteConnectStatusSource.invalidate();
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
  mounted = true;
});

afterEach(async () => {
  if (mounted) await act(async () => { root.unmount(); });
  container.remove();
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('Remote Connect shared status through the real dialog and sidebar', () => {
  it.each([
    ['openbitfun_server', 'https://remote.openbitfun.com/relay'],
    ['custom_server', relayA],
  ] as const)('shows the same live clients and relay URL for %s', async (method, relay) => {
    boundary.backend = status({
      account_control_connected: true,
      account_control_relay_url: relay,
      account_control_clients: [
        { id: 'phone', name: 'Safari · iOS' },
        { id: 'browser', name: 'Chrome · Windows' },
      ],
      account_control_has_unidentified_clients: false,
    });
    await render('network');
    const connections = element('[data-openbitfun-part="connections"]');
    expect(connections.textContent).toContain('remoteConnect.clientCount:2');
    expect(connections.querySelectorAll('li')).toHaveLength(2);
    expect(connections.textContent).toContain('Safari · iOS');
    expect(connections.textContent).toContain('Chrome · Windows');
    expect(dialog().querySelectorAll('.openbitfun-remote-connect__network-card')).toHaveLength(1);
    expect(connections.querySelectorAll('input[type="url"]')).toHaveLength(1);
    const input = element('input[type="url"]') as HTMLInputElement;
    expect(input.value).toBe(relay);
    expect(input.readOnly).toBe(method === 'openbitfun_server');
    await click(element('button[aria-label="remoteConnect.copyServerUrl"]'));
    expect(boundary.copyText).toHaveBeenCalledWith(relay);
    boundary.backend = { ...boundary.backend!, account_control_clients: [{ id: 'browser', name: 'Chrome · Windows' }] };
    await tick();
    expect(connections.textContent).toContain('remoteConnect.clientCount:1');
    expect(connections.textContent).not.toContain('Safari · iOS');
    boundary.backend = { ...boundary.backend!, account_control_connected: false, account_control_clients: [] };
    await tick();
    expect(element('[data-openbitfun-part="connections"]').textContent).toContain('remoteConnect.clientCount:0');
    expect(element('[data-openbitfun-part="connections"]').querySelectorAll('li')).toHaveLength(0);
  });

  it('does not invent a total for old clients or mix account clients into another relay tab', async () => {
    boundary.backend = status({ account_control_connected: true, account_control_relay_url: relayA });
    await render('network');
    expect(element('[data-openbitfun-part="connections"]').textContent).toContain('remoteConnect.clientDetailsUnavailable');
    expect(dialog().textContent).not.toContain('remoteConnect.clientCount:');
    await click(element('#remote-connect-network-tab-openbitfun_server'));
    expect(element('[data-openbitfun-part="connections"]').textContent).toContain('remoteConnect.clientCount:0');
    expect(element('[data-openbitfun-part="connections"]').querySelectorAll('li')).toHaveLength(0);
  });

  it('allows connection setup and shows live status without a selected workspace', async () => {
    boundary.hasWorkspace = false;
    await render();
    expect((overviewNetwork() as HTMLButtonElement).disabled).toBe(false);
    expect(overviewNetwork().textContent).toContain('remoteConnect.notConnected');
    const bot = element('[data-openbitfun-part="overviewAction"][data-openbitfun-group="bot"]') as HTMLButtonElement;
    expect(bot.disabled).toBe(false);
    expect(bot.textContent).toContain('remoteConnect.stateConnected');
    await click(bot);
    expect(element('#remote-connect-bot-tabpanel')).toBeDefined();
    await clickText('remoteConnect.backToOverview');
    await generateInvitation();
    expect(boundary.startConnection).toHaveBeenCalledOnce();
    expect(cardStatus()).toBe('remoteConnect.stateWaiting');
    boundary.backend = { ...boundary.backend!, account_control_connected: true, account_control_relay_url: relayA };
    await tick();
    await clickText('remoteConnect.backToOverview');
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
  });

  it.each(['network', 'bot'] as const)('keeps the contextual %s destination open without a selected workspace', async group => {
    boundary.hasWorkspace = false;
    await render(group);
    expect(element(`#remote-connect-${group}-tabpanel`)).toBeDefined();
    expect(document.querySelector('[data-openbitfun-part="overviewAction"]')).toBeNull();
  });

  it('keeps an active invitation when the selected workspace is cleared', async () => {
    await render();
    await generateInvitation();
    boundary.hasWorkspace = false;
    await render();
    expect(cardStatus()).toBe('remoteConnect.stateWaiting');
    expect(boundary.stopConnection).not.toHaveBeenCalled();
  });

  it('keeps QR, overview, close/reopen and sidebar connected, then permits another invitation', async () => {
    await render();
    expect(overviewNetwork().textContent).toContain('remoteConnect.notConnected');
    expect(attachedMobile()).toBeNull();
    expect(attachedBot()).not.toBeNull();
    await generateInvitation();
    expect(cardStatus()).toBe('remoteConnect.stateWaiting');
    boundary.backend = { ...boundary.backend!, account_control_connected: true, account_control_relay_url: relayA };
    await tick();
    expect(cardStatus()).toBe('remoteConnect.stateConnected');
    expect(attachedMobile()).not.toBeNull();
    expect(attachedBot()).not.toBeNull();
    expect(dialog().textContent).toContain('remoteConnect.cancelInvitation');
    expect(dialog().textContent).toContain('remoteConnect.accountConnectedHint');
    expect(dialog().textContent).not.toContain('remoteConnect.disconnect');
    await clickText('remoteConnect.backToOverview');
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
    expect(boundary.stopConnection).toHaveBeenCalledOnce();
    expect(boundary.stopBot).not.toHaveBeenCalled();
    expect(boundary.backend!.account_control_connected).toBe(true);
    await closeDialog();
    expect(attachedMobile()).not.toBeNull();
    await click(element('[data-testid="nav-footer-device-status"]'));
    const devices = element('[data-testid="nav-device-status-connected-devices"]');
    expect(devices.querySelector('[data-openbitfun-device-kind="mobile"] strong')?.textContent).toBe('remoteConnect.mobileBrowserTitle');
    expect(devices.querySelector('[data-openbitfun-device-kind="message-app"] strong')?.textContent).toBe('remoteConnect.weixin');
    expect(document.querySelector('[data-testid="nav-device-connection-service"]')).toBeNull();
    await click(element('[data-testid="nav-footer-device-status"]'));
    await click(element('[data-testid="reopen-remote-connect"]'));
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
    await openNetwork();
    expect(cardStatus()).toBe('remoteConnect.stateConnected');
    expect((element('input[type="url"]') as HTMLInputElement).value).toBe(relayA);
    expect(dialog().textContent).not.toContain('remoteConnect.disconnect');
    await clickText('remoteConnect.showConnectionCode');
    expect(boundary.startConnection).toHaveBeenCalledTimes(2);
    expect(cardStatus()).toBe('remoteConnect.stateConnected');
  });

  it('propagates expiry and reconnect while pairing, on the overview, and after closing the dialog', async () => {
    await render();
    await generateInvitation();
    for (const connected of [true, false, true]) {
      boundary.backend = { ...boundary.backend!, account_control_connected: connected, account_control_relay_url: relayA };
      await tick();
      expect(cardStatus()).toBe(connected ? 'remoteConnect.stateConnected' : 'remoteConnect.stateWaiting');
      expect(Boolean(attachedMobile())).toBe(connected);
      expect(attachedBot()).not.toBeNull();
    }
    await clickText('remoteConnect.backToOverview');
    boundary.backend = { ...boundary.backend!, account_control_connected: false };
    await tick();
    expect(overviewNetwork().textContent).toContain('remoteConnect.notConnected');
    boundary.backend = { ...boundary.backend!, account_control_connected: true };
    await tick();
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
    await closeDialog();
    boundary.backend = { ...boundary.backend!, account_control_connected: false };
    await tick(15_000);
    expect(attachedMobile()).toBeNull();
    expect(attachedBot()).not.toBeNull();
    boundary.backend = { ...boundary.backend!, account_control_connected: true };
    await tick(15_000);
    expect(attachedMobile()).not.toBeNull();
  });

  it('keeps configuration selectable beside an account connection and does not mark another relay path connected', async () => {
    boundary.backend = status({ account_control_connected: true, account_control_relay_url: relayA });
    await render();
    await openNetwork();
    await click(element('#remote-connect-network-tab-lan'));
    await tick(4000);
    expect(element('#remote-connect-network-tab-lan').getAttribute('aria-selected')).toBe('true');
    await clickText('remoteConnect.backToOverview');
    await generateInvitation(relayB);
    expect(boundary.startConnection).toHaveBeenCalledWith('custom_server', relayB, undefined);
    expect(cardStatus()).toBe('remoteConnect.stateWaiting');
    await tick(4000);
    expect(cardStatus()).toBe('remoteConnect.stateWaiting');
    expect(element('#remote-connect-network-tab-custom_server').getAttribute('aria-selected')).toBe('true');
    expect((element('#remote-connect-network-tab-lan') as HTMLButtonElement).disabled).toBe(true);
    expect(attachedMobile()).not.toBeNull();
    await clickText('remoteConnect.cancelAndBack');
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
    expect(boundary.backend!.account_control_relay_url).toBe(relayA);
  });

  it('keeps legacy room disconnect explicit and independent of a coexisting account route and WeChat', async () => {
    boundary.backend = status({ is_connected: true, pairing_state: 'connected', active_method: 'OpenBitFunServer', peer_device_name: 'Phone' });
    delete boundary.backend.account_control_connected;
    delete boundary.backend.account_control_relay_url;
    await render();
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
    expect(attachedMobile()).not.toBeNull();
    await openNetwork();
    await clickText('remoteConnect.backToOverview');
    expect(boundary.stopConnection).not.toHaveBeenCalled();
    boundary.backend = { ...boundary.backend!, account_control_connected: true, account_control_relay_url: relayA };
    await tick();
    await openNetwork();
    await clickText('remoteConnect.disconnect');
    expect(boundary.stopConnection).toHaveBeenCalledOnce();
    expect(boundary.stopBot).not.toHaveBeenCalled();
    expect(boundary.backend!.account_control_connected).toBe(true);
    expect(attachedMobile()).not.toBeNull();
    await clickText('remoteConnect.backToOverview');
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
  });

  it('does not override a selected method when account control appears during the initial status probes', async () => {
    boundary.backend = status({ bot_connected: null });
    await render();
    await openNetwork();
    await click(element('#remote-connect-network-tab-custom_server'));
    await click(element('#remote-connect-network-tab-lan'));
    boundary.backend = { ...boundary.backend!, account_control_connected: true, account_control_relay_url: relayA };
    await tick(4000);
    expect(element('#remote-connect-network-tab-lan').getAttribute('aria-selected')).toBe('true');
    expect(attachedMobile()).not.toBeNull();
    expect(boundary.startConnection).not.toHaveBeenCalled();
  });

  it('preserves a fresh QR across initial probes when no chat app is connected', async () => {
    boundary.backend = status({ bot_connected: null });
    await render();
    await generateInvitation();
    await tick(4000);
    expect(cardStatus()).toBe('remoteConnect.stateWaiting');
    expect(dialog().textContent).toContain(invitation().qr_url);
    expect(dialog().querySelector('.openbitfun-remote-connect__qr-box svg')).not.toBeNull();
    expect(boundary.startConnection).toHaveBeenCalledOnce();
    expect(boundary.stopConnection).not.toHaveBeenCalled();
  });

  it('preserves an explicit method selection while the first dialog read refreshes a cached sidebar snapshot', async () => {
    boundary.backend = status({ bot_connected: null });
    await remoteConnectStatusSource.refresh();
    const pending = deferred<RemoteConnectStatus>();
    boundary.getStatus.mockReturnValueOnce(pending.promise);
    await render();
    await openNetwork();
    await click(element('#remote-connect-network-tab-lan'));
    boundary.backend = { ...boundary.backend!, account_control_connected: true, account_control_relay_url: relayA };
    await act(async () => { pending.resolve(boundary.backend!); });
    expect(element('#remote-connect-network-tab-lan').getAttribute('aria-selected')).toBe('true');
    expect(attachedMobile()).not.toBeNull();
  });

  it('publishes a slow read to both surfaces and shows unavailable instead of waiting when a later read fails', async () => {
    const pending = deferred<RemoteConnectStatus>();
    boundary.getStatus.mockReturnValueOnce(pending.promise);
    await render();
    expect(overviewNetwork().textContent).toContain('remoteConnect.statusChecking');
    await tick(4000);
    expect(boundary.getStatus).toHaveBeenCalledOnce();
    boundary.backend = status({ account_control_connected: true, account_control_relay_url: relayA });
    await act(async () => { pending.resolve(boundary.backend!); });
    expect(overviewNetwork().textContent).toContain('remoteConnect.stateConnected');
    expect(attachedMobile()).not.toBeNull();
    boundary.getStatus.mockRejectedValueOnce(new Error('transport temporarily unavailable'));
    await tick();
    expect(overviewNetwork().textContent).toContain('remoteConnect.statusUnavailable');
    expect(overviewNetwork().textContent).not.toContain('remoteConnect.notConnected');
    await openNetwork();
    expect(cardStatus()).toBe('remoteConnect.statusUnavailable');
    await tick();
    expect(cardStatus()).toBe('remoteConnect.stateConnected');
    expect(attachedMobile()).not.toBeNull();
  });

  it('cleans up an invitation created after closing or unmounting without publishing it back into the UI', async () => {
    for (const unmount of [false, true]) {
      const pending = deferred<ConnectionResult>();
      boundary.startConnection.mockReturnValueOnce(pending.promise);
      await render();
      await generateInvitation();
      if (unmount) {
        await act(async () => { root.unmount(); });
        mounted = false;
      } else await closeDialog();
      const stopCount = boundary.stopConnection.mock.calls.length;
      await act(async () => {
        boundary.backend = status({ active_method: `CustomServer { url: "${relayA}" }`, pairing_state: 'waiting_for_scan' });
        pending.resolve(invitation());
      });
      expect(boundary.stopConnection).toHaveBeenCalledTimes(stopCount + 1);
      expect(boundary.backend!.pairing_state).toBe('idle');
      expect(document.querySelector('[data-openbitfun-part="pairingCard"]')).toBeNull();
      expect(attachedMobile()).toBeNull();
      if (!unmount) await click(element('[data-testid="reopen-remote-connect"]'));
    }
  });

  it('keeps a bot read failure explicit without opening another login, and exposes retry in the sidebar', async () => {
    await render();
    await click(element('[data-openbitfun-part="overviewAction"][data-openbitfun-group="bot"]'));
    expect(dialog().textContent).toContain('remoteConnect.disconnect');
    boundary.getStatus.mockRejectedValue(new Error('status unavailable'));
    await tick();
    expect(cardStatus()).toBe('remoteConnect.statusUnavailable');
    expect(dialog().textContent).not.toContain('remoteConnect.botWeixinQrButton');
    expect(dialog().textContent).not.toContain('remoteConnect.getPairingCode');
    expect(dialog().querySelector('input')).toBeNull();
    await clickText('remoteConnect.backToOverview');
    expect(element('[data-openbitfun-part="overviewAction"][data-openbitfun-group="bot"]').textContent).toContain('remoteConnect.statusUnavailable');
    await closeDialog();
    await click(element('[data-testid="nav-footer-device-status"]'));
    const notice = element('.openbitfun-device-overview__notice');
    expect(notice.textContent).toBe('deviceOverview.statusUnavailable');
    boundary.getStatus.mockImplementation(async () => ({ ...boundary.backend! }));
    await click(notice);
    expect(document.querySelector('.openbitfun-device-overview__notice')).toBeNull();
    expect(attachedBot()).not.toBeNull();
    expect(boundary.stopBot).not.toHaveBeenCalled();
    expect(boundary.startConnection).not.toHaveBeenCalled();
  });

  it('preserves an invitation during a failed read and restores its confirmed account state afterward', async () => {
    await render();
    await generateInvitation();
    boundary.backend = { ...boundary.backend!, account_control_connected: true, account_control_relay_url: relayA };
    await tick();
    expect(cardStatus()).toBe('remoteConnect.stateConnected');
    boundary.getStatus.mockRejectedValueOnce(new Error('status unavailable'));
    await tick();
    expect(cardStatus()).toBe('remoteConnect.statusUnavailable');
    expect(dialog().textContent).toContain(invitation().qr_url);
    expect(dialog().textContent).toContain('remoteConnect.cancelInvitation');
    await tick();
    expect(cardStatus()).toBe('remoteConnect.stateConnected');
    expect(boundary.stopConnection).not.toHaveBeenCalled();
  });

  it('ignores connected replies begun before and during the actual Disconnect handler', async () => {
    const connected = status({ is_connected: true, pairing_state: 'connected', active_method: 'OpenBitFunServer' });
    boundary.backend = connected;
    await render();
    await openNetwork();
    const before = deferred<RemoteConnectStatus>();
    const during = deferred<RemoteConnectStatus>();
    boundary.getStatus.mockReturnValueOnce(before.promise).mockReturnValueOnce(during.promise);
    await tick();
    const stop = deferred<void>();
    boundary.stopConnection.mockImplementationOnce(async () => {
      await stop.promise;
      boundary.backend = status();
    });
    await clickText('remoteConnect.disconnect');
    await tick();
    await act(async () => { stop.resolve(); });
    expect(attachedMobile()).toBeNull();
    expect(attachedBot()).not.toBeNull();
    expect(dialog().textContent).toContain('remoteConnect.showConnectionCode');
    await act(async () => {
      during.resolve(connected);
      before.resolve(connected);
    });
    expect(attachedMobile()).toBeNull();
    expect(dialog().textContent).not.toContain('remoteConnect.disconnect');
    await clickText('remoteConnect.backToOverview');
    expect(overviewNetwork().textContent).toContain('remoteConnect.notConnected');
    expect(boundary.stopConnection).toHaveBeenCalledOnce();
  });
});
