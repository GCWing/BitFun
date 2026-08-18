import { scanBarcode, scanCore } from '@kit.ScanKit';
import { OhosError } from './OhosError';

export class QrScanService {
  async scanRemoteUrl(context: Context): Promise<string> {
    try {
      const result = await scanBarcode.startScanForResult(context, {
        scanTypes: [scanCore.ScanType.QR_CODE],
        enableMultiMode: false,
        enableAlbum: true
      });
      return result.originalValue || '';
    } catch (err) {
      throw OhosError.wrap('qr scan', err);
    }
  }
}
