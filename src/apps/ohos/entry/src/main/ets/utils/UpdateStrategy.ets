import { common } from '@kit.AbilityKit';
import { updateManager } from '@kit.AppGalleryKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

const DOMAIN: number = 0x0000;
const TAG: string = 'UpdateStrategy';

export interface UpdateStrategyResult {
  success: boolean;
  errorMessage: string;
}

export interface UpdateStrategy {
  onUpdateAvailable(context: common.UIAbilityContext): Promise<UpdateStrategyResult>;
}

export class DialogUpdateStrategy implements UpdateStrategy {
  async onUpdateAvailable(context: common.UIAbilityContext): Promise<UpdateStrategyResult> {
    hilog.info(DOMAIN, TAG, 'showUpdateDialog start');
    let resultCode: updateManager.ShowUpdateResultCode;
    try {
      resultCode = await updateManager.showUpdateDialog(context);
    } catch (error) {
      let message: string = (error && error.message) ? error.message : 'showUpdateDialog threw';
      hilog.error(DOMAIN, TAG, 'showUpdateDialog failed: ' + JSON.stringify(error));
      let failResult: UpdateStrategyResult = { success: false, errorMessage: message };
      return failResult;
    }
    if (resultCode === updateManager.ShowUpdateResultCode.SHOW_DIALOG_SUCCESS) {
      hilog.info(DOMAIN, TAG, 'showUpdateDialog success, resultCode: ' + resultCode);
      let okResult: UpdateStrategyResult = { success: true, errorMessage: '' };
      return okResult;
    }
    hilog.warn(DOMAIN, TAG, 'showUpdateDialog returned non-success, resultCode: ' + resultCode);
    let nonOkResult: UpdateStrategyResult = {
      success: false,
      errorMessage: 'showUpdateDialog non-success resultCode: ' + resultCode
    };
    return nonOkResult;
  }
}

export class SilentUpdateStrategy implements UpdateStrategy {
  async onUpdateAvailable(context: common.UIAbilityContext): Promise<UpdateStrategyResult> {
    hilog.info(DOMAIN, TAG, 'SilentUpdateStrategy: download in background');
    let result: UpdateStrategyResult = { success: true, errorMessage: '' };
    return result;
  }
}
