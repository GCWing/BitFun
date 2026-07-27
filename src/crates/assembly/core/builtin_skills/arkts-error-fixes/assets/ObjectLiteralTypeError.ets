import { common } from '@kit.AbilityKit';

interface WindowSize {
  width: number;
  height: number;
}

@Entry
@Component
struct ObjectLiteralTypeError {
  @State windowSize: WindowSize = { width: 0, height: 0 };

  aboutToAppear() {
    this.windowSize = this.getWindowSize();
  }

  private getWindowSize(): WindowSize {
    const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
    const windowStage = context.windowStage;
    if (!windowStage) {
      return { width: 0, height: 0 };
    }
    const windowClass = windowStage.getMainWindowSync();
    const windowProperties = windowClass.getWindowProperties();
    return {
      width: windowProperties.windowRect.width,
      height: windowProperties.windowRect.height
    };
  }

  build() {
    Column() {
      Text('Object Literal Type Error Example')
        .fontSize(20)
        .margin(20)

      Text(`窗口宽度: ${this.windowSize.width}`)
        .fontSize(16)
        .margin(10)

      Text(`窗口高度: ${this.windowSize.height}`)
        .fontSize(16)
        .margin(10)

      Button('刷新窗口尺寸')
        .onClick(() => {
          this.windowSize = this.getWindowSize();
        })
        .margin(20)
    }
    .width('100%')
    .height('100%')
  }
}