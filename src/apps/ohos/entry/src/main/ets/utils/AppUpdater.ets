import { common } from '@kit.AbilityKit';
import { updateManager } from '@kit.AppGalleryKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { UpdateStrategy, DialogUpdateStrategy, UpdateStrategyResult } from './UpdateStrategy';

const DOMAIN: number = 0x0000;
const TAG: string = 'AppUpdater';

export interface UpdateCheckResult {
  updateAvailable: boolean;
  errorMessage: string;
}

export class AppUpdater {
  private strategy: UpdateStrategy;

  constructor(strategy?: UpdateStrategy) {
    this.strategy = strategy ?? new DialogUpdateStrategy();
  }

  setStrategy(strategy: UpdateStrategy): void {
    this.strategy = strategy;
  }

  check(context: common.UIAbilityContext): void {
    hilog.info(DOMAIN, TAG, 'auto-update check start (fire-and-forget)');
    this.checkUpdateWithResult(context)
      .then((result: UpdateCheckResult) => {
        if (result.updateAvailable) {
          hilog.info(DOMAIN, TAG, 'auto-update check done, updateAvailable=true, errorMessage=' + result.errorMessage);
        } else {
          hilog.info(DOMAIN, TAG, 'auto-update check done, updateAvailable=false, errorMessage=' + result.errorMessage);
        }
      })
      .catch((error: Error) => {
        hilog.error(DOMAIN, TAG, 'auto-update check unexpected error: ' + JSON.stringify(error));
      });
  }

  async checkUpdateWithResult(context: common.UIAbilityContext): Promise<UpdateCheckResult> {
    hilog.info(DOMAIN, TAG, 'checkAppUpdate start');
    let checkResult: updateManager.CheckUpdateResult;
    try {
      checkResult = await updateManager.checkAppUpdate(context);
    } catch (error) {
      let message: string = (error && error.message) ? error.message : 'checkAppUpdate threw';
      hilog.error(DOMAIN, TAG, 'checkAppUpdate failed: ' + JSON.stringify(error));
      let failResult: UpdateCheckResult = { updateAvailable: false, errorMessage: message };
      return failResult;
    }
    if (checkResult.updateAvailable !== updateManager.UpdateAvailableCode.LATER_VERSION_EXIST) {
      hilog.info(DOMAIN, TAG, 'checkAppUpdate: no update available, code=' + checkResult.updateAvailable);
      let noUpdateResult: UpdateCheckResult = { updateAvailable: false, errorMessage: '' };
      return noUpdateResult;
    }
    hilog.info(DOMAIN, TAG, 'checkAppUpdate: new version available, invoking update strategy');
    let strategyResult: UpdateStrategyResult;
    try {
      strategyResult = await this.strategy.onUpdateAvailable(context);
    } catch (error) {
      let message: string = (error && error.message) ? error.message : 'onUpdateAvailable threw';
      hilog.error(DOMAIN, TAG, 'update strategy unexpected error: ' + JSON.stringify(error));
      let strategyFailResult: UpdateCheckResult = { updateAvailable: true, errorMessage: message };
      return strategyFailResult;
    }
    if (strategyResult.success) {
      hilog.info(DOMAIN, TAG, 'update strategy completed successfully');
      let okResult: UpdateCheckResult = { updateAvailable: true, errorMessage: '' };
      return okResult;
    }
    hilog.warn(DOMAIN, TAG, 'update strategy reported failure: ' + strategyResult.errorMessage);
    let strategyFailResult: UpdateCheckResult = {
      updateAvailable: true,
      errorMessage: strategyResult.errorMessage
    };
    return strategyFailResult;
  }
}
