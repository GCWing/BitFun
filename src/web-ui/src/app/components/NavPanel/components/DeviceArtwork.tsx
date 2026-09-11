import type { DeviceOverviewDevice } from '../deviceInterconnectionOverview';
import macbookAir from '../assets/macbook-air.png';

import { getDeviceArtworkKind } from './deviceArtworkKind';

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
