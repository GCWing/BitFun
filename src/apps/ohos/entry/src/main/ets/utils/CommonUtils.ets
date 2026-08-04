import { bundleManager } from "@kit.AbilityKit";
import hilog from "@ohos.hilog";
import filePicker from '@ohos.file.picker';
import fileUri from "@ohos.file.fileuri";
import pasteboard from '@ohos.pasteboard';
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

  /// Normalize a docs:// (or file://) URI from the picker / pasteboard into the
  /// sandbox-relative path the Rust side expects. Mirrors the two-step
  /// normalization that originally lived inline in `open_file_dialog`:
  ///   1. `fileUri.FileUri(uri).path` — strip the `file://docs/...` prefix.
  ///   2. remap appdata → /data/storage, and the bundle name → `base`.
  /// Extracted so the picker and the clipboard reader stay in sync.
  static normalizeFileUri(uri: string): string {
    const fileUrl = new fileUri.FileUri(uri);
    const bundleName = CommonUtils.getBundleName();
    const tmp = fileUrl.path.replace('/storage/Users/currentUser/appdata', '/data/storage');
    return tmp.replace(bundleName, 'base');
  }

  /// Read file paths from the OHOS system pasteboard.
  ///
  /// When the user taps "复制" (copy) in `DocumentViewPicker`, the picker
  /// writes the selected file URIs to the system pasteboard. This reads them
  /// back so the project's paste flow can reach them.
  ///
  /// Iterates every record and collects file URIs from both the legacy `uri` /
  /// `plainText` fields and API 14+ URI/plain-text MIME entries. Some system
  /// file managers store file lists only in those additional entries. Each
  /// candidate is run through the same normalization as the picker. Returns the
  /// same envelope as `open_file_dialog`: `{ paths: string[] }` /
  /// `{ paths: [] }` (no files) / `{ error: string }`.
  static async read_clipboard_files(): Promise<string> {
    try {
      const systemPasteboard = pasteboard.getSystemPasteboard();
      const hasData = await systemPasteboard.hasData();
      if (!hasData) {
        return JSON.stringify({ paths: [] });
      }

      const data = await systemPasteboard.getData();
      const recordCount = data.getRecordCount();
      const paths: string[] = [];
      const seen = new Set<string>();
      for (let index = 0; index < recordCount; index++) {
        const record = data.getRecord(index);
        const candidates: string[] = [];
        const addStringCandidates = (value: string): void => {
          for (const line of value.split(/\r?\n/)) {
            const trimmed = line.trim();
            if (trimmed.length > 0) {
              candidates.push(trimmed);
            }
          }
        };

        addStringCandidates(record.uri || '');
        addStringCandidates(record.plainText || '');

        // File Manager can place URI and plain-text values in additional MIME
        // entries. Those values are only available through getData().
        const supportedTypes = record.getValidTypes([
          pasteboard.MIMETYPE_TEXT_URI,
          pasteboard.MIMETYPE_TEXT_PLAIN
        ]);
        for (const type of supportedTypes) {
          try {
            const value = await record.getData(type);
            if (typeof value === 'string') {
              addStringCandidates(value);
            }
          } catch (e) {
            hilog.warn(0x0000, 'vnext', 'read_clipboard_files: failed to read mime entry ' + type + ': ' + e);
          }
        }

        for (const candidate of candidates) {
          if (seen.has(candidate)) {
            continue;
          }
          // Only accept things that look like a file/docs URI; plain text
          // snippets (e.g. a copied URL) must not become a "file path".
          if (!candidate.startsWith('file://') && !candidate.startsWith('docs://')) {
            continue;
          }
          seen.add(candidate);
          try {
            paths.push(CommonUtils.normalizeFileUri(candidate));
          } catch (e) {
            hilog.warn(0x0000, 'vnext', 'read_clipboard_files: skip un-normalizable uri: ' + candidate + ' ' + e);
          }
        }
      }

      return JSON.stringify({ paths });
    } catch (e) {
      return JSON.stringify({ error: 'read clipboard files failed: ' + e });
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