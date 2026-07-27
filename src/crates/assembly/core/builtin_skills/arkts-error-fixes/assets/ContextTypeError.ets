import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

@Entry
@Component
struct ContextTypeErrorExample {
  @State windowWidth: number = 0;
  @State windowHeight: number = 0;

  async aboutToAppear() {
    // ✅ 正确：使用 getUIContext().getHostContext() 并进行类型检查
    const context = this.getUIContext().getHostContext();
    if (context) {
      await this.getWindowSize(context as common.UIAbilityContext);
    }
  }

  private async getWindowSize(context: common.UIAbilityContext) {
    try {
      const win = await window.getLastWindow(context);
      const properties = win.getWindowProperties();
      this.windowWidth = properties.windowRect.width;
      this.windowHeight = properties.windowRect.height;
    } catch (err) {
      console.error('获取窗口大小失败:', err);
    }
  }

  build() {
    Column() {
      Text('Context 类型错误示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 20 })

      Column({ space: 12 }) {
        Text('❌ 错误示例 1: 直接使用可能为 undefined 的 Context')
          .fontSize(14)
          .fontColor('#666666')
        
        Text('const context = this.getUIContext().getHostContext();')
          .fontSize(12)
          .fontColor('#FF5722')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#FFF3E0')
          .borderRadius(4)

        Text('await window.getLastWindow(context);  // 类型错误')
          .fontSize(12)
          .fontColor('#FF5722')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#FFF3E0')
          .borderRadius(4)

        Text('✅ 正确示例 1: 添加 null 检查')
          .fontSize(14)
          .fontColor('#666666')
        
        Text('const context = this.getUIContext().getHostContext();')
          .fontSize(12)
          .fontColor('#4CAF50')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#E8F5E9')
          .borderRadius(4)

        Text('if (context) {')
          .fontSize(12)
          .fontColor('#4CAF50')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#E8F5E9')
          .borderRadius(4)

        Text('  await window.getLastWindow(context as common.UIAbilityContext);')
          .fontSize(12)
          .fontColor('#4CAF50')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#E8F5E9')
          .borderRadius(4)

        Text('}')
          .fontSize(12)
          .fontColor('#4CAF50')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#E8F5E9')
          .borderRadius(4)
      }
      .alignItems(HorizontalAlign.Start)
      .margin({ bottom: 20 })

      Text(`窗口大小: ${this.windowWidth} x ${this.windowHeight}`)
        .fontSize(16)
        .fontColor('#333333')
    }
    .width('100%')
    .height('100%')
    .padding(20)
    .alignItems(HorizontalAlign.Center)
  }
}
