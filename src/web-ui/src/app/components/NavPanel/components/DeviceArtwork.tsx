import type { DeviceOverviewDevice } from '../deviceInterconnectionOverview';
import macbookAir from '../assets/macbook-air.png';

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

export function DeviceArtwork({ device }: { device: DeviceOverviewDevice }) {
  const artwork = getDeviceArtworkKind(device);
  return (
    <div className="openbitfun-device-overview__artwork" data-artwork={artwork} aria-hidden="true">
      {artwork === 'macbook-air'
        ? <img src={macbookAir} alt="" width={202} height={202} draggable={false} />
        : <span className="openbitfun-device-overview__artwork-glyph" />}
    </div>
  );
}
