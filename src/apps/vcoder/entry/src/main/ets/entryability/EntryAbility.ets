import {
  abilityAccessCtrl, AbilityConstant, common, Permissions, Want
} from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import { RustAbility } from '@ohos-rs/ability';
import { CommonEventListener } from '../utils/CommonEventListener';
import { harmonyShare, systemShare } from '@kit.ShareKit';
import { fileIo } from '@kit.CoreFileKit';
import { uniformTypeDescriptor } from '@kit.ArkData';
import { calendarManager } from '@kit.CalendarKit';
import { BusinessError } from '@kit.BasicServicesKit';
import RustModule from 'libbitfun_desktop_lib.so';
import { CommonUtils } from '../utils/CommonUtils';
import { runDeveco } from '../utils/DevecoStart';


const DOMAIN = 0x0000;

export default class EntryAbility extends RustAbility {
  public moduleName: string = "bitfun_desktop_lib";
  public defaultPage: boolean = true;
  public commonEventListener: CommonEventListener | undefined = undefined;

  async onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): Promise<void> {
    super.onCreate(want, launchParam);
    this.commonEventListener = new CommonEventListener();
    this.aboutToAppear();
  }

  onDestroy(): void {
    hilog.info(DOMAIN, 'testTag', '%{public}s', 'Ability onDestroy');
  }

  onWindowStageCreate(windowStage: window.WindowStage): Promise<void> {
    AppGlobal.calendarMgr = calendarManager.getCalendarManager(this.context);
    AppGlobal.mContext = this.context;
    const permissions: Permissions[] = ['ohos.permission.READ_CALENDAR', 'ohos.permission.WRITE_CALENDAR'];
    const DOMAIN_NUMBER: number = 0xFF00;
    const TAG: string = '[BitfunUIAbilityComponents]';

    let atManager = abilityAccessCtrl.createAtManager();
    AppGlobal.calendarMgr = calendarManager.getCalendarManager(this.context);

    atManager.requestPermissionsFromUser(this.context, permissions).then(() => {
      AppGlobal.calendarMgr = calendarManager.getCalendarManager(this.context);
    }).catch((error: BusinessError) => {
      hilog.info(DOMAIN_NUMBER, TAG, 'get permissions error: ' + error);
    });
    RustModule.registerArktsFunction('open_dialog_file', async (err: Error, arg: string): Promise<string> => {
      let res = await CommonUtils.open_file_dialog();
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
    RustModule.registerArktsFunction('call_calendar', async (err: Error, arg: string): Promise<string> => {
      return await createMeetingEvent(arg);
    });
    RustModule.registerArktsFunction('call_harmony_build', async (err: Error, arg: string): Promise<string> => {
      hilog.info(DOMAIN_NUMBER, TAG, 'register harmony build callback ' + arg);
      runDeveco(this.context, arg);
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
    return super.onWindowStageCreate(windowStage);
  }

  aboutToAppear(): void {
    console.info("aboutToAppear");
    harmonyShare.on('knockShare', this.sendOnlyCallback);
  }

  private sendOnlyCallback = (sharableTable: harmonyShare.SharableTarget) => {
    let filePath = "/data/storage/el2/base/files/dist/output.txt";
    fileIo.readText(filePath).then((content: string) => {
      console.info("readText success:" + content);
      let shareData: systemShare.SharedData = new systemShare.SharedData({
        utd: uniformTypeDescriptor.UniformDataType.HYPERLINK,
        content,
        title: "Bitfun",
        description: "Phone",
      });
      sharableTable.share(shareData)
    }).catch((err: BusinessError) => {
      hilog.error(DOMAIN, 'vnext', 'sendOnlyCallback error:' + err);
    });
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
    displayName: 'vcoder',
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