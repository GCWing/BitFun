import { common } from '@kit.AbilityKit';

export interface AppRootHostPort {
  attach(context: Context, uiContext: UIContext): void;
  context(): Context;
  animate(duration: number, callback: () => void): void;
  showToast(message: string, duration: number): boolean;
  openExternalLink?(link: string): Promise<boolean>;
}

export class ArkUiAppRootHostAdapter implements AppRootHostPort {
  private hostContext: Context = {} as Context;
  private uiContext: UIContext = {} as UIContext;

  attach(context: Context, uiContext: UIContext): void {
    this.hostContext = context;
    this.uiContext = uiContext;
  }

  context(): Context {
    return this.hostContext;
  }

  animate(duration: number, callback: () => void): void {
    try {
      this.uiContext.animateTo({ duration, curve: Curve.EaseOut }, callback);
    } catch (_err) {
      callback();
    }
  }

  showToast(message: string, duration: number): boolean {
    try {
      this.uiContext.getPromptAction().showToast({ message, duration });
      return true;
    } catch (_err) {
      return false;
    }
  }

  async openExternalLink(link: string): Promise<boolean> {
    const value = link.trim();
    if (!/^https?:\/\//i.test(value)) {
      return false;
    }
    try {
      await (this.hostContext as common.UIAbilityContext).openLink(value);
      return true;
    } catch (_err) {
      return false;
    }
  }
}
