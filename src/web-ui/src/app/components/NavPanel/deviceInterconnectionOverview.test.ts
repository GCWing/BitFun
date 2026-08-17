import { describe, expect, it } from 'vitest';
import type { RemoteConnectStatus } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import {
  classifyAccountRelayUrl,
  connectionServiceFromRelayUrl,
  projectDeviceInterconnectionOverview,
  type DeviceInterconnectionOverviewInput,
} from './deviceInterconnectionOverview';

const disconnectedStatus: RemoteConnectStatus = {
  is_connected: false,
  pairing_state: 'idle',
  active_method: null,
  peer_device_name: null,
  peer_user_id: null,
  bot_connected: null,
  bot_verbose_mode: false,
};

const baseInput = (
  overrides: Partial<DeviceInterconnectionOverviewInput> = {},
): DeviceInterconnectionOverviewInput => ({
  localDeviceName: 'This Windows',
  peer: null,
  remoteStatus: disconnectedStatus,
  remoteStatusState: 'ready',
  dispatchJobs: [],
  accountService: null,
  ...overrides,
});

describe('projectDeviceInterconnectionOverview', () => {
  it('presents local work as a complete state without a connection-service row', () => {
    const overview = projectDeviceInterconnectionOverview(baseInput());

    expect(overview.mode).toBe('local');
    expect(overview.devices).toEqual([
      expect.objectContaining({
        name: 'This Windows',
        activities: ['current-use'],
      }),
    ]);
    expect(overview.connectionService).toBeNull();
    expect(overview.topologyUnavailable).toBe(false);
  });

  it('does not turn account service availability into a connected-device state', () => {
    const overview = projectDeviceInterconnectionOverview(baseInput({
      accountService: connectionServiceFromRelayUrl(
        'https://remote.openbitfun.com/relay',
      ),
    }));

    expect(overview.mode).toBe('local');
    expect(overview.connectedDevices).toHaveLength(0);
    expect(overview.connectionService).toBeNull();
  });

  it('shows a paired phone once as a controller and identifies the active service', () => {
    const overview = projectDeviceInterconnectionOverview(baseInput({
      remoteStatus: {
        ...disconnectedStatus,
        is_connected: true,
        pairing_state: 'connected',
        active_method: 'BitfunServer',
        peer_device_name: 'My iPhone',
        peer_user_id: 'mobile-user',
      },
    }));

    expect(overview.mode).toBe('connected');
    expect(overview.devices).toHaveLength(2);
    expect(overview.connectedDevices).toEqual([
      expect.objectContaining({
        name: 'My iPhone',
        activities: ['controlling'],
      }),
    ]);
    expect(overview.connectionService?.kind).toBe('official');
  });

  it('makes the peer the current device and the local desktop its controller', () => {
    const overview = projectDeviceInterconnectionOverview(baseInput({
      peer: { deviceId: 'linux-1', deviceName: 'Linux Workstation' },
      accountService: connectionServiceFromRelayUrl('https://relay.example.com'),
    }));

    expect(overview.primaryDevice).toEqual(expect.objectContaining({
      id: 'device:linux-1',
      name: 'Linux Workstation',
      activities: ['current-use'],
    }));
    expect(overview.connectedDevices).toEqual([
      expect.objectContaining({
        id: 'device:local',
        name: 'This Windows',
        activities: ['controlling'],
      }),
    ]);
    expect(overview.connectionService?.kind).toBe('self-hosted');
  });

  it('shows same-account distributed hosts but excludes ordinary SSH targets', () => {
    const overview = projectDeviceInterconnectionOverview(baseInput({
      accountService: connectionServiceFromRelayUrl('https://relay.example.com'),
      dispatchJobs: [
        {
          id: 'job-device',
          state: 'running',
          target: { kind: 'device', id: 'linux-2', name: 'Build Server' },
        },
        {
          id: 'job-ssh',
          state: 'running',
          target: { kind: 'ssh', id: 'archive', name: 'SSH Archive' },
        },
      ],
    }));

    expect(overview.backgroundTaskCount).toBe(1);
    expect(overview.connectedDevices).toEqual([
      expect.objectContaining({
        id: 'device:linux-2',
        name: 'Build Server',
        kind: 'execution-host',
        activities: ['background-execution'],
        backgroundTaskCount: 1,
      }),
    ]);
    expect(overview.devices.some(device => device.name === 'SSH Archive')).toBe(false);
    expect(overview.connectionService?.kind).toBe('self-hosted');
  });

  it('merges current use and background execution for the same peer device', () => {
    const overview = projectDeviceInterconnectionOverview(baseInput({
      peer: { deviceId: 'linux-1', deviceName: 'Linux Workstation' },
      dispatchJobs: [
        {
          id: 'job-1',
          state: 'running',
          target: { kind: 'device', id: 'linux-1', name: 'Linux Workstation' },
        },
      ],
    }));

    expect(overview.devices.filter(device => device.id === 'device:linux-1')).toHaveLength(1);
    expect(overview.primaryDevice.activities).toEqual([
      'current-use',
      'background-execution',
    ]);
    expect(overview.primaryDevice.backgroundTaskCount).toBe(1);
  });
});

describe('connection service classification', () => {
  it('only classifies the canonical BitFun relay host as official', () => {
    expect(classifyAccountRelayUrl('https://remote.openbitfun.com/relay')).toBe('official-relay');
    expect(classifyAccountRelayUrl('https://relay.example.com')).toBe('self-hosted-relay');
    expect(classifyAccountRelayUrl('not a url')).toBe('unknown');
  });
});
