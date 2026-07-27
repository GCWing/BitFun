// window.Rect 和 window.Size 类型使用示例
import { window } from '@kit.ArkUI';

@Entry
@Component
struct WindowRectSizeExample {
  @State windowRect: window.Rect = { left: 0, top: 0, width: 0, height: 0 };
  @State windowSize: window.Size = { width: 0, height: 0 };
  @State breakpoint: string = 'sm';

  aboutToAppear() {
    try {
      window.getLastWindow(this.getUIContext().getHostContext(), (err, win) => {
        if (err.code !== 0) {
          console.error('Failed to get window:', err);
          return;
        }
        const properties = win.getWindowProperties();
        this.windowRect = properties.windowRect;
        this.windowSize = { width: properties.windowRect.width, height: properties.windowRect.height };
        this.breakpoint = this.calculateBreakpoint(properties.windowRect.width);

        win.on('windowSizeChange', (size: window.Size) => {
          this.windowSize = size;
          this.windowRect = {
            left: this.windowRect.left,
            top: this.windowRect.top,
            width: size.width,
            height: size.height
          };
          this.breakpoint = this.calculateBreakpoint(size.width);
        });
      });
    } catch (e) {
      console.error('Error:', e);
    }
  }

  private calculateBreakpoint(width: number): string {
    if (width < 600) return 'sm';
    if (width < 840) return 'md';
    if (width < 1440) return 'lg';
    return 'xl';
  }

  build() {
    Column({ space: 20 }) {
      Text('窗口信息')
        .fontSize(24)
        .fontWeight(FontWeight.Bold)

      Text(`窗口位置: x=${this.windowRect.left}, y=${this.windowRect.top}`)
        .fontSize(16)

      Text(`窗口尺寸: w=${this.windowRect.width}, h=${this.windowRect.height}`)
        .fontSize(16)

      Text(`当前断点: ${this.breakpoint}`)
        .fontSize(16)
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}
