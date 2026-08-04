import { AbilityConstant, Configuration, UIAbility, Want } from "@kit.AbilityKit";
import window from "@ohos.window";
import webview from "@ohos.web.webview";
import * as Entry from "../components/MainPage";
import { ApplicationLifecycle, Module } from "./type";
import { Loadable } from "../helper/loadable";

export const STATE_KEY = "ohos.rs.ability.application.state";

export class RustAbility extends UIAbility {
  /**
   * load dynamic library
   */
  public moduleName: string = "";
  /**
   * Jump to defaultPage by default
   * @default true
   */
  public defaultPage: boolean = true;
  /**
   * Current page mode,support xcomponent and webview
   * @default xcomponent
   * @deprecated don't use it. Since 0.3, we can render xcomponent and webview in mixed mode.
   */
  public mode: "xcomponent" | "webview" = "xcomponent";
  /**
   * Load dynamic library mode.
   *
   */
  public loadMode: "async" | "sync" = "async";
  private nativeModule: Module | null = null;
  private lifecycle: ApplicationLifecycle | null = null;

  async onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): Promise<void> {
    const isRestore: boolean =
      (want.parameters?.["ohos.ability.params.abilityRecoveryRestart"] as boolean) ?? false;
    const state = isRestore ? ((want.parameters?.[STATE_KEY] as string) ?? "") : "";

    AppStorage.setOrCreate("moduleName", this.moduleName);
    AppStorage.setOrCreate("loadMode", this.loadMode);

    const packageName = `lib${this.moduleName}.so`;
    this.nativeModule = await Loadable.load(packageName, this.loadMode);
    // Register custom protocol as first
    // You can define it by yourself or use ability to define.
    if (typeof this.nativeModule?.registerCustomProtocol === "function") {
      this.nativeModule!.registerCustomProtocol();
    }

    // Must call it when custom protocol is enabled.
    webview.WebviewController.initializeWebEngine();

    this.lifecycle = this.nativeModule!.init();
    this.lifecycle?.windowStageEventCallback.onAbilityCreate(state);
  }

  async onWindowStageCreate(windowStage: window.WindowStage): Promise<void> {
    this.lifecycle?.windowStageEventCallback.onWindowStageCreate();

    windowStage.on("windowStageEvent", (event: window.WindowStageEventType) => {
      this.lifecycle?.windowStageEventCallback.onWindowStageEvent(event);
    });

    if (this.defaultPage) {
      await windowStage.loadContentByName(Entry.RouteName);
    }
  }

  onMemoryLevel(level: AbilityConstant.MemoryLevel): void {
    this.lifecycle?.environmentCallback.onMemoryLevel(level);
  }

  onDestroy(): void | Promise<void> {
    this.lifecycle?.windowStageEventCallback.onAbilityDestroy();
  }

  onConfigurationUpdate(newConfig: Configuration): void {
    this.lifecycle?.environmentCallback.onConfigurationUpdated(newConfig);
  }

  onSaveState(
    reason: AbilityConstant.StateType,
    wantParam: Record<string, Object>,
  ): AbilityConstant.OnSaveResult {
    const ret = this.lifecycle?.windowStageEventCallback.onAbilitySaveState();
    wantParam[STATE_KEY] = ret as string;
    return AbilityConstant.OnSaveResult.RECOVERY_AGREE;
  }
}
