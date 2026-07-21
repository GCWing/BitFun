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

  static async open_file_dialog(arg: string): Promise<string> {
    // Options JSON shape (aligned with @tauri-apps/plugin-dialog OpenDialogOptions):
    //   { multiple?: boolean, directory?: boolean, filters?: { name: string, extensions: string[] }[] }
    // `directory` is tri-state: true→FOLDER, false→FILE, undefined→MIXED (legacy default).
    // `filters[].extensions` are dot-less; OHOS `fileSuffixFilters` wants a leading dot.
    // Multi-filter groups are flattened into a single suffix array (OHOS has no group UI).
    // Returns JSON: { paths: string[] } on success, { paths: [] } on cancel, { error: string } on failure.
    let opts: PickerOptions = {};
    try {
      if (arg && arg.length > 0) {
        opts = JSON.parse(arg) as PickerOptions;
      }
    } catch (e) {
      hilog.warn(0x0000, 'vnext', 'open_file_dialog: invalid options json, fallback to defaults: ' + e);
    }

    const multiple: boolean = opts.multiple === true;
    // tri-state: true→FOLDER, false→FILE, undefined→MIXED
    const selectMode: number = opts.directory === true
      ? filePicker.DocumentSelectMode.FOLDER
      : opts.directory === false
        ? filePicker.DocumentSelectMode.FILE
        : filePicker.DocumentSelectMode.MIXED;

    try {
      hilog.info(0x0000, 'vnext', `open_file_dialog: multiple=${multiple}, mode=${selectMode}`);
      const documentOptions = new filePicker.DocumentSelectOptions;
      documentOptions.defaultFilePathUri = 'file://docs/storage/Users/currentUser';
      documentOptions.selectMode = selectMode;
      documentOptions.maxSelectNumber = multiple ? 500 : 1;

      // File-type filters only apply when not in FOLDER mode. Flatten all filter groups
      // into one suffix list, each with a leading dot.
      if (selectMode !== filePicker.DocumentSelectMode.FOLDER && opts.filters && opts.filters.length > 0) {
        const suffixes: string[] = [];
        for (const f of opts.filters) {
          const exts = f?.extensions ?? [];
          for (const ext of exts) {
            const normalized = ext.startsWith('.') ? ext : '.' + ext;
            if (suffixes.indexOf(normalized) < 0) {
              suffixes.push(normalized);
            }
          }
        }
        if (suffixes.length > 0) {
          documentOptions.fileSuffixFilters = suffixes;
        }
      }

      const documentPicker = new filePicker.DocumentViewPicker();
      hilog.info(0x0000, 'vnext', 'open filePicker success');
      const select: string[] = await documentPicker.select(documentOptions);
      if (!select || select.length === 0) {
        // user cancelled
        return JSON.stringify({ paths: [] });
      }
      const paths = select.map((uri: string) => {
        const fileUrl = new fileUri.FileUri(uri);
        return fileUrl.path
      });
      return JSON.stringify({ paths });
    } catch (e) {
      return JSON.stringify({ error: 'open file or folder failed: ' + e });
    }
  }
}

interface PickerFilter {
  name: string;
  extensions: string[];
}

interface PickerOptions {
  multiple?: boolean;
  directory?: boolean;
  filters?: PickerFilter[];
}