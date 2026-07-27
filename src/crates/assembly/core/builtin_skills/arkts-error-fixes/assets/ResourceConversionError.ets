import { hilog } from '@kit.PerformanceAnalysisKit';

const DOMAIN = 0xFF00;
const TAG = 'ResourceConversionError';

@Entry
@Component
struct ResourceConversionError {
  @State displayText: string = '';
  @State fontSize: number = 16;
  @State paddingValue: number = 10;

  loadStringResource() {
    try {
      const context = this.getUIContext().getHostContext();
      if (!context) {
        hilog.error(DOMAIN, TAG, 'Context is null or undefined');
        this.displayText = '加载失败';
        return;
      }
      const manager = context.resourceManager;
      const resourceId = $r('app.string.hello').id;
      this.displayText = manager.getString(resourceId);
      hilog.info(DOMAIN, TAG, 'String resource loaded: %{public}s', this.displayText);
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to load string resource: %{public}s', 
        JSON.stringify(err));
      this.displayText = '加载失败';
    }
  }

  loadNumberResource() {
    try {
      const context = this.getUIContext().getHostContext();
      if (!context) {
        hilog.error(DOMAIN, TAG, 'Context is null or undefined');
        this.fontSize = 16;
        return;
      }
      const manager = context.resourceManager;
      const resourceId = $r('app.float.title_font_size').id;
      this.fontSize = manager.getNumber(resourceId);
      hilog.info(DOMAIN, TAG, 'Number resource loaded: %{public}d', this.fontSize);
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to load number resource: %{public}s', 
        JSON.stringify(err));
      this.fontSize = 16;
    }
  }

  build() {
    Column() {
      Text('Resource 类型转换错误示例')
        .fontSize($r('app.float.title_font_size'))
        .fontWeight(FontWeight.Bold)
        .margin({ top: 20, bottom: 20 })

      Text('错误写法（不推荐）：')
        .fontSize(14)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 10 })

      Text(`const message: string = $r('app.string.hello');`)
        .fontSize(12)
        .fontColor('#FF0000')
        .fontFamily('monospace')
        .padding(10)
        .backgroundColor('#FFF0F0')
        .borderRadius(4)
        .margin({ bottom: 20 })

      Text('正确写法（推荐）：')
        .fontSize(14)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 10 })

      Text('直接在 UI 组件中使用 Resource 类型')
        .fontSize(12)
        .fontColor('#666666')
        .margin({ bottom: 10 })

      Text($r('app.string.hello'))
        .fontSize($r('app.float.content_font_size'))
        .width($r('app.float.layout_width'))
        .padding($r('app.float.padding'))
        .backgroundColor($r('app.color.background_color'))
        .borderRadius($r('app.float.border_radius'))
        .margin({ bottom: 20 })

      Text('方案1：直接在 UI 组件中使用（推荐）')
        .fontSize(14)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 10 })

      Text('Resource 类型可以直接作为属性值传递给 UI 组件')
        .fontSize(12)
        .fontColor('#666666')
        .margin({ bottom: 10 })

      Text('方案2：使用 ResourceManager 获取实际值')
        .fontSize(14)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 10 })

      Row() {
        Button('加载字符串资源')
          .onClick(() => {
            this.loadStringResource();
          })
          .margin({ right: 10 })

        Button('加载数值资源')
          .onClick(() => {
            this.loadNumberResource();
          })
      }
      .margin({ bottom: 20 })

      if (this.displayText) {
        Text(`加载的字符串: ${this.displayText}`)
          .fontSize(14)
          .margin(20)
      }

      if (this.fontSize !== 16) {
        Text(`加载的字体大小: ${this.fontSize}`)
          .fontSize(14)
          .margin(20)
      }

      Text('说明:')
        .fontSize(14)
        .fontWeight(FontWeight.Bold)
        .margin({ top: 20, bottom: 10 })

      Text('• Resource 类型不能直接转换为 string 或 number')
        .fontSize(12)
        .fontColor('#666666')
        .margin({ bottom: 5 })

      Text('• 直接在 UI 组件中使用 Resource 类型')
        .fontSize(12)
        .fontColor('#666666')
        .margin({ bottom: 5 })

      Text('• 使用 ResourceManager 获取实际值')
        .fontSize(12)
        .fontColor('#666666')
        .margin({ bottom: 5 })

      Text('• Resource 支持多语言和主题适配')
        .fontSize(12)
        .fontColor('#666666')
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}
