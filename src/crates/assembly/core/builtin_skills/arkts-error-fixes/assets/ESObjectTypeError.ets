import { hilog } from '@kit.PerformanceAnalysisKit';

const DOMAIN = 0xFF00;
const TAG = 'ESObjectTypeError';

@Entry
@Component
struct ESObjectTypeError {
  @State moduleLoaded: boolean = false;
  @State moduleName: string = '';
  @State errorMessage: string = '';

  async loadNFCModule() {
    try {
      await import('@kit.ConnectivityKit');
      this.moduleLoaded = true;
      this.moduleName = 'NFC';
      this.errorMessage = '';
      hilog.info(DOMAIN, TAG, 'NFC module imported successfully');
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to import NFC module: %{public}s', JSON.stringify(err));
      this.moduleLoaded = false;
      this.moduleName = 'NFC';
      this.errorMessage = '模块导入失败';
    }
  }

  async loadCameraModule() {
    try {
      await import('@kit.CameraKit');
      this.moduleLoaded = true;
      this.moduleName = 'Camera';
      this.errorMessage = '';
      hilog.info(DOMAIN, TAG, 'Camera module imported successfully');
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to import Camera module: %{public}s', JSON.stringify(err));
      this.moduleLoaded = false;
      this.moduleName = 'Camera';
      this.errorMessage = '模块导入失败';
    }
  }

  build() {
    Column() {
      Text('ESObject 类型限制错误示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ top: 20, bottom: 20 })

      Text('错误写法（不推荐）：')
        .fontSize(14)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 10 })

      Text(`let nfcModule: ESObject = null;`)
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

      Text(`let nfcModule: ESModule | null = null;`)
        .fontSize(12)
        .fontColor('#00AA00')
        .fontFamily('monospace')
        .padding(10)
        .backgroundColor('#F0FFF0')
        .borderRadius(4)
        .margin({ bottom: 20 })

      Row() {
        Button('加载 NFC 模块')
          .onClick(() => {
            this.loadNFCModule();
          })
          .margin({ right: 10 })

        Button('加载 Camera 模块')
          .onClick(() => {
            this.loadCameraModule();
          })
      }
      .margin({ bottom: 20 })

      if (this.moduleName) {
        Column() {
          Text(`模块名称: ${this.moduleName}`)
            .fontSize(14)
            .margin({ bottom: 5 })

          if (this.moduleLoaded) {
            Text('状态: 加载成功 ✓')
              .fontSize(14)
              .fontColor('#00AA00')
          } else {
            Text(`状态: ${this.errorMessage} ✗`)
              .fontSize(14)
              .fontColor('#FF0000')
          }
        }
        .width('100%')
        .padding(15)
        .backgroundColor('#F5F5F5')
        .borderRadius(8)
      }

      Text('说明:')
        .fontSize(14)
        .fontWeight(FontWeight.Bold)
        .margin({ top: 20, bottom: 10 })

      Text('• ESObject 类型使用受限，不能直接用作变量类型')
        .fontSize(12)
        .fontColor('#666666')
        .margin({ bottom: 5 })

      Text('• 使用 ESModule 类型或具体类型来声明动态导入的模块')
        .fontSize(12)
        .fontColor('#666666')
        .margin({ bottom: 5 })

      Text('• 使用 import() 函数进行动态导入')
        .fontSize(12)
        .fontColor('#666666')
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}
