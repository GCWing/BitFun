import { abilityAccessCtrl, Context, Permissions } from '@kit.AbilityKit';
import pasteboard from '@ohos.pasteboard';
import { OhosError } from './OhosError';

const PASTEBOARD_PERMISSION: Permissions = 'ohos.permission.READ_PASTEBOARD';

export class ClipboardService {
  async readText(context: Context): Promise<string> {
    try {
      await ClipboardService.ensurePasteboardPermission(context);
      const systemPasteboard = pasteboard.getSystemPasteboard();
      const hasData = await systemPasteboard.hasData();
      if (!hasData) {
        return '';
      }

      const atManager = abilityAccessCtrl.createAtManager();
      const tokenId = context.applicationInfo.accessTokenId;
      const grant = await atManager.checkAccessToken(tokenId, PASTEBOARD_PERMISSION);
      if (grant !== abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
        throw new Error('Pasteboard permission denied.');
      }
      const data = await systemPasteboard.getData();
      const recordCount = data.getRecordCount();
      for (let index = 0; index < recordCount; index++) {
        const record = data.getRecord(index);
        const plainText = (record.plainText || '').trim();
        if (plainText.length > 0) {
          return plainText;
        }
        const uri = (record.uri || '').trim();
        if (uri.length > 0) {
          return uri;
        }
      }

      return '';
    } catch (err) {
      throw OhosError.wrap('clipboard read', err);
    }
  }

  async writeText(text: string): Promise<void> {
    try {
      const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text);
      await pasteboard.getSystemPasteboard().setData(data);
    } catch (err) {
      throw OhosError.wrap('clipboard write', err);
    }
  }

  private static async ensurePasteboardPermission(context: Context): Promise<void> {
    try {
      const atManager = abilityAccessCtrl.createAtManager();
      const result = await atManager.requestPermissionsFromUser(context, [PASTEBOARD_PERMISSION]);
      if (result.authResults.length === 0 ||
        result.authResults[0] !== abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
        throw new Error('Pasteboard permission denied.');
      }
    } catch (err) {
      throw OhosError.wrap('clipboard permission', err);
    }
  }
}
