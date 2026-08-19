import { AppRoute } from '../navigation/AppRouteContract';
import { SettingsSheetState } from './SettingsSheetState';

/** The connect sheet decides for itself where to open. */
export const CONNECT_INTENT_AUTO: string = 'auto';
/** The user tapped something that promised a camera, so open the camera. */
export const CONNECT_INTENT_SCAN: string = 'scan';

@ObservedV2
export class AppShellState {
  /**
   * Mirror of the navigation stack top. NavPathStack is not observable, so
   * surfaces that live outside Navigation — the drawer above all — have no way
   * to follow the route without this traced copy.
   */
  @Trace activeRoute: AppRoute = AppRoute.ChatHome;
  @Trace showSidebar: boolean = false;
  @Trace showSettings: boolean = false;
  @Trace settingsMode: string = 'general';
  @Trace showConnectSheet: boolean = false;
  /**
   * Why the connect sheet was opened, which is not something the sheet can work
   * out for itself: a signed-in phone has an account device list to offer and a
   * camera to offer, and only the entry point knows which one the user just
   * asked for by name.
   */
  @Trace connectSheetIntent: string = CONNECT_INTENT_AUTO;
  /**
   * Mirror of the resolved master-detail layout mode. Only the presentation
   * layer measures the viewport, so runtime logic that must branch on compact
   * versus wide reads it from here.
   */
  @Trace wideLayout: boolean = false;
  /**
   * Bumped when the in-app locale changes so shell consumers that already
   * observe this object rebuild even if they do not call RemoteI18n.t().
   */
  @Trace localeRevision: number = 0;
  readonly settingsSheet: SettingsSheetState = new SettingsSheetState();
  private accountReturnMode: string = '';

  setSidebarVisible(visible: boolean): void {
    this.showSidebar = visible;
  }

  setWideLayout(wide: boolean): void {
    this.wideLayout = wide;
  }

  setLocaleRevision(revision: number): void {
    this.localeRevision = revision;
  }

  setSettingsVisible(visible: boolean): void {
    this.showSettings = visible;
    if (!visible) {
      this.accountReturnMode = '';
      this.settingsSheet.resetOnClose();
    }
  }

  openSettings(mode: string): void {
    if (mode === 'account') {
      this.accountReturnMode = this.showSettings ? this.settingsMode : '';
    } else {
      this.accountReturnMode = '';
      if (mode === 'general' && !this.showSettings) {
        this.settingsSheet.resetOnClose();
      }
    }
    this.settingsMode = mode;
    this.showSettings = true;
  }

  leaveSettings(): void {
    if (this.settingsMode === 'account' && this.accountReturnMode.length > 0) {
      this.settingsMode = this.accountReturnMode;
      this.accountReturnMode = '';
      this.showSettings = true;
      return;
    }
    this.setSettingsVisible(false);
  }

  /**
   * The intent is set on every open rather than cleared on close, because the
   * sheet can also be dismissed by a drag that never reaches this class — and a
   * default that only holds while nobody swipes is not a default.
   */
  setConnectSheetVisible(visible: boolean, intent: string = CONNECT_INTENT_AUTO): void {
    if (visible) {
      this.connectSheetIntent = intent;
    }
    this.showConnectSheet = visible;
  }

  closeGlobalSurfaces(): void {
    this.showSidebar = false;
    this.showSettings = false;
    this.accountReturnMode = '';
    this.settingsSheet.resetOnClose();
    this.showConnectSheet = false;
  }
}
