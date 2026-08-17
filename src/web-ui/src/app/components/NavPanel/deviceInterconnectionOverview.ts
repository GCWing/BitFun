import type { RemoteConnectStatus } from '@/infrastructure/api/service-api/RemoteConnectAPI';

export type DeviceOverviewMode = 'local' | 'connected';
export type DeviceOverviewDeviceKind =
  | 'desktop'
  | 'mobile'
  | 'execution-host'
  | 'message-app';
export type DeviceOverviewActivity =
  | 'current-use'
  | 'controlling'
  | 'background-execution';
export type DeviceOverviewConnectionServiceKind =
  | 'official'
  | 'self-hosted'
  | 'local-network'
  | 'public-tunnel'
  | 'device-service'
  | 'message-app';

export interface DeviceOverviewDevice {
  id: string;
  name: string;
  kind: DeviceOverviewDeviceKind;
  local: boolean;
  activities: DeviceOverviewActivity[];
  backgroundTaskCount: number;
}

export interface DeviceOverviewConnectionService {
  kind: DeviceOverviewConnectionServiceKind;
  url: string | null;
  host: string | null;
}

export interface DeviceInterconnectionOverview {
  mode: DeviceOverviewMode;
  localDeviceName: string;
  currentWorkDeviceName: string;
  primaryDevice: DeviceOverviewDevice;
  connectedDevices: DeviceOverviewDevice[];
  devices: DeviceOverviewDevice[];
  connectionService: DeviceOverviewConnectionService | null;
  controllerCount: number;
  backgroundTaskCount: number;
  peerActive: boolean;
  topologyUnavailable: boolean;
}

export interface DeviceOverviewDispatchJob {
  id: string;
  state: 'submitting' | 'submission_unknown' | 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';
  target:
    | { kind: 'local' }
    | { kind: 'ssh'; id: string; name: string }
    | { kind: 'device'; id: string; name: string };
}

export interface DeviceInterconnectionOverviewInput {
  localDeviceName: string;
  peer: { deviceId: string; deviceName: string } | null;
  remoteStatus: RemoteConnectStatus | null;
  remoteStatusState: 'loading' | 'ready' | 'unavailable';
  dispatchJobs: DeviceOverviewDispatchJob[];
  accountService: DeviceOverviewConnectionService | null;
}

const ACTIVE_DISPATCH_STATES = new Set<DeviceOverviewDispatchJob['state']>([
  'submitting',
  'submission_unknown',
  'queued',
  'running',
]);

const OFFICIAL_RELAY_HOST = 'remote.openbitfun.com';

export function connectionServiceFromRelayUrl(
  relayUrl: string | null | undefined,
): DeviceOverviewConnectionService | null {
  const value = relayUrl?.trim();
  if (!value) return null;

  try {
    const parsed = new URL(value);
    const host = parsed.host.toLowerCase();
    return {
      kind: parsed.hostname.toLowerCase() === OFFICIAL_RELAY_HOST
        ? 'official'
        : 'self-hosted',
      url: value,
      host,
    };
  } catch {
    return null;
  }
}

export function classifyAccountRelayUrl(
  relayUrl: string | null | undefined,
): 'official-relay' | 'self-hosted-relay' | 'unknown' {
  const service = connectionServiceFromRelayUrl(relayUrl);
  if (!service) return 'unknown';
  return service.kind === 'official' ? 'official-relay' : 'self-hosted-relay';
}

function connectionServiceFromActiveMethod(
  activeMethod: string | null,
): DeviceOverviewConnectionService {
  const method = activeMethod?.trim().toLowerCase() ?? '';
  if (method.startsWith('lan')) {
    return { kind: 'local-network', url: null, host: null };
  }
  if (method.startsWith('ngrok')) {
    return { kind: 'public-tunnel', url: null, host: null };
  }
  if (method.startsWith('bitfunserver')) {
    return { kind: 'official', url: null, host: OFFICIAL_RELAY_HOST };
  }
  if (method.startsWith('customserver')) {
    return { kind: 'self-hosted', url: null, host: null };
  }
  return { kind: 'device-service', url: null, host: null };
}

function messageApplicationName(botConnected: string): string | undefined {
  const name = botConnected.split('(', 1)[0]?.trim();
  return name || undefined;
}

function addActivity(
  device: DeviceOverviewDevice,
  activity: DeviceOverviewActivity,
): void {
  if (!device.activities.includes(activity)) {
    device.activities.push(activity);
  }
}

function addOrMergeDevice(
  devices: DeviceOverviewDevice[],
  device: DeviceOverviewDevice,
): DeviceOverviewDevice {
  const existing = devices.find(item => item.id === device.id);
  if (!existing) {
    devices.push(device);
    return device;
  }

  for (const activity of device.activities) {
    addActivity(existing, activity);
  }
  existing.backgroundTaskCount += device.backgroundTaskCount;
  if (device.kind === 'execution-host' && existing.kind === 'desktop') {
    existing.kind = 'execution-host';
  }
  return existing;
}

export function projectDeviceInterconnectionOverview(
  input: DeviceInterconnectionOverviewInput,
): DeviceInterconnectionOverview {
  const localDeviceName = input.localDeviceName.trim();
  const currentWorkDeviceName = input.peer?.deviceName.trim() || localDeviceName;
  const devices: DeviceOverviewDevice[] = [];

  const primaryDevice = addOrMergeDevice(devices, {
    id: input.peer ? `device:${input.peer.deviceId}` : 'device:local',
    name: currentWorkDeviceName,
    kind: 'desktop',
    local: input.peer === null,
    activities: ['current-use'],
    backgroundTaskCount: 0,
  });

  if (input.peer) {
    addOrMergeDevice(devices, {
      id: 'device:local',
      name: localDeviceName,
      kind: 'desktop',
      local: true,
      activities: ['controlling'],
      backgroundTaskCount: 0,
    });
  }

  let connectionService = input.peer ? input.accountService : null;

  if (input.remoteStatus?.is_connected) {
    addOrMergeDevice(devices, {
      id: `mobile:${input.remoteStatus.peer_user_id ?? input.remoteStatus.peer_device_name ?? 'connected'}`,
      name: input.remoteStatus.peer_device_name?.trim() || 'Mobile device',
      kind: 'mobile',
      local: false,
      activities: ['controlling'],
      backgroundTaskCount: 0,
    });
    connectionService ??= connectionServiceFromActiveMethod(input.remoteStatus.active_method);
  }

  if (input.remoteStatus?.bot_connected) {
    const applicationName = messageApplicationName(input.remoteStatus.bot_connected);
    addOrMergeDevice(devices, {
      id: `message-app:${applicationName ?? 'connected'}`,
      name: applicationName ?? 'Message app',
      kind: 'message-app',
      local: false,
      activities: ['controlling'],
      backgroundTaskCount: 0,
    });
    connectionService ??= {
      kind: 'message-app',
      url: null,
      host: applicationName ?? null,
    };
  }

  let backgroundTaskCount = 0;
  for (const job of input.dispatchJobs) {
    // SSH is a remote-workspace / transport concern, not a BitFun device.
    // Only a same-account BitFun host belongs in this overview.
    if (!ACTIVE_DISPATCH_STATES.has(job.state) || job.target.kind !== 'device') {
      continue;
    }

    backgroundTaskCount += 1;
    const device = addOrMergeDevice(devices, {
      id: `device:${job.target.id}`,
      name: job.target.name,
      kind: 'execution-host',
      local: false,
      activities: ['background-execution'],
      backgroundTaskCount: 1,
    });
    addActivity(device, 'background-execution');
    connectionService ??= input.accountService;
  }

  const connectedDevices = devices.filter(device => device.id !== primaryDevice.id);
  const controllerCount = devices.filter(device => (
    device.activities.includes('controlling')
  )).length;
  const mode: DeviceOverviewMode = connectedDevices.length > 0 || input.peer !== null
    ? 'connected'
    : 'local';

  return {
    mode,
    localDeviceName,
    currentWorkDeviceName,
    primaryDevice,
    connectedDevices,
    devices,
    connectionService: mode === 'connected' ? connectionService : null,
    controllerCount,
    backgroundTaskCount,
    peerActive: input.peer !== null,
    topologyUnavailable: mode === 'connected' && input.remoteStatusState === 'unavailable',
  };
}
