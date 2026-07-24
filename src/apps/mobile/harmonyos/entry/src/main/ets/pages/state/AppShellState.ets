@ObservedV2
export class AppShellState {
  @Trace showSidebar: boolean = false;
  @Trace showSettings: boolean = false;
  @Trace settingsMode: string = 'general';
  @Trace showConnectSheet: boolean = false;

  setSidebarVisible(visible: boolean): void {
    this.showSidebar = visible;
  }

  setSettingsVisible(visible: boolean): void {
    this.showSettings = visible;
  }

  openSettings(mode: string): void {
    this.settingsMode = mode;
    this.showSettings = true;
  }

  setConnectSheetVisible(visible: boolean): void {
    this.showConnectSheet = visible;
  }

  closeGlobalSurfaces(): void {
    this.showSidebar = false;
    this.showSettings = false;
    this.showConnectSheet = false;
  }
}
