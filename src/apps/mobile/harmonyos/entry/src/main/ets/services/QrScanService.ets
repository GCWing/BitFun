import { abilityAccessCtrl, Context, Permissions } from '@kit.AbilityKit';
import { customScan, scanBarcode, scanCore } from '@kit.ScanKit';
import { OhosError } from './OhosError';

const CAMERA_PERMISSION: Permissions = 'ohos.permission.CAMERA';

export class QrScanService {
  private inlineScanning: boolean = false;

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

  async ensureCameraPermission(context: Context): Promise<void> {
    try {
      const atManager = abilityAccessCtrl.createAtManager();
      const result = await atManager.requestPermissionsFromUser(context, [CAMERA_PERMISSION]);
      if (result.authResults.length > 0 &&
        result.authResults[0] === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
        return;
      }
    } catch (err) {
      throw OhosError.wrap('camera permission', err);
    }
    throw new Error('Camera permission denied.');
  }

  startInlineScan(
    surfaceId: string,
    onDetected: (value: string) => void,
    onError: (err: Object) => void
  ): void {
    if (this.inlineScanning) {
      return;
    }
    try {
      customScan.init({
        scanTypes: [scanCore.ScanType.QR_CODE],
        enableMultiMode: false,
        enableAlbum: false
      });
      this.inlineScanning = true;
      customScan.start({
        width: 282,
        height: 282,
        surfaceId
      }, (err, results: Array<scanBarcode.ScanResult>) => {
        if (err) {
          onError(OhosError.wrap('inline qr scan', err));
          void this.stopInlineScan();
          return;
        }
        if (!results || results.length === 0) {
          return;
        }
        const value = (results[0].originalValue || '').trim();
        if (value.length > 0) {
          onDetected(value);
        }
      });
    } catch (err) {
      onError(OhosError.wrap('inline qr scan', err));
      void this.stopInlineScan();
    }
  }

  async stopInlineScan(): Promise<void> {
    if (!this.inlineScanning) {
      return;
    }
    this.inlineScanning = false;
    try {
      await customScan.stop();
    } catch (_err) {
    }
    try {
      await customScan.release();
    } catch (_err) {
    }
  }
}
