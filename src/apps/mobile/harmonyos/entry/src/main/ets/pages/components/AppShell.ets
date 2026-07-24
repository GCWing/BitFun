import { AppShellState } from '../state/AppShellState';

@ComponentV2
export struct AppShell {
  @Param shellState: AppShellState = new AppShellState();
  @BuilderParam content: () => void = this.EmptyBuilder;
  @BuilderParam sidebar: () => void = this.EmptyBuilder;
  @BuilderParam settings: () => void = this.EmptyBuilder;
  @BuilderParam connect: () => void = this.EmptyBuilder;
  @Event onCloseSidebar: () => void = () => {};
  @Local shellWidth: number = 440;

  build() {
    Stack({ alignContent: Alignment.Start }) {
      Column() {
        this.sidebar()
      }
      .width('76%')
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
        .bindSheet($$this.shellState.showConnectSheet, this.connect(), {
          height: SheetSize.LARGE,
          backgroundColor: '#00000000',
          maskColor: '#44000000',
          showClose: false,
          dragBar: false
        })

        if (this.shellState.showSidebar) {
          Column() {
          }
          .width('100%')
          .height('100%')
          .backgroundColor('#66FFFFFF')
          .transition(TransitionEffect.opacity(0)
            .animation({ duration: 210, curve: Curve.EaseOut }))
          .onClick(() => {
            this.onCloseSidebar();
          })
        }
      }
      .width('100%')
      .height('100%')
      .backgroundColor('#FFFFFF')
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
      .translate({ x: this.shellState.showSidebar ? this.shellWidth * 0.765 : 0 })
      .animation({
        duration: this.shellState.showSidebar ? 320 : 250,
        curve: Curve.EaseOut
      })
    }
    .width('100%')
    .height('100%')
    .onAreaChange((_oldArea: Area, newArea: Area) => {
      this.shellWidth = Number(newArea.width);
    })
    .bindSheet($$this.shellState.showSettings, this.settings(), {
      height: SheetSize.LARGE,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    })
  }

  @Builder
  EmptyBuilder() {
    Column() {
    }
  }
}
