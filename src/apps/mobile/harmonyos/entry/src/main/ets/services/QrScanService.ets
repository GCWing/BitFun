import { scanBarcode, scanCore } from '@kit.ScanKit';

export class QrScanService {
  async scanRemoteUrl(context: Context): Promise<string> {
    const result = await scanBarcode.startScanForResult(context, {
      scanTypes: [scanCore.ScanType.QR_CODE],
      enableMultiMode: false,
      enableAlbum: true
    });
    return result.originalValue || '';
  }
}
