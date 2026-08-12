import { AppRoute } from '../navigation/AppRouteContract';

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
   * Mirror of the resolved master-detail layout mode. Only the presentation
   * layer measures the viewport, so runtime logic that must branch on compact
   * versus wide reads it from here.
   */
  @Trace wideLayout: boolean = false;
  private accountReturnMode: string = '';

  setSidebarVisible(visible: boolean): void {
    this.showSidebar = visible;
  }

  setWideLayout(wide: boolean): void {
    this.wideLayout = wide;
  }

  setSettingsVisible(visible: boolean): void {
    this.showSettings = visible;
    if (!visible) {
      this.accountReturnMode = '';
    }
  }

  openSettings(mode: string): void {
    if (mode === 'account') {
      this.accountReturnMode = this.showSettings ? this.settingsMode : '';
    } else {
      this.accountReturnMode = '';
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

  setConnectSheetVisible(visible: boolean): void {
    this.showConnectSheet = visible;
  }

  closeGlobalSurfaces(): void {
    this.showSidebar = false;
    this.showSettings = false;
    this.accountReturnMode = '';
    this.showConnectSheet = false;
  }
}
