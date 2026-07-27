import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

@Entry
@Component
struct WindowTypeError {
  @State windowWidth: number = 0;

  aboutToAppear() {
    try {
      const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
      
      window.getLastWindow(context, (err, win) => {
        if (err.code !== 0) {
          console.error('Failed to get window:', err);
          return;
        }
        
        const properties = win.getWindowProperties();
        this.windowWidth = properties.windowRect.width;
        
        win.on('windowSizeChange', (size: window.Size) => {
          this.windowWidth = size.width;
        });
      });
    } catch (err) {
      console.error('Failed to get context:', err);
    }
  }

  build() {
    Column() {
      Text(`Window Width: ${this.windowWidth}`)
        .fontSize(20)
    }
  }
}
