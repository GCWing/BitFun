import common from "@ohos.app.ability.common";
import { JSON } from "@kit.ArkTS";
import hilog from "@ohos.hilog";
import Want from "@ohos.app.ability.Want";
import { CommonUtils } from "./CommonUtils";
import { fileIo } from "@kit.CoreFileKit";

let srcDirPathLocal: string =
  '/storage/Users/currentUser/Documents/DevecoStudioProjects/MyApplication/build-profile.json5'

export function runDeveco(context: common.UIAbilityContext, workspace: string): void {
  let args = defaultStartArgs();
  args.workspace = workspace;
  args.build_after_start = true;
  startNewInstance(context, args)
}

export function copySignFile(path: string): void {
  let dstDirPathLocal: string = path + "/build-profile.json5";
  try {
    fileIo.copyFile(srcDirPathLocal, dstDirPathLocal, 0).then(() => {
      hilog.info(0x0000, 'vnext', 'copyFile success ');
    }).catch((err: BusinessError) => {
      hilog.info(0x0000, 'vnext', 'copyFile error: ' + JSON.stringify(err));
    })
  } catch (err) {
    hilog.info(0x0000, 'vnext', 'copyFile error: ' + JSON.stringify(err));
  }
}

export function startNewInstance(context: common.UIAbilityContext, args: StartArgs | undefined): void {
  hilog.info(0x0000, 'vnext', 'startNewInstance ' + JSON.stringify(args));
  const want: Want = {
    bundleName: 'com.huawei.devecostudio',
    abilityName: 'EntryAbility',
    parameters: {
      'startParameters': JSON.stringify(args),
    }
  };
  context.startAbility(want).then(() => {
    hilog.info(0x0000, 'vnext', 'startAbility success ');
  }).catch((error: BusinessError) => {
    hilog.warn(0x0000, 'vnext', `startAbility failed: code: ${error.code}, msg: ${error.message} `);
  });

}

export interface StartArgs {
  workspace: string,
  opened_files: string,
  window_state: WindowState,
  x: number | undefined,
  y: number | undefined,
  width: number | undefined,
  height: number | undefined,
  restart: boolean,
  build_after_start: boolean
}

export enum WindowState {
  Restored = "Restored",
  Maximized = "Maximized",
  Minimized = "Minimized"
}

export function defaultStartArgs(): StartArgs {
  return {
    workspace: "",
    opened_files: "",
    window_state: WindowState.Restored,
    x: undefined,
    y: undefined,
    width: undefined,
    height: undefined,
    restart: false,
    build_after_start: false
  }
}