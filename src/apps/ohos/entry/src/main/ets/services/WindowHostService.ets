import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';

const MAIN_WINDOW_LABEL: string = 'main';
const FLOATING_TIMEOUT_MS: number = 2000;

export interface WindowRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WindowHostRequest {
  action: string;
  label: string;
  rect?: WindowRect;
}

interface WindowHostResponse {
  ok: boolean;
  result?: WindowRect;
  handles?: string[];
  error?: string;
}

/**
 * Application-window provider for the embedded WebDriver server.
 *
 * This service deliberately controls only the HarmonyOS main WindowStage
 * window. Embedded ArkUI Web nodes remain page/WebView resources and are not
 * exposed as top-level WebDriver window handles.
 */
export class WindowHostService {
  private readonly windowStage: window.WindowStage;
  private readonly terminateAbility: () => void;

  constructor(windowStage: window.WindowStage, terminateAbility: () => void) {
    this.windowStage = windowStage;
    this.terminateAbility = terminateAbility;
  }

  async handle(request: WindowHostRequest): Promise<string> {
    try {
      this.validateLabel(request.label);
      const mainWindow: window.Window = this.windowStage.getMainWindowSync();
      let response: WindowHostResponse;

      switch (request.action) {
        case 'getRect':
          response = { ok: true, result: this.readRect(mainWindow) };
          break;
        case 'setRect':
          if (request.rect === undefined || request.rect.width <= 0 || request.rect.height <= 0) {
            throw new Error('setRect requires a positive width and height');
          }
          await this.ensureFloating(mainWindow);
          await mainWindow.resizeAsync(request.rect.width, request.rect.height);
          await mainWindow.moveWindowToGlobalDisplay(request.rect.x, request.rect.y);
          response = { ok: true, result: this.readRect(mainWindow) };
          break;
        case 'maximize':
          await mainWindow.maximize(window.MaximizePresentation.EXIT_IMMERSIVE);
          response = { ok: true, result: this.readRect(mainWindow) };
          break;
        case 'minimize':
          await mainWindow.minimize();
          response = { ok: true };
          break;
        case 'fullscreen':
          await mainWindow.maximize(window.MaximizePresentation.ENTER_IMMERSIVE_DISABLE_TITLE_AND_DOCK_HOVER);
          response = { ok: true, result: this.readRect(mainWindow) };
          break;
        case 'close':
          // Let the WebDriver HTTP response leave the process before Ability
          // termination tears down the embedded server.
          setTimeout(() => this.terminateAbility(), 100);
          response = { ok: true, handles: [] };
          break;
        default:
          throw new Error('Unsupported window action: ' + request.action);
      }

      return JSON.stringify(response);
    } catch (error) {
      return JSON.stringify({ ok: false, error: this.describeError(error) } as WindowHostResponse);
    }
  }

  private validateLabel(label: string): void {
    if (label !== MAIN_WINDOW_LABEL) {
      throw new Error('Unknown HarmonyOS window handle: ' + label);
    }
  }

  private readRect(mainWindow: window.Window): WindowRect {
    const properties: window.WindowProperties = mainWindow.getWindowProperties();
    const rect: window.Rect = properties.globalDisplayRect ?? mainWindow.getGlobalRect();
    return {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
    };
  }

  private async ensureFloating(mainWindow: window.Window): Promise<void> {
    if (mainWindow.getWindowStatus() === window.WindowStatusType.MINIMIZE) {
      await mainWindow.restore();
    }
    if (mainWindow.getWindowStatus() === window.WindowStatusType.FLOATING) {
      return;
    }

    await mainWindow.recover();
    if (mainWindow.getWindowStatus() !== window.WindowStatusType.FLOATING) {
      await this.waitForStatus(mainWindow, window.WindowStatusType.FLOATING);
    }
  }

  private waitForStatus(
    mainWindow: window.Window,
    expected: window.WindowStatusType,
  ): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      let completed: boolean = false;
      const finish = (error?: Error): void => {
        if (completed) {
          return;
        }
        completed = true;
        mainWindow.off('windowStatusDidChange', listener);
        if (error === undefined) {
          resolve();
        } else {
          reject(error);
        }
      };
      const listener = (status: window.WindowStatusType): void => {
        if (status === expected) {
          finish();
        }
      };
      mainWindow.on('windowStatusDidChange', listener);
      if (mainWindow.getWindowStatus() === expected) {
        finish();
        return;
      }
      setTimeout(() => {
        if (mainWindow.getWindowStatus() === expected) {
          finish();
        } else {
          finish(new Error('Timed out waiting for HarmonyOS window status change'));
        }
      }, FLOATING_TIMEOUT_MS);
    });
  }

  private describeError(error: Object): string {
    const businessError: BusinessError = error as BusinessError;
    const code: number = businessError.code;
    const message: string = businessError.message ?? String(error);
    return code === undefined ? message : 'HarmonyOS error ' + code + ': ' + message;
  }
}
