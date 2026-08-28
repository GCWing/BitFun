import { Context } from '@kit.AbilityKit';
import { inputMethod } from '@kit.IMEKit';
import { window } from '@kit.ArkUI';

/** Platform adapter for dismissing the IME without leaving callers waiting forever. */
export class KeyboardReleaseService {
  static async release(context: Context, timeoutMs: number = 500): Promise<void> {
    try {
      const appWindow = await window.getLastWindow(context);
      const keyboardArea = appWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_KEYBOARD);
      if (!keyboardArea.visible) {
        return;
      }

      await new Promise<void>((resolve) => {
        let completed = false;
        let timeoutId: number = -1;
        let onKeyboardDidHide: (keyboardInfo: window.KeyboardInfo) => void =
          (_keyboardInfo: window.KeyboardInfo): void => {};
        const finish = (): void => {
          if (completed) {
            return;
          }
          completed = true;
          appWindow.off('keyboardDidHide', onKeyboardDidHide);
          if (timeoutId >= 0) {
            clearTimeout(timeoutId);
          }
          resolve();
        };
        onKeyboardDidHide = (_keyboardInfo: window.KeyboardInfo): void => {
          finish();
        };

        appWindow.on('keyboardDidHide', onKeyboardDidHide);
        timeoutId = setTimeout(finish, timeoutMs);
        inputMethod.getController().hideTextInput().catch((_err: Object) => {
          finish();
        });
      });
    } catch (_err) {
      // Releasing the keyboard is best effort and must never block message submission.
    }
  }
}
