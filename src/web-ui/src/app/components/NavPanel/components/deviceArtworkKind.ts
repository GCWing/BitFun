import type { DeviceOverviewDevice } from '../deviceInterconnectionOverview';

type DeviceArtworkKind = 'device' | 'server' | 'macbook-air';

export function getDeviceArtworkKind(
  device: Pick<DeviceOverviewDevice, 'kind' | 'name'>,
): DeviceArtworkKind {
  if (device.kind === 'execution-host') return 'server';
  // Device names can identify a model, but the controller's OS cannot identify
  // a remote machine. Unrecognized names deliberately use neutral artwork.
  if (device.kind === 'desktop' && /\bmacbook[\s._-]*air\b/i.test(device.name)) {
    return 'macbook-air';
  }
  return 'device';
}
