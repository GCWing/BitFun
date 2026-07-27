import { mediaquery } from '@kit.ArkUI';

@Entry
@Component
struct BreakpointTypeError {
  @State currentBreakpoint: string = 'sm';
  private smListener: mediaquery.MediaQueryListener | null = null;
  private mdListener: mediaquery.MediaQueryListener | null = null;
  private lgListener: mediaquery.MediaQueryListener | null = null;

  aboutToAppear() {
    const mediaQuery = this.getUIContext().getMediaQuery();
    
    this.smListener = mediaQuery.matchMediaSync('(0vp<=width<600vp)');
    this.mdListener = mediaQuery.matchMediaSync('(600vp<=width<840vp)');
    this.lgListener = mediaQuery.matchMediaSync('(840vp<=width)');
    
    const smCallback = (result: mediaquery.MediaQueryResult): void => {
      if (result.matches) {
        this.currentBreakpoint = 'sm';
      }
    };
    const mdCallback = (result: mediaquery.MediaQueryResult): void => {
      if (result.matches) {
        this.currentBreakpoint = 'md';
      }
    };
    const lgCallback = (result: mediaquery.MediaQueryResult): void => {
      if (result.matches) {
        this.currentBreakpoint = 'lg';
      }
    };
    
    if (this.smListener) {
      this.smListener.on('change', smCallback);
    }
    if (this.mdListener) {
      this.mdListener.on('change', mdCallback);
    }
    if (this.lgListener) {
      this.lgListener.on('change', lgCallback);
    }
  }

  aboutToDisappear() {
    if (this.smListener) {
      this.smListener.off('change');
    }
    if (this.mdListener) {
      this.mdListener.off('change');
    }
    if (this.lgListener) {
      this.lgListener.off('change');
    }
  }

  build() {
    Column() {
      Text('断点类型错误示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 16 })
      
      Text(`当前断点: ${this.currentBreakpoint}`)
        .fontSize(16)
        .fontColor('#333333')
    }
    .width('100%')
    .padding(16)
  }
}