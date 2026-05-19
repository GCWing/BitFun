import { bundleManager } from "@kit.AbilityKit";
import hilog from "@ohos.hilog";
import filePicker from '@ohos.file.picker';
import fileUri from "@ohos.file.fileuri";
import { Err, Ok, Result } from "./Result";

export class CommonUtils {
  static getBundleName(): string {
    let bundleName = "";
    try {
      bundleName = bundleManager.getBundleInfoForSelfSync(bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT).name;
    } catch (e) {
      hilog.warn(0x0000, 'vnext', 'getBundleName failed' + e);
    }
    return bundleName;
  }

  static async open_file_dialog(): Promise<string> {
    try {
      hilog.info(0x0000, 'vnext', 'open_file_dialog');
      let documentOptions = new filePicker.DocumentSelectOptions;
      documentOptions.defaultFilePathUri = 'file://docs/storage/Users/currentUser';
      documentOptions.selectMode = filePicker.DocumentSelectMode.MIXED;
      documentOptions.maxSelectNumber = 1;
      let documentPicker = new filePicker.DocumentViewPicker();
      hilog.info(0x0000, 'vnext', 'open filePicker success');
      let select = await documentPicker.select(documentOptions);
      let fileUrl = new fileUri.FileUri(select.pop() as string);
      let tmp = fileUrl.path.replace('/storage/Users/currentUser/appdata', '/data/storage');
      let uri = tmp.replace(CommonUtils.getBundleName(), 'base');
      return uri
    } catch (e) {
      return 'open file or folder failed';
    }
  }
}