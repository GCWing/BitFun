import { mediaquery } from '@kit.ArkUI';

@Entry
@Component
struct ArrowFunctionConversionError {
  @State isWideScreen: boolean = false;
  private mediaListener: mediaquery.MediaQueryListener | null = null;

  aboutToAppear() {
    const mediaQuery = this.getUIContext().getMediaQuery();
    this.mediaListener = mediaQuery.matchMediaSync('(min-width: 600vp)');
    
    if (this.mediaListener) {
      this.isWideScreen = this.mediaListener.matches;
      
      this.mediaListener.on('change', (result: mediaquery.MediaQueryResult) => {
        this.isWideScreen = result.matches;
      });
    }
  }

  aboutToDisappear() {
    if (this.mediaListener) {
      this.mediaListener.off('change');
    }
  }

  build() {
    Column() {
      Text('Function.bind 错误示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 16 })
      
      Text(`屏幕宽度: ${this.isWideScreen ? '宽屏' : '窄屏'}`)
        .fontSize(16)
        .fontColor('#333333')
    }
    .width('100%')
    .padding(16)
  }
}