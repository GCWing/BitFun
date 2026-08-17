import { AppShellState } from '../state/AppShellState';
import { ConversationLayoutPolicy } from '../policy/ConversationLayoutPolicy';
import { PAGE_BG } from './Theme';

const WIDE_SETTINGS_SHEET_MAX_WIDTH: number = 680;
const WIDE_CONNECT_SHEET_MAX_WIDTH: number = 620;
const WIDE_SHEET_MIN_WIDTH: number = 540;
const WIDE_SHEET_MAX_HEIGHT: number = 760;
const WIDE_SHEET_MIN_HEIGHT: number = 560;
const WIDE_SHEET_VERTICAL_MARGIN: number = 80;
const WIDE_SHEET_RADIUS: number = 30;

@ComponentV2
export struct AppShell {
  @Param shellState: AppShellState = new AppShellState();
  @Param useWideLayout: boolean = false;
  @BuilderParam content: () => void = this.EmptyBuilder;
  @BuilderParam sidebar: () => void = this.EmptyBuilder;
  @BuilderParam settings: () => void = this.EmptyBuilder;
  @BuilderParam connect: () => void = this.EmptyBuilder;
  @Event onCloseSidebar: () => void = () => {};
  @Local shellWidth: number = 440;
  @Local shellHeight: number = 0;

  build() {
    Stack({ alignContent: Alignment.Start }) {
      Column() {
        // The drawer is a compact affordance. Wide layouts put the same session
        // list in the master pane instead, and this copy stayed mounted behind
        // an opacity of 0 — invisible, but rebuilt on every selection change,
        // which on a tablet doubled the cost of switching sessions. Compact
        // keeps it mounted so opening and closing still cross-fades.
        if (!this.useWideLayout || this.shellState.showSidebar) {
          this.sidebar()
        }
      }
      .width(this.sidebarWidth())
      .height('100%')
      .alignItems(HorizontalAlign.Start)
      .opacity(this.shellState.showSidebar ? 1 : 0)
      .translate({ x: this.shellState.showSidebar ? '0%' : '-10%' })
      .animation({
        duration: this.shellState.showSidebar ? 300 : 220,
        curve: Curve.EaseOut
      })

      Stack({ alignContent: Alignment.Start }) {
        Column() {
          this.content()
        }
        .width('100%')
        .height('100%')
        .bindSheet($$this.shellState.showConnectSheet, this.ConnectSheetContent(), this.connectSheetOptions())

        if (this.shellState.showSidebar) {
          Column() {
          }
          .width('100%')
          .height('100%')
          .backgroundColor(PAGE_BG)
          .opacity(0.62)
          .transition(TransitionEffect.opacity(0)
            .animation({ duration: 210, curve: Curve.EaseOut }))
          .onClick(() => {
            this.onCloseSidebar();
          })
        }
      }
      .width('100%')
      .height('100%')
      .backgroundColor(PAGE_BG)
      .borderRadius(this.shellState.showSidebar ? 28 : 0)
      .clip(true)
      .shadow({
        radius: this.shellState.showSidebar ? 34 : 0,
        color: this.shellState.showSidebar ? '#24000000' : '#00000000',
        offsetX: this.shellState.showSidebar ? -10 : 0
      })
      .blur(this.shellState.showSidebar ? 1.1 : 0)
      .scale({
        x: this.shellState.showSidebar ? 0.985 : 1,
        y: this.shellState.showSidebar ? 0.992 : 1,
        centerX: '0%',
        centerY: '50%'
      })
      .translate({ x: this.shellState.showSidebar ? this.sidebarWidth() : 0 })
      .animation({
        duration: this.shellState.showSidebar ? 320 : 250,
        curve: Curve.EaseOut
      })
    }
    .width('100%')
    .height('100%')
    .onAreaChange((_oldArea: Area, newArea: Area) => {
      this.shellWidth = this.areaLength(newArea.width);
      this.shellHeight = this.areaLength(newArea.height);
    })
    .bindSheet($$this.shellState.showSettings, this.SettingsSheetContent(), this.settingsSheetOptions())
  }

  @Builder
  private SettingsSheetContent() {
    Column() {
      this.settings()
    }
    .width('100%')
    .height('100%')
    .borderRadius(this.shouldUseWideSheetLayout() ? WIDE_SHEET_RADIUS : 0)
    .clip(this.shouldUseWideSheetLayout())
    .shadow({
      radius: this.shouldUseWideSheetLayout() ? 30 : 0,
      color: this.shouldUseWideSheetLayout() ? '#22000000' : '#00000000',
      offsetY: this.shouldUseWideSheetLayout() ? 12 : 0
    })
  }

  @Builder
  private ConnectSheetContent() {
    Column() {
      this.connect()
    }
    .width('100%')
    .height('100%')
    .borderRadius(this.shouldUseWideSheetLayout() ? WIDE_SHEET_RADIUS : 0)
    .clip(this.shouldUseWideSheetLayout())
    .shadow({
      radius: this.shouldUseWideSheetLayout() ? 30 : 0,
      color: this.shouldUseWideSheetLayout() ? '#22000000' : '#00000000',
      offsetY: this.shouldUseWideSheetLayout() ? 12 : 0
    })
  }

  private sidebarWidth(): number {
    if (this.shellWidth <= 0) {
      return 280;
    }
    if (this.shellWidth < ConversationLayoutPolicy.MD_MIN_WIDTH) {
      return Math.min(280, Math.max(220, Math.round(this.shellWidth * 0.68)));
    }
    return Math.min(420, Math.max(280, Math.round(this.shellWidth * 0.35)));
  }

  private settingsSheetOptions(): SheetOptions {
    if (!this.shouldUseWideSheetLayout()) {
      return this.bottomSheetOptions();
    }
    return this.wideCenterSheetOptions(this.settingsSheetWidth());
  }

  private connectSheetOptions(): SheetOptions {
    if (!this.shouldUseWideSheetLayout()) {
      return this.bottomSheetOptions();
    }
    return this.wideCenterSheetOptions(this.connectSheetWidth());
  }

  private bottomSheetOptions(): SheetOptions {
    return {
      height: SheetSize.LARGE,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    };
  }

  private wideCenterSheetOptions(width: number): SheetOptions {
    return {
      height: this.wideSheetHeight(),
      width,
      preferType: SheetType.CENTER,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    };
  }

  private settingsSheetWidth(): number {
    return Math.min(
      WIDE_SETTINGS_SHEET_MAX_WIDTH,
      Math.max(WIDE_SHEET_MIN_WIDTH, Math.round(this.shellWidth * 0.48))
    );
  }

  private connectSheetWidth(): number {
    return Math.min(
      WIDE_CONNECT_SHEET_MAX_WIDTH,
      Math.max(WIDE_SHEET_MIN_WIDTH, Math.round(this.shellWidth * 0.46))
    );
  }

  private wideSheetHeight(): number {
    if (this.shellHeight <= 0) {
      return WIDE_SHEET_MAX_HEIGHT;
    }
    return Math.min(
      WIDE_SHEET_MAX_HEIGHT,
      Math.max(WIDE_SHEET_MIN_HEIGHT, Math.round(this.shellHeight - WIDE_SHEET_VERTICAL_MARGIN))
    );
  }

  private shouldUseWideSheetLayout(): boolean {
    return this.useWideLayout;
  }

  private areaLength(value: Object): number {
    const parsed = Number.parseFloat(`${value}`);
    return Number.isNaN(parsed) ? 0 : parsed;
  }

  @Builder
  EmptyBuilder() {
    Column() {
    }
  }
}
