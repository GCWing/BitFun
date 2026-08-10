import { AppRootPresentation } from './components/AppRootPresentation';
import { ArkUiAppRootHostAdapter } from './host/AppRootHostAdapter';
import { AppRootRuntime } from './state/AppRootRuntime';

@Entry
@Component
struct AppRoot {
  private readonly hostAdapter: ArkUiAppRootHostAdapter = new ArkUiAppRootHostAdapter();
  private readonly runtime: AppRootRuntime = new AppRootRuntime(this.hostAdapter);

  async aboutToAppear(): Promise<void> {
    this.hostAdapter.attach(getContext(this), this.getUIContext());
    await this.runtime.aboutToAppear();
  }

  onPageShow(): void {
    this.runtime.onPageShow();
  }

  onPageHide(): void {
    this.runtime.onPageHide();
  }

  aboutToDisappear(): void {
    this.runtime.aboutToDisappear();
  }

  onBackPress(): boolean {
    return this.runtime.handleRootBack();
  }

  build() {
    Stack() {
      AppRootPresentation({
        shellState: this.runtime.appShellState,
        navigationStack: this.runtime.navigationStack,
        remotePageState: this.runtime.remotePageState,
        remoteCreateState: this.runtime.remoteCreateState,
        generalPageState: this.runtime.generalChatPageState,
        filePreviewState: this.runtime.filePreviewState,
        deviceId: this.runtime.remoteConnectionViewModel.getDeviceId(),
        actions: this.runtime.presentationActions
      })
    }
    .width('100%')
    .height('100%')
  }
}
