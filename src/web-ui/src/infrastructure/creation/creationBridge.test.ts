// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { JSDOM } from 'jsdom';
import { attachCreationRuntime, CREATION_REQUEST_EVENT, executeCreationRequest, listenForCreationRequests, recordCreationActivationError } from './creationBridge';
import { createCreationUiApi } from '@/app/creation/creationUiApi';
import { setPeerDeviceModeActiveFlag } from '../peer-device/peerModeFlag';
import { api } from '../api/service-api/ApiClient';

vi.mock('../api/service-api/ApiClient', () => ({ api: { listen: vi.fn(() => vi.fn()), invoke: vi.fn(async () => {}) } }));
vi.mock('@/app/stores/sceneStore', () => ({ useSceneStore: { getState: () => ({ activeTabId: 'session' }), subscribe: () => vi.fn() } }));
beforeEach(() => vi.stubGlobal('localStorage', new JSDOM('', { url: 'https://creation.test' }).window.localStorage));
afterEach(() => { setPeerDeviceModeActiveFlag(false); document.body.innerHTML = ''; vi.unstubAllGlobals(); vi.clearAllMocks(); });

it('delivers a correlated Agent request to a registered command and updates the mounted UI through state events', async () => {
  document.body.innerHTML = '<div data-openbitfun-creation-slot="sidebar-footer"></div>';
  const creation = createCreationUiApi(new AbortController().signal);
  const root = creation.api.mount('sidebar-footer');
  creation.api.events.on('state.changed', () => { root.textContent = String(creation.api.state.get('counter.value')); });
  creation.api.commands.register({ id: 'counter.set', description: 'Set counter', parameters: { value: { type: 'number', required: true } } }, args => creation.api.state.set('counter.value', args.value));
  const detach = attachCreationRuntime(creation);
  const unlisten = listenForCreationRequests();
  const snapshot = await executeCreationRequest({ requestId: 'inspect-1', action: 'inspect' }) as ReturnType<typeof creation.inspect>;
  expect(snapshot.slots[0]).toEqual({ id: 'sidebar-footer', present: true, mounts: 1 });
  expect(snapshot.commands[0].id).toBe('counter.set');
  const [event, handler] = vi.mocked(api.listen).mock.calls[0];
  expect(event).toBe(CREATION_REQUEST_EVENT);
  handler({ requestId: 'invoke-1', action: 'invoke', commandId: 'counter.set', arguments: { value: 12 } });
  await vi.waitFor(() => expect(api.invoke).toHaveBeenCalledWith('report_openbitfun_control_result', { request: { requestId: 'invoke-1', success: true, result: 12 } }));
  expect(root.textContent).toBe('12');
  setPeerDeviceModeActiveFlag(true);
  await expect(executeCreationRequest({ requestId: 'peer', action: 'inspect' })).rejects.toThrow('local Desktop');
  setPeerDeviceModeActiveFlag(false);
  detach(); creation.dispose(); unlisten();
  await expect(executeCreationRequest({ requestId: 'old', action: 'invoke', commandId: 'counter.set' })).rejects.toThrow('not active');
  recordCreationActivationError(new Error('Unknown customization slot after upgrade'));
  await expect(executeCreationRequest({ requestId: 'failed', action: 'inspect' })).rejects.toThrow('Unknown customization slot after upgrade');
  recordCreationActivationError(null);
  expect(root.isConnected).toBe(false);
});
