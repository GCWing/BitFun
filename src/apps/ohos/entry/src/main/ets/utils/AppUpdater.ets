import { common } from '@kit.AbilityKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { updateManager } from '@kit.AppGalleryKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { UpdateStrategy, DialogUpdateStrategy } from './UpdateStrategy';

export class AppUpdater {
  private strategy: UpdateStrategy;

  constructor(strategy?: UpdateStrategy) {
    this.strategy = strategy ?? new DialogUpdateStrategy();
  }

  setStrategy(strategy: UpdateStrategy): void {
    this.strategy = strategy;
  }

  check(context: common.UIAbilityContext): void {
    try {
      updateManager.checkAppUpdate(context)
        .then((checkResult: updateManager.CheckUpdateResult) => {
          if (checkResult.updateAvailable === updateManager.UpdateAvailableCode.LATER_VERSION_EXIST) {
            hilog.info(0x0000, 'vnext', 'New version available');
            this.strategy.onUpdateAvailable(context);
          } else {
            hilog.info(0x0000, 'vnext', 'No update available');
          }
        })
        .catch((error: BusinessError) => {
          hilog.error(0x0000, 'vnext', 'checkAppUpdate error: ' + JSON.stringify(error));
        });
    } catch (error) {
      hilog.error(0x0000, 'vnext', 'checkAppUpdate exception: ' + JSON.stringify(error));
    }
  }
}
