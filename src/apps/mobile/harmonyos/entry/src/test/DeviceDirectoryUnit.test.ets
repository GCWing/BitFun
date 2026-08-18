import { describe, it, expect } from '@ohos/hypium';
import { AssistantEntry, RecentWorkspaceEntry, RemoteSession } from '../main/ets/model/RemoteModels';
import { AccountDeviceDirectorySource } from '../main/ets/services/AccountDeviceDirectory';
import {
  mapInConcurrentBatches,
  mergeUniqueSessions
} from '../main/ets/services/ConcurrentWorkspaceListing';
import { DeviceDirectoryCoordinator } from '../main/ets/services/DeviceDirectoryCoordinator';
import { DeviceDirectoryViewModel } from '../main/ets/pages/viewmodel/DeviceDirectoryViewModel';
import { DeviceDirectoryState } from '../main/ets/pages/state/DeviceDirectoryState';
import { remoteSessionIdentity, stampRemoteSessionsDevice } from '../main/ets/services/RemoteSessionIdentity';
import { RemoteUiState } from '../main/ets/services/RemoteUiState';
import { FakeRemoteSessionListStore, remoteSession } from './LocalTestFixtures';

class FakeAccountDeviceDirectory implements AccountDeviceDirectorySource {
  listedDeviceIds: string[] = [];
  listedWorkspacePaths: string[] = [];
  workspacesByDevice: Map<string, RecentWorkspaceEntry[]> = new Map<string, RecentWorkspaceEntry[]>();
  assistantsByDevice: Map<string, AssistantEntry[]> = new Map<string, AssistantEntry[]>();
  sessionsByDevicePath: Map<string, RemoteSession[]> = new Map<string, RemoteSession[]>();
  failingDevices: string[] = [];
  failingPaths: string[] = [];

  async listRecentWorkspaces(deviceId: string): Promise<RecentWorkspaceEntry[]> {
    this.listedDeviceIds.push(deviceId);
    if (this.failingDevices.includes(deviceId)) {
      throw new Error(`device unavailable: ${deviceId}`);
    }
    return this.workspacesByDevice.get(deviceId) || [];
  }

  async listAssistants(deviceId: string): Promise<AssistantEntry[]> {
    return this.assistantsByDevice.get(deviceId) || [];
  }

  async listSessions(deviceId: string, workspacePath: string): Promise<RemoteSession[]> {
    this.listedWorkspacePaths.push(`${deviceId}:${workspacePath}`);
    if (this.failingPaths.includes(`${deviceId}:${workspacePath}`)) {
      throw new Error(`workspace unavailable: ${workspacePath}`);
    }
    return this.sessionsByDevicePath.get(`${deviceId}:${workspacePath}`) || [];
  }
}

class GatedAccountDeviceDirectory extends FakeAccountDeviceDirectory {
  private releaseWorkspaceHold: () => void = () => {};
  private releaseSessionHold: () => void = () => {};
  private notifyWorkspacesListed: () => void = () => {};
  private readonly workspaceHold: Promise<void>;
  private readonly sessionHold: Promise<void>;
  readonly workspacesListed: Promise<void>;

  constructor() {
    super();
    this.workspaceHold = new Promise<void>((resolve: () => void) => {
      this.releaseWorkspaceHold = resolve;
    });
    this.sessionHold = new Promise<void>((resolve: () => void) => {
      this.releaseSessionHold = resolve;
    });
    this.workspacesListed = new Promise<void>((resolve: () => void) => {
      this.notifyWorkspacesListed = resolve;
    });
  }

  releaseWorkspaces(): void {
    this.releaseWorkspaceHold();
  }

  releaseSessions(): void {
    this.releaseSessionHold();
  }

  async listRecentWorkspaces(deviceId: string): Promise<RecentWorkspaceEntry[]> {
    await this.workspaceHold;
    return super.listRecentWorkspaces(deviceId);
  }

  async listSessions(deviceId: string, workspacePath: string): Promise<RemoteSession[]> {
    this.notifyWorkspacesListed();
    await this.sessionHold;
    return super.listSessions(deviceId, workspacePath);
  }
}

function workspace(path: string, name: string): RecentWorkspaceEntry {
  return { path, name, lastOpened: '', workspaceKind: 'normal' };
}

function sessionOn(deviceId: string, id: string, title: string): RemoteSession {
  const item = remoteSession(id, title);
  item.deviceId = deviceId;
  return item;
}

export default function deviceDirectoryUnitTest() {
  describe('ConcurrentWorkspaceListing', () => {
    it('keeps merge order independent of batch width', 0, async () => {
      const paths: string[] = [];
      for (let index = 0; index < 9; index++) {
        paths.push(`/workspace-${index}`);
      }
      const wide = await mapInConcurrentBatches(paths, async (path: string): Promise<RemoteSession[]> => {
        return [remoteSession(`session-${path}`, path)];
      }, 4);
      const narrow = await mapInConcurrentBatches(paths, async (path: string): Promise<RemoteSession[]> => {
        return [remoteSession(`session-${path}`, path)];
      }, 2);
      const wideMerged = mergeUniqueSessions(wide);
      const narrowMerged = mergeUniqueSessions(narrow);
      expect(wideMerged.length).assertEqual(9);
      expect(narrowMerged.length).assertEqual(9);
      expect(wideMerged[0].id).assertEqual(narrowMerged[0].id);
      expect(wideMerged[4].id).assertEqual(narrowMerged[4].id);
      expect(wideMerged[8].id).assertEqual(narrowMerged[8].id);
    });

    it('keeps same-id sessions from two devices', 0, () => {
      const merged = mergeUniqueSessions([
        [sessionOn('desktop-a', 'shared', 'A')],
        [sessionOn('desktop-b', 'shared', 'B')]
      ]);
      expect(merged.length).assertEqual(2);
      expect(remoteSessionIdentity(merged[0])).assertEqual('desktop-a\u0001shared');
      expect(remoteSessionIdentity(merged[1])).assertEqual('desktop-b\u0001shared');
    });
  });

  describe('DeviceDirectoryCoordinator', () => {
    it('returns the other devices when one device cannot be listed', 0, async () => {
      const source = new FakeAccountDeviceDirectory();
      source.workspacesByDevice.set('desk-a', [workspace('/a', 'A')]);
      source.workspacesByDevice.set('desk-b', [workspace('/b', 'B')]);
      source.workspacesByDevice.set('desk-c', [workspace('/c', 'C')]);
      source.sessionsByDevicePath.set('desk-a:/a', [sessionOn('desk-a', 'a-1', 'A1')]);
      source.sessionsByDevicePath.set('desk-c:/c', [sessionOn('desk-c', 'c-1', 'C1')]);
      source.failingDevices = ['desk-b'];
      const results: RemoteSession[][] = [];
      const failures: string[] = [];
      const ids = ['desk-a', 'desk-b', 'desk-c'];
      for (let index = 0; index < ids.length; index++) {
        const deviceId = ids[index];
        try {
          const catalog = await new DeviceDirectoryCoordinator(source).loadDeviceCatalog(deviceId);
          results.push(catalog.sessions);
        } catch (_err) {
          failures.push(deviceId);
        }
      }
      expect(failures.length).assertEqual(1);
      expect(failures[0]).assertEqual('desk-b');
      expect(results.length).assertEqual(2);
      expect(results[0][0].id).assertEqual('a-1');
      expect(results[1][0].id).assertEqual('c-1');
    });

    it('keeps other workspaces when one workspace cannot be listed', 0, async () => {
      const source = new FakeAccountDeviceDirectory();
      source.workspacesByDevice.set('desk-a', [
        workspace('/one', 'One'),
        workspace('/two', 'Two'),
        workspace('/three', 'Three')
      ]);
      source.sessionsByDevicePath.set('desk-a:/one', [sessionOn('desk-a', 'one-session', 'One')]);
      source.sessionsByDevicePath.set('desk-a:/three', [sessionOn('desk-a', 'three-session', 'Three')]);
      source.failingPaths = ['desk-a:/two'];
      const catalog = await new DeviceDirectoryCoordinator(source).loadDeviceCatalog('desk-a');
      expect(catalog.sessions.length).assertEqual(2);
      expect(catalog.sessions[0].id).assertEqual('one-session');
      expect(catalog.sessions[1].id).assertEqual('three-session');
    });
  });

  describe('DeviceDirectoryViewModel', () => {
    it('does not issue RPC for an offline device', 0, async () => {
      let credentialCalls = 0;
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => {
          credentialCalls += 1;
          throw new Error('offline devices must not build a directory');
        },
        activeDeviceId: () => 'desk-active',
        activeDeviceConnected: () => false
      });
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-offline', deviceName: 'Offline', online: false }
      ]);
      await viewModel.toggleDevice('desk-offline');
      expect(credentialCalls).assertEqual(0);
      expect(state.find('desk-offline')?.expanded).assertTrue();
      expect(state.find('desk-offline')?.status).assertEqual('idle');
    });

    it('does not expand an offline device even when it is the control target', 0, () => {
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-offline',
        activeDeviceConnected: () => false
      });
      viewModel.syncDevices([
        { deviceId: 'desk-offline', deviceName: 'Studio', online: false }
      ]);
      expect(state.find('desk-offline')?.deviceName).assertEqual('Studio');
      expect(state.find('desk-offline')?.expanded).assertFalse();
    });

    it('collapses a device when it goes offline and keeps the last known name', 0, () => {
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-studio',
        activeDeviceConnected: () => false
      });
      viewModel.syncDevices([
        { deviceId: 'desk-studio', deviceName: 'Studio', online: true }
      ]);
      expect(state.find('desk-studio')?.expanded).assertTrue();
      viewModel.syncDevices([
        { deviceId: 'desk-studio', deviceName: '', online: false }
      ]);
      expect(state.find('desk-studio')?.deviceName).assertEqual('Studio');
      expect(state.find('desk-studio')?.expanded).assertFalse();
    });

    it('keeps the live conversation device online when the relay says otherwise', 0, () => {
      const state = new DeviceDirectoryState();
      let connected = true;
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-live',
        activeDeviceConnected: () => connected
      });
      // The relay's registry can lag or lose an entry while the desktop answers
      // every RPC we send it. First-hand evidence outranks that projection.
      viewModel.syncDevices([
        { deviceId: 'desk-live', deviceName: 'Studio', online: false },
        { deviceId: 'desk-other', deviceName: 'Other', online: false }
      ]);
      expect(state.find('desk-live')?.online).assertTrue();
      expect(state.find('desk-live')?.expanded).assertTrue();
      // The override must not outlive the link it is based on, or a device that
      // really did go away keeps a lit row forever.
      expect(state.find('desk-other')?.online).assertFalse();
      connected = false;
      viewModel.syncDevices([
        { deviceId: 'desk-live', deviceName: 'Studio', online: false }
      ]);
      expect(state.find('desk-live')?.online).assertFalse();
      expect(state.find('desk-live')?.expanded).assertFalse();
    });

    it('keeps a collapsed active device collapsed across syncDevices', 0, async () => {
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-active',
        activeDeviceConnected: () => false
      });
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-other', deviceName: 'Other', online: true }
      ]);
      expect(state.find('desk-active')?.expanded).assertTrue();
      await viewModel.toggleDevice('desk-active');
      expect(state.find('desk-active')?.expanded).assertFalse();
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-other', deviceName: 'Other', online: true }
      ]);
      expect(state.find('desk-active')?.expanded).assertFalse();
      viewModel.refreshActive();
      expect(state.find('desk-active')?.expanded).assertFalse();
    });

    it('toggles expand on the same directory entry instance', 0, async () => {
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-active',
        activeDeviceConnected: () => false
      });
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-other', deviceName: 'Other', online: false }
      ]);
      const before = state.find('desk-other');
      await viewModel.toggleDevice('desk-other');
      expect(state.find('desk-other') === before).assertTrue();
      expect(state.find('desk-other')?.expanded).assertTrue();
      await viewModel.toggleDevice('desk-other');
      expect(state.find('desk-other') === before).assertTrue();
      expect(state.find('desk-other')?.expanded).assertFalse();
    });

    it('marks an online device loading before its catalog returns', 0, async () => {
      const source = new GatedAccountDeviceDirectory();
      source.workspacesByDevice.set('desk-other', [workspace('/other', 'Other')]);
      source.sessionsByDevicePath.set('desk-other:/other', [sessionOn('desk-other', 's1', 'S1')]);
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-active',
        activeDeviceConnected: () => false
      }, undefined, source);
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-other', deviceName: 'Other', online: true }
      ]);
      const pending = viewModel.toggleDevice('desk-other');
      expect(state.find('desk-other')?.expanded).assertTrue();
      expect(state.find('desk-other')?.status).assertEqual('loading');
      expect(state.find('desk-other')?.workspaces.length).assertEqual(0);
      source.releaseWorkspaces();
      source.releaseSessions();
      await pending;
      expect(state.find('desk-other')?.status).assertEqual('ready');
      expect(state.find('desk-other')?.workspaces.length).assertEqual(1);
      expect(state.find('desk-other')?.sessions.length).assertEqual(1);
    });

    it('shows workspaces while session listing is still in flight', 0, async () => {
      const source = new GatedAccountDeviceDirectory();
      source.workspacesByDevice.set('desk-other', [workspace('/other', 'Other')]);
      source.sessionsByDevicePath.set('desk-other:/other', [sessionOn('desk-other', 's1', 'S1')]);
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-active',
        activeDeviceConnected: () => false
      }, undefined, source);
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-other', deviceName: 'Other', online: true }
      ]);
      const pending = viewModel.toggleDevice('desk-other');
      source.releaseWorkspaces();
      await source.workspacesListed;
      expect(state.find('desk-other')?.status).assertEqual('loading');
      expect(state.find('desk-other')?.workspaces.length).assertEqual(1);
      expect(state.find('desk-other')?.sessions.length).assertEqual(0);
      source.releaseSessions();
      await pending;
      expect(state.find('desk-other')?.status).assertEqual('ready');
      expect(state.find('desk-other')?.sessions[0].id).assertEqual('s1');
    });

    it('does not move the cold-start last-device pointer when persisting an observed catalog', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      await store.saveList('desk-active', [sessionOn('desk-active', 'live', 'Live')], false);
      const source = new FakeAccountDeviceDirectory();
      source.workspacesByDevice.set('desk-other', [workspace('/other', 'Other')]);
      source.sessionsByDevicePath.set('desk-other:/other', [sessionOn('desk-other', 's1', 'S1')]);
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-active',
        activeDeviceConnected: () => false
      }, store, source);
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-other', deviceName: 'Other', online: true }
      ]);
      await viewModel.toggleDevice('desk-other');
      expect(state.find('desk-other')?.status).assertEqual('ready');
      expect(store.lists.get('desk-other')?.sessions.length).assertEqual(1);
      expect(store.lastDeviceKey).assertEqual('desk-active');
    });

    it('treats a hydrated catalog as cached and still fetches on expand', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      await store.saveWorkspaces('desk-other', [workspace('/cached', 'Cached')]);
      await store.saveObservedList('desk-other', [sessionOn('desk-other', 'cached', 'Cached')], false);
      const source = new FakeAccountDeviceDirectory();
      source.workspacesByDevice.set('desk-other', [workspace('/live', 'Live')]);
      source.sessionsByDevicePath.set('desk-other:/live', [sessionOn('desk-other', 'live', 'Live')]);
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-active',
        activeDeviceConnected: () => false
      }, store, source);
      viewModel.syncDevices([
        { deviceId: 'desk-active', deviceName: 'Active', online: true },
        { deviceId: 'desk-other', deviceName: 'Other', online: true }
      ]);
      await viewModel.hydrateKnownDevices();
      expect(state.find('desk-other')?.status).assertEqual('cached');
      expect(state.find('desk-other')?.workspaces[0].path).assertEqual('/cached');
      expect(source.listedDeviceIds.length).assertEqual(0);
      await viewModel.toggleDevice('desk-other');
      expect(source.listedDeviceIds.length).assertEqual(1);
      expect(source.listedDeviceIds[0]).assertEqual('desk-other');
      expect(state.find('desk-other')?.status).assertEqual('ready');
      expect(state.find('desk-other')?.workspaces[0].path).assertEqual('/live');
      expect(state.find('desk-other')?.sessions[0].id).assertEqual('live');
    });

    it('keeps a captured live catalog after the device is no longer the control target', 0, async () => {
      let activeId = 'desk-a';
      const source = new FakeAccountDeviceDirectory();
      source.workspacesByDevice.set('desk-a', [workspace('/gone', 'Gone')]);
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => activeId,
        activeDeviceConnected: () => false
      }, undefined, source);
      viewModel.syncDevices([
        { deviceId: 'desk-a', deviceName: 'Alpha', online: true },
        { deviceId: 'desk-b', deviceName: 'Beta', online: true }
      ]);
      expect(state.find('desk-a')?.status).assertEqual('idle');
      viewModel.captureLive(
        'desk-a',
        [workspace('/a', 'A')],
        [sessionOn('desk-a', 'a-1', 'A1')]
      );
      activeId = 'desk-b';
      viewModel.refreshActive();
      expect(state.find('desk-a')?.status).assertEqual('ready');
      expect(state.find('desk-a')?.sessions[0].id).assertEqual('a-1');
      expect(state.find('desk-a')?.workspaces[0].path).assertEqual('/a');
      await viewModel.toggleDevice('desk-a');
      await viewModel.toggleDevice('desk-a');
      expect(source.listedDeviceIds.length).assertEqual(0);
      expect(state.find('desk-a')?.sessions[0].id).assertEqual('a-1');
    });

    it('does not overwrite a captured catalog with an empty teardown snapshot', 0, () => {
      const state = new DeviceDirectoryState();
      const viewModel = new DeviceDirectoryViewModel(state, {
        credentials: () => undefined,
        activeDeviceId: () => 'desk-a',
        activeDeviceConnected: () => false
      });
      viewModel.syncDevices([
        { deviceId: 'desk-a', deviceName: 'Alpha', online: true }
      ]);
      viewModel.captureLive('desk-a', [workspace('/a', 'A')], [sessionOn('desk-a', 'a-1', 'A1')]);
      viewModel.captureLive('desk-a', [], []);
      expect(state.find('desk-a')?.sessions.length).assertEqual(1);
      expect(state.find('desk-a')?.workspaces.length).assertEqual(1);
    });
  });

  describe('RemoteSessionIdentity', () => {
    it('keeps same-id sessions from two devices when merging UI lists', 0, () => {
      const merged = RemoteUiState.mergeSessions(
        [sessionOn('desk-a', 'shared', 'A')],
        [sessionOn('desk-b', 'shared', 'B')]
      );
      expect(merged.length).assertEqual(2);
      expect(merged[0].deviceId || '').assertEqual('desk-a');
      expect(merged[1].deviceId || '').assertEqual('desk-b');
    });

    it('collapses a stamped current session with an unstamped extra of the same id', 0, () => {
      const current = [sessionOn('desk-a', 'shared', 'Live')];
      const extras = [remoteSession('shared', 'Discovered')];
      expect(RemoteUiState.mergeSessions(current, extras).length).assertEqual(2);
      const merged = RemoteUiState.mergeSessions(current, stampRemoteSessionsDevice(extras, 'desk-a'));
      expect(merged.length).assertEqual(1);
      expect(merged[0].deviceId || '').assertEqual('desk-a');
    });
  });
}
