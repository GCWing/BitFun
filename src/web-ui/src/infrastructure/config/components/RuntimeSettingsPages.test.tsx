// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDesktopControlSettingsPage } from './RuntimeSettingsPages';

const mocks = vi.hoisted(() => {
  Object.defineProperty(window, '__TAURI__', { configurable: true, value: {} });
  return {
    invoke: vi.fn(), setConfig: vi.fn(), setEnabled: vi.fn(),
    t: (key: string) => key,
  };
});
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: mocks.t }) }));
vi.mock('@/infrastructure/i18n', () => ({ i18nService: { formatNumber: String } }));
vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({ api: { invoke: mocks.invoke } }));
vi.mock('@/infrastructure/api/service-api/SystemAPI', () => ({ systemAPI: { getSystemInfo: async () => ({ platform: 'macos' }) } }));
vi.mock('../services/ConfigManager', () => ({ configManager: {
  getConfig: async () => false, setConfig: mocks.setConfig,
} }));
vi.mock('../hooks/useComputerUseEnabled', () => ({ useComputerUseEnabled: () => ({ computerUseEnabled: false, setComputerUseEnabled: mocks.setEnabled }) }));
vi.mock('../services/AIExperienceConfigService', () => ({ aiExperienceConfigService: {} }));
vi.mock('../services/AgentCompanionPetService', () => ({ DEFAULT_AGENT_COMPANION_PET: 'default' }));
vi.mock('../services/PermissionConfigService', () => ({ DEFAULT_TOOL_PERMISSION_CONFIG: {}, permissionConfigService: {} }));
vi.mock('@/infrastructure/peer-device/peerDeviceContextState', () => ({ usePeerDeviceModeOptional: () => null }));
vi.mock('@/infrastructure/confirm-dialog', () => ({ confirmDanger: vi.fn() }));
vi.mock('@/shared/notification-system', () => ({ useNotification: () => ({}), notificationService: { dismiss: vi.fn(), success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() } }));
vi.mock('./GlobalPermissionRulesDialog', () => ({ GlobalPermissionRulesDialog: () => null }));
vi.mock('./SessionTitleConfig', () => ({ default: () => null }));
vi.mock('./ReviewCapacitySection', () => ({ default: () => null }));
vi.mock('./ToolJsonRepairSection', () => ({ default: () => null }));
vi.mock('@openbitfun/ui', async (original) => ({
  ...await original<typeof import('@openbitfun/ui')>(),
  Select: ({ value, options, disabled }: { value: string; options: Array<{ value: string; label: string }>; disabled: boolean }) => (
    <select value={value} disabled={disabled} onChange={() => {}}>{options.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}</select>
  ),
}));

const readyStatus = { cdpAvailable: false, defaultCdpSupported: true, defaultCdpEnabled: true, browserReady: true, browserKind: 'Chrome', browserVersion: '146', pageCount: 3 };
let status = { ...readyStatus };
let container: HTMLDivElement;
let root: Root;
const button = (label: string) => Array.from(container.querySelectorAll('button')).find(el => el.textContent === label || el.getAttribute('aria-label') === label)!;
const render = async () => { await act(async () => root.render(<BrowserDesktopControlSettingsPage />)); };

beforeEach(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  status = { ...readyStatus };
  mocks.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'computer_use_get_status') return { computerUseEnabled: false, accessibilityGranted: true, screenCaptureGranted: false, platformNote: null };
    if (command === 'browser_control_get_status') return { ...status };
    if (command === 'browser_control_list_browsers') return { options: [{ value: 'default', label: 'Default browser', installed: true }] };
    return { success: true, status: 'connected', browserKind: 'Chrome' };
  });
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});
afterEach(async () => { await act(async () => root.unmount()); container.remove(); });

describe('Browser and desktop control settings', () => {
  it('localizes the browser choice and puts the single connect action next to connection status', async () => {
    await render();
    expect(container.querySelector('option')?.textContent).toBe('browserControl.defaultBrowser');
    const connect = button('browserControl.connect');
    expect(connect.closest('[data-openbitfun-part="row"]')?.textContent).toContain('browserControl.status');
    expect(container.textContent).toContain('browserControl.readyNotConnected');
    expect(Array.from(container.querySelectorAll('button')).filter(el => el.textContent === 'browserControl.connect')).toHaveLength(1);
    await act(async () => connect.click());
    expect(mocks.invoke).toHaveBeenCalledWith('browser_control_enable_default_cdp', expect.anything());
  });

  it('uses the existing launch path when the browser cannot reuse its current profile', async () => {
    status.defaultCdpSupported = false;
    await render();
    await act(async () => button('browserControl.connect').click());
    expect(mocks.invoke).toHaveBeenCalledWith('browser_control_launch', { request: { port: 9222 } });
    expect(mocks.invoke).not.toHaveBeenCalledWith('browser_control_enable_default_cdp', expect.anything());
  });

  it('keeps the selected browser visible while connected and disconnects through the existing command', async () => {
    status.cdpAvailable = true;
    await render();
    expect(container.querySelector('select')?.disabled).toBe(true);
    expect(container.textContent).toContain('browserControl.connected');
    expect(container.textContent).toContain('Chrome · 3');
    await act(async () => button('browserControl.disconnect').click());
    expect(mocks.invoke).toHaveBeenCalledWith('browser_control_disconnect', expect.anything());
  });

  it('does not show stale connected status as current after a refresh fails', async () => {
    status.cdpAvailable = true;
    await render();
    mocks.invoke.mockRejectedValue(new Error('Host is offline'));
    await act(async () => button('browserControl.refreshStatus').click());
    const connectionRow = button('browserControl.disconnect').closest('[data-openbitfun-part="row"]');
    expect(connectionRow?.textContent).toContain('browserControl.statusUnavailable');
    expect(connectionRow?.textContent).not.toContain('browserControl.connected');
    expect(button('browserControl.disconnect').disabled).toBe(true);
    expect(button('browserControl.refreshStatus').disabled).toBe(false);
  });

  it('retains an explicit unsupported state when a CLI peer rejects control commands', async () => {
    mocks.invoke.mockRejectedValue(new Error('Command is not supported on CLI peer host'));
    await render();
    expect(container.textContent).toContain('computerUse.peerUnsupported');
    expect(container.textContent).toContain('browserControl.peerUnsupported');
    expect(container.querySelector('select')).toBeNull();
  });
});
