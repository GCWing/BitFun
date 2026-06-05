import { common } from '@kit.AbilityKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { updateManager } from '@kit.AppGalleryKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

export interface UpdateStrategy {
  onUpdateAvailable(context: common.UIAbilityContext): void;
}

export class DialogUpdateStrategy implements UpdateStrategy {
  onUpdateAvailable(context: common.UIAbilityContext): void {
    updateManager.showUpdateDialog(context)
      .then((resultCode: updateManager.ShowUpdateResultCode) => {
        hilog.info(0x0000, 'vnext', 'showUpdateDialog result: ' + resultCode);
      })
      .catch((error: BusinessError) => {
        hilog.error(0x0000, 'vnext', 'showUpdateDialog error: ' + JSON.stringify(error));
      });
  }
}

export class SilentUpdateStrategy implements UpdateStrategy {
  onUpdateAvailable(context: common.UIAbilityContext): void {
    hilog.info(0x0000, 'vnext', 'SilentUpdateStrategy: download in background');
    // TODO: implement silent download + restart prompt
  }
}
