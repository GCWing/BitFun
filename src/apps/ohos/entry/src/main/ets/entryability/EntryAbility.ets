import {
  AbilityConstant,
  common,
  Configuration,
  ConfigurationConstant,
  Want
} from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window, display } from '@kit.ArkUI';
import { RustAbility } from '@ohos-rs/ability';
import { CommonEventListener } from '../utils/CommonEventListener';
import { harmonyShare, systemShare } from '@kit.ShareKit';
import { fileIo, fileUri } from '@kit.CoreFileKit';
import { uniformTypeDescriptor } from '@kit.ArkData';
import { calendarManager } from '@kit.CalendarKit';
import { BusinessError } from '@kit.BasicServicesKit';
import RustModule from 'libbitfun_desktop_lib.so';
import { AppUpdater, UpdateCheckResult } from '../utils/AppUpdater';
import { CommonUtils } from '../utils/CommonUtils';
import { runDeveco } from '../utils/DevecoStart';
import { VoiceInputService } from '../services/VoiceInputService';
import { FeedbackCredentialStore } from '../utils/FeedbackCredentialStore';
import { MarketCredentialStore } from '../utils/MarketCredentialStore';
import { WindowHostRequest, WindowHostService } from '../services/WindowHostService';
import {
  BrowserWebviewService,
  CreateRequest,
  EvalRequest,
  NavigateRequest,
  BoundsRequest,
  LabelRequest,
} from '../services/BrowserWebviewService';


const DOMAIN = 0x0000;

interface SetAlwaysOnTopArg {
  isTopmost: boolean;
}
interface SetDecorationsArg {
  visible: boolean;
}
interface SetSkipTaskbarArg {
  skip: boolean;
}
interface SetWindowSizeArg {
  width: number;
  height: number;
}
interface SetWindowPositionArg {
  x: number;
  y: number;
}
interface SetMinSizeArg {
  width: number;
  height: number;
}
interface SetResizableArg {
  resizable: boolean;
}
interface PointResult {
  x: number;
  y: number;
}
interface SizeResult {
  width: number;
  height: number;
}
interface MonitorResult {
  width: number;
  height: number;
  scaleFactor: number;
}

export default class EntryAbility extends RustAbility {
  public moduleName: string = "bitfun_desktop_lib";
  public defaultPage: boolean = true;
  public commonEventListener: CommonEventListener | undefined = undefined;
  public remoteUrl: string = "";
  private appUpdater: AppUpdater = new AppUpdater();
  private voiceInputService: VoiceInputService = new VoiceInputService();
  public shareStatus: boolean = false;

  async onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): Promise<void> {
    super.onCreate(want, launchParam);
    this.commonEventListener = new CommonEventListener();
    // Cold-start best-effort report of the current system color mode so the
    // web-ui "follow system" selection resolves correctly without waiting for
    // the first system change. web-ui dedups when it equals the resolved theme.
    this.notifySystemColorMode();
  }

  onDestroy(): void {
    RustModule.ohosMarkCleanShutdown();
    hilog.info(DOMAIN, 'testTag', '%{public}s', 'Ability onDestroy');
  }

  /**
   * HarmonyOS calls this when the system configuration changes, including the
   * light/dark color mode. We forward the new mode to the native module, which
   * emits an event the web-ui listens for to re-resolve "follow system" live —
   * the OHOS webview does not fire prefers-color-scheme change events on its own.
   */
  onConfigurationUpdate(newConfig: Configuration): void {
    const mode = this.colorModeToString(newConfig?.colorMode);
    if (mode.length > 0) {
      try {
        RustModule.notifySystemColorMode(mode);
        hilog.info(DOMAIN, 'vnext', 'onConfigurationUpdate: system color mode -> ' + mode);
      } catch (e) {
        hilog.warn(DOMAIN, 'vnext', 'notifySystemColorMode failed: ' + JSON.stringify(e));
      }
    }
  }

  /** Read the current Ability config color mode and push it to the web-ui. */
  private notifySystemColorMode(): void {
    const mode = this.readSystemColorMode();
    try {
      RustModule.notifySystemColorMode(mode);
    } catch (e) {
      hilog.warn(DOMAIN, 'vnext', 'notifySystemColorMode failed: ' + JSON.stringify(e));
    }
  }

  /** Map a ConfigurationConstant.ColorMode value to 'light' | 'dark' (empty if unset). */
  private colorModeToString(colorMode: number | undefined): string {
    if (colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_LIGHT) {
      return 'light';
    }
    if (colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK) {
      return 'dark';
    }
    return '';
  }

  onWindowStageCreate(windowStage: window.WindowStage): Promise<void> {
    AppGlobal.calendarMgr = calendarManager.getCalendarManager(this.context);
    AppGlobal.mContext = this.context;
    const DOMAIN_NUMBER: number = 0xFF00;
    const TAG: string = '[BitfunUIAbilityComponents]';
    const feedbackCredentialStore = new FeedbackCredentialStore();
    const marketCredentialStore = new MarketCredentialStore();
    const windowHostService = new WindowHostService(windowStage, () => {
      this.context.terminateSelf(() => {
        hilog.info(DOMAIN_NUMBER, TAG, 'WebDriver requested Ability termination');
      });
    });
    RustModule.registerArktsFunction('webdriver_window_ohos', async (
      _err: Error,
      arg: string,
    ): Promise<string> => {
      try {
        const request: WindowHostRequest = JSON.parse(arg) as WindowHostRequest;
        return await windowHostService.handle(request);
      } catch (error) {
        const businessError: BusinessError = error as BusinessError;
        return JSON.stringify({
          ok: false,
          error: businessError.message ?? 'Invalid WebDriver window request',
        });
      }
    });
    RustModule.registerArktsFunction('feedback_secure_credentials', async (
      _err: Error,
      arg: string
    ): Promise<string> => feedbackCredentialStore.handle(arg));
    RustModule.registerArktsFunction('market_secure_credentials', async (
      _err: Error,
      arg: string
    ): Promise<string> => marketCredentialStore.handle(arg));
    RustModule.registerArktsFunction('open_dialog_file', async (err: Error, arg: string): Promise<string> => {
      let res = await CommonUtils.open_file_dialog(arg);
      return res;
    });
    RustModule.registerArktsFunction('get_clipboard_files', async (err: Error, _arg: string): Promise<string> => {
      let res = await CommonUtils.read_clipboard_files();
      return res;
    });
    RustModule.registerArktsFunction('handle_min_window', async (err: Error, arg: string): Promise<string> => {
      windowStage.getMainWindowSync().minimize();
      return '';
    });
    RustModule.registerArktsFunction('handle_max_window', async (err: Error, arg: string): Promise<string> => {
      windowStage.getMainWindowSync().maximize();
      return '';
    });
    RustModule.registerArktsFunction('handle_restore_window', async (err: Error, arg: string): Promise<string> => {
      windowStage.getMainWindowSync().recover();
      return '';
    });
    RustModule.registerArktsFunction('window_is_maximized', async (err: Error, arg: string): Promise<string> => {
      let state = windowStage.getMainWindowSync().getWindowStatus();
      if (state == window.WindowStatusType.FULL_SCREEN || state == window.WindowStatusType.MAXIMIZE) {
        return 'true';
      } else {
        return 'false';
      }
    });
    RustModule.registerArktsFunction('check_app_update_ohos', async (err: Error, _arg: string): Promise<string> => {
      hilog.info(0x0000, 'vnext', 'check_app_update_ohos invoked');
      let result: UpdateCheckResult;
      try {
        result = await this.appUpdater.checkUpdateWithResult(this.context);
      } catch (error) {
        let message: string = (error && error.message) ? error.message : 'unexpected error';
        hilog.error(0x0000, 'vnext', 'check_app_update_ohos unexpected error: ' + JSON.stringify(error));
        return '{"updateAvailable":false,"error":' + JSON.stringify(message) + '}';
      }
      hilog.info(0x0000, 'vnext', 'check_app_update_ohos done: updateAvailable=' + result.updateAvailable
        + ', errorMessage=' + result.errorMessage);
      if (result.errorMessage.length > 0) {
        return '{"updateAvailable":' + (result.updateAvailable ? 'true' : 'false')
          + ',"error":' + JSON.stringify(result.errorMessage) + '}';
      }
      return '{"updateAvailable":' + (result.updateAvailable ? 'true' : 'false') + '}';
    });
    RustModule.registerArktsFunction('open_browser', async (err: Error, url: string): Promise<string> => {
      try {
        let want : Want = {
          "action": "ohos.want.action.viewData",
          "entities": ["entity.system.browsable"],
          "uri": url
        }
        await this.context.startAbility(want);
        hilog.info(201, 'vnext', `open url succeed: ${url}`);
        return `start ability succeed with ${url}`;
      } catch (error) {
        hilog.info(201, 'vnext', `fail to open ${url} ${error.code}`);
        return `fail to open ${url} ${error.code}`;
      }
    });
    RustModule.registerArktsFunction('window_is_minimized', async (err: Error, arg: string): Promise<string> => {
      let state = windowStage.getMainWindowSync().getWindowStatus();
      if (state == window.WindowStatusType.MINIMIZE) {
        return 'true';
      } else {
        return 'false';
      }
    });
    RustModule.registerArktsFunction('close_window', async (err: Error, arg: string): Promise<string> => {
      this.context.terminateSelf(() => {
        hilog.info(DOMAIN_NUMBER, TAG, 'terminateSelf success');
      });
      return '';
    });
    RustModule.registerArktsFunction('window_start_dragging', async (err: Error, arg: string): Promise<string> => {
      windowStage.getMainWindowSync().startMoving();
      return '';
    });
    RustModule.registerArktsFunction('set_always_on_top_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: SetAlwaysOnTopArg = JSON.parse(arg);
      const mainWin = windowStage.getMainWindowSync();
      try {
        await mainWin.setWindowTopmost(req.isTopmost);
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `setWindowTopmost failed (code=${be.code}), falling back to showWindow: ${be.message}`);
        try {
          await mainWin.showWindow();
        } catch (e2) {
          const be2 = e2 as BusinessError;
          hilog.warn(DOMAIN, TAG, `showWindow fallback failed: ${be2.message}`);
        }
      }
      return '';
    });
    RustModule.registerArktsFunction('set_decorations_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: SetDecorationsArg = JSON.parse(arg);
      try {
        await windowStage.getMainWindowSync().setWindowDecorVisible(req.visible);
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `setWindowDecorVisible failed: ${be.message}`);
      }
      return '';
    });
    RustModule.registerArktsFunction('set_skip_taskbar_ohos', async (err: Error, arg: string): Promise<string> => {
      return '';
    });
    RustModule.registerArktsFunction('set_window_size_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: SetWindowSizeArg = JSON.parse(arg);
      try {
        await windowStage.getMainWindowSync().resize(req.width, req.height);
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `resize failed: ${be.message}`);
      }
      return '';
    });
    RustModule.registerArktsFunction('set_window_position_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: SetWindowPositionArg = JSON.parse(arg);
      try {
        await windowStage.getMainWindowSync().moveWindowTo(req.x, req.y);
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `moveWindowTo failed: ${be.message}`);
      }
      return '';
    });
    RustModule.registerArktsFunction('outer_position_ohos', async (err: Error, arg: string): Promise<string> => {
      const rect = windowStage.getMainWindowSync().getWindowProperties().windowRect;
      const result: PointResult = { x: rect.left, y: rect.top };
      return JSON.stringify(result);
    });
    RustModule.registerArktsFunction('outer_size_ohos', async (err: Error, arg: string): Promise<string> => {
      const rect = windowStage.getMainWindowSync().getWindowProperties().windowRect;
      const result: SizeResult = { width: rect.width, height: rect.height };
      return JSON.stringify(result);
    });
    RustModule.registerArktsFunction('inner_size_ohos', async (err: Error, arg: string): Promise<string> => {
      const rect = windowStage.getMainWindowSync().getWindowProperties().windowRect;
      const result: SizeResult = { width: rect.width, height: rect.height };
      return JSON.stringify(result);
    });
    RustModule.registerArktsFunction('current_monitor_ohos', async (err: Error, arg: string): Promise<string> => {
      let dispWidth: number = 0;
      let dispHeight: number = 0;
      let scale: number = 1;
      try {
        const disp = display.getDefaultDisplaySync();
        dispWidth = disp.width;
        dispHeight = disp.height;
        scale = disp.densityPixels > 0 ? disp.densityPixels : 1;
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `getDefaultDisplaySync failed: ${be.message}`);
      }
      const result: MonitorResult = { width: dispWidth, height: dispHeight, scaleFactor: scale };
      return JSON.stringify(result);
    });
    RustModule.registerArktsFunction('unmaximize_ohos', async (err: Error, arg: string): Promise<string> => {
      try {
        await windowStage.getMainWindowSync().recover();
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `recover failed: ${be.message}`);
      }
      return '';
    });
    RustModule.registerArktsFunction('set_min_size_ohos', async (err: Error, arg: string): Promise<string> => {
      return '';
    });
    RustModule.registerArktsFunction('set_focus_ohos', async (err: Error, arg: string): Promise<string> => {
      try {
        await windowStage.getMainWindowSync().showWindow();
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `showWindow failed: ${be.message}`);
      }
      return '';
    });
    RustModule.registerArktsFunction('set_resizable_ohos', async (err: Error, arg: string): Promise<string> => {
      return '';
    });
    RustModule.registerArktsFunction('maximize_ohos', async (err: Error, arg: string): Promise<string> => {
      try {
        await windowStage.getMainWindowSync().maximize();
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `maximize failed: ${be.message}`);
      }
      return '';
    });
    RustModule.registerArktsFunction('center_ohos', async (err: Error, arg: string): Promise<string> => {
      try {
        const mainWin = windowStage.getMainWindowSync();
        const rect = mainWin.getWindowProperties().windowRect;
        const disp = display.getDefaultDisplaySync();
        const x: number = Math.max(0, Math.floor((disp.width - rect.width) / 2));
        const y: number = Math.max(0, Math.floor((disp.height - rect.height) / 2));
        await mainWin.moveWindowTo(x, y);
      } catch (e) {
        const be = e as BusinessError;
        hilog.warn(DOMAIN, TAG, `center failed: ${be.message}`);
      }
      return '';
    });
    RustModule.registerArktsFunction('call_calendar', async (err: Error, arg: string): Promise<string> => {
      return await createMeetingEvent(arg);
    });
    RustModule.registerArktsFunction('call_harmony_build', async (err: Error, arg: string): Promise<string> => {
      hilog.info(DOMAIN_NUMBER, TAG, 'register harmony build callback ' + arg);
      runDeveco(this.context, arg);
      return '';
    });
    RustModule.registerArktsFunction('send_remote_url', async (err: Error, arg: string): Promise<string> => {
      hilog.info(DOMAIN_NUMBER, TAG, 'get remote url ' + arg);
      this.remoteUrl = arg;
      if (this.remoteUrl.length == 0) {
        this.shareDisablingListening();
      }
      else {
        this.shareListening();
      }
      return '';
    });
    RustModule.registerArktsFunction('send_remote_dialog_status', async (err: Error, arg: string): Promise<string> => {
          hilog.info(DOMAIN_NUMBER, TAG, 'get remote dialog status ' + arg);
          if (arg.length == 0) {
            this.shareDisablingListening();
          }
          else {
            this.shareListening();
          }
          return '';
    });
    RustModule.registerArktsFunction('harmony_create', async (err: Error, arg: string): Promise<string> => {
      await fileIo.copyDir('/storage/Users/currentUser/Documents/DevecoStudioProjects/MyApplication',
        '/storage/Users/currentUser/Documents/files', 1).then(() => {
        hilog.info(0x0000, 'vnext', 'copyDir success');
      }).catch((err: BusinessError) => {
        hilog.info(0x0000, 'vnext', 'copyFile error: ' + JSON.stringify(err));
      });
      await fileIo.rename('/storage/Users/currentUser/Documents/files',
        `/storage/Users/currentUser/Documents/DevecoStudioProjects/${arg.length == 0 ? 'MyApplication' : arg}`)
        .then(() => {
          hilog.info(0x0000, 'vnext', 'rename success');
        }).catch((err: BusinessError) => {
          hilog.info(0x0000, 'vnext', 'rename error: ' + JSON.stringify(err));
        });
      return '';
    });
    // Embedded browser webview bridge: each ArkTS callback parses a JSON-string
    // request from the Rust side (browser_api.rs OHOS branches) and delegates to
    // BrowserWebviewService, returning a JSON-string envelope. The service owns
    // a Map<label, BrowserWebviewEntry> and the RustWebviewNodeController
    // exposed via AppStorage by the vendored DefaultXComponent.
    const browserService = BrowserWebviewService.getInstance();
    RustModule.registerArktsFunction('browser_webview_create_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: CreateRequest = JSON.parse(arg);
      return await browserService.create(req);
    });
    RustModule.registerArktsFunction('browser_webview_eval_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: EvalRequest = JSON.parse(arg);
      return await browserService.eval(req);
    });
    RustModule.registerArktsFunction('browser_webview_navigate_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: NavigateRequest = JSON.parse(arg);
      return await browserService.navigate(req);
    });
    RustModule.registerArktsFunction('browser_webview_reload_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: LabelRequest = JSON.parse(arg);
      return await browserService.reload(req);
    });
    RustModule.registerArktsFunction('browser_webview_set_bounds_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: BoundsRequest = JSON.parse(arg);
      return await browserService.setBounds(req);
    });
    RustModule.registerArktsFunction('browser_webview_show_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: LabelRequest = JSON.parse(arg);
      return await browserService.show(req);
    });
    RustModule.registerArktsFunction('browser_webview_hide_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: LabelRequest = JSON.parse(arg);
      return await browserService.hide(req);
    });
    RustModule.registerArktsFunction('browser_webview_close_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: LabelRequest = JSON.parse(arg);
      return await browserService.close(req);
    });
    RustModule.registerArktsFunction('browser_webview_set_focus_ohos', async (err: Error, arg: string): Promise<string> => {
      const req: LabelRequest = JSON.parse(arg);
      return await browserService.setFocus(req);
    });
    setTimeout(() => {
      windowStage.getMainWindow((err, data) => {
        data.setWindowDecorHeight(44);
        data.setWindowTitleMoveEnabled(false);
        data.setWindowDecorVisible(false);
      })
    }, 40)
    RustModule.registerArktsFunction('reveal_in_explorer', async (err: Error, arg: string): Promise<string> => {
      const context = getContext(this) as common.UIAbilityContext;
      let uri: string;
      if (arg.startsWith('file://')) {
        uri = arg;
      } else if (arg.startsWith('/data/storage/')) {
        uri = fileUri.getUriFromPath(arg);
      } else {
        uri = 'file://docs' + arg;
      }
      const want: Want = {
        bundleName: 'com.huawei.hmos.filemanager',
        abilityName: 'MainAbility',
        parameters: {
          fileUri: uri
        }
      };
      try {
        await context.startAbility(want)
      } catch (err) {
        hilog.error(201, 'vnext', `reveal in explorer failed ${JSON.stringify(err)}`)
      }
      return '';
    });
    RustModule.registerArktsFunction('set_theme_mode', async (err: Error, arg: string): Promise<string> => {
      // arg is the color mode the webview should adopt: 'light' | 'dark' | 'system'.
      // - 'light'/'dark' pin the native override so the webview matches a fixed theme.
      // - 'system' releases the override (COLOR_MODE_NOT_SET) and returns the real
      //   system color mode ('light' | 'dark') so the web-ui can resolve a concrete
      //   theme without relying on `prefers-color-scheme`, which the OHOS webview does
      //   not update live (the web-ui also polls this return value to follow system).
      // Fixed themes return '' — the web-ui already knows the resolved theme id.
      const appContext = this.context.getApplicationContext();
      if (arg === 'light') {
        appContext.setColorMode(ConfigurationConstant.ColorMode.COLOR_MODE_LIGHT);
        return '';
      }
      if (arg === 'dark') {
        appContext.setColorMode(ConfigurationConstant.ColorMode.COLOR_MODE_DARK);
        return '';
      }
      appContext.setColorMode(ConfigurationConstant.ColorMode.COLOR_MODE_NOT_SET);
      return this.readSystemColorMode();
    });
    this.voiceInputService.setContext(this.context);
    RustModule.registerArktsFunction('ohos_speech_start', async (err: Error, arg: string): Promise<string> => {
      try {
        return await this.voiceInputService.start(arg);
      } catch (e) {
        const detail: string = describeError(e);
        hilog.error(0x0000, 'BitFunVoiceInput', 'ohos_speech_start callback error: %{public}s', detail);
        return JSON.stringify({ '__error': detail });
      }
    });
    RustModule.registerArktsFunction('ohos_speech_append', async (err: Error, arg: string): Promise<string> => {
      return await this.voiceInputService.append(arg);
    });
    RustModule.registerArktsFunction('ohos_speech_finish', async (err: Error, arg: string): Promise<string> => {
      return await this.voiceInputService.finish(arg);
    });
    RustModule.registerArktsFunction('ohos_speech_cancel', async (err: Error, _arg: string): Promise<string> => {
      return await this.voiceInputService.cancel('');
    });
    return super.onWindowStageCreate(windowStage);
  }

  /**
   * Resolve the current system color mode as a plain 'light' | 'dark' string.
   * Reads the Ability config's colorMode; falls back to dark (the app's default
   * appearance) when the config is unavailable or unset. Called right after
   * releasing the native color override so the web-ui can resolve a concrete
   * theme without depending on `prefers-color-scheme` (which the OHOS webview
   * does not emit).
   */
  private readSystemColorMode(): string {
    try {
      const mode = this.colorModeToString(this.context?.config?.colorMode);
      if (mode.length > 0) {
        return mode;
      }
    } catch (e) {
      hilog.warn(DOMAIN, 'vnext', 'readSystemColorMode failed: ' + JSON.stringify(e));
    }
    return 'dark';
  }

  private shareListening() {
    hilog.info(0x0000, 'vnext', 'shareListening');
    if (this.remoteUrl.length != 0 && !this.shareStatus) {
      harmonyShare.on('knockShare', this.sendOnlyCallback);
      this.shareStatus = true;
    }
  }
  private shareDisablingListening() {
    harmonyShare.off('knockShare', this.sendOnlyCallback);
    this.shareStatus = false;
  }
  private sendOnlyCallback = (sharableTable: harmonyShare.SharableTarget) => {
    if (this.remoteUrl.length != 0) {
      let content = this.remoteUrl;
      let shareData: systemShare.SharedData = new systemShare.SharedData({
        utd: uniformTypeDescriptor.UniformDataType.HYPERLINK,
        content,
        title: "BitFun",
        description: "Phone",
      });
      sharableTable.share(shareData)
    } else {
      hilog.error(DOMAIN, 'vnext', 'sendOnlyCallback error: remote url is empty');
    }
  }
}

interface AppGlobalType {
  calendarMgr: calendarManager.CalendarManager | null,
  mContext: common.UIAbilityContext | null
}

export const AppGlobal: AppGlobalType = {
  calendarMgr: null,
  mContext: null
}

function describeError(e: Object): string {
  if (e instanceof Error) {
    return `Error: ${e.message}`;
  }
  try {
    return `json: ${JSON.stringify(e)}`;
  } catch (_jsonErr) {
    return `str: ${e}`;
  }
}

export async function createMeetingEvent(input: string): Promise<string> {
  interface CalendarInfo {
    title: string,
    description: string,
    startTime: string,
    endTime: string
  }

  const info: CalendarInfo = JSON.parse(input)
  const calendarAccount: calendarManager.CalendarAccount = {
    name: info.title,
    type: calendarManager.CalendarType.LOCAL,
    displayName: 'bitfun',
  };
  const startTime = new Date(info.startTime).valueOf();

  const time = info.endTime == '' ? startTime : info.endTime;
  const endTime = new Date(time).valueOf();

  const event: calendarManager.Event = {
    type: calendarManager.EventType.NORMAL,
    title: info.title,
    startTime: startTime,
    endTime: endTime,
    isAllDay: false,
    description: info.description
  };

  try {
    let data: calendarManager.Calendar | undefined = await AppGlobal.calendarMgr?.createCalendar(calendarAccount);
    if (!data || data === null) {
      hilog.warn(0x0000, 'vnext', 'Failed to create calendar, data is null');
      return ""
    }
    let id = await data.addEvent(event);
    return "succeed in creating calendar and event " + id;
  } catch (error) {
    return "Failed to create calendar or event. Code: " + error;
  }
}
