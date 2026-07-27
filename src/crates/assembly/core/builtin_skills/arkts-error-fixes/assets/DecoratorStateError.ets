import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

class WindowStateManager {
  widthBreakpoint: string = 'sm';
  isFolded: boolean = true;

  updateState(width: number, height: number) {
    const area = width * height;
    if (area < 600 * 600) {
      this.widthBreakpoint = 'sm';
      this.isFolded = true;
    } else if (area < 840 * 600) {
      this.widthBreakpoint = 'md';
      this.isFolded = false;
    } else {
      this.widthBreakpoint = 'lg';
      this.isFolded = false;
    }
  }
}

@Component
struct DecoratorStateExample {
  @State currentBreakpoint: string = 'sm';
  @State isFolded: boolean = true;

  aboutToAppear() {
    try {
      let context = this.getUIContext().getHostContext() as common.UIAbilityContext;
      window.getLastWindow(context, (err, win) => {
        if (err.code !== 0) {
          console.error('Failed to get window:', err);
          return;
        }
        const properties = win.getWindowProperties();
        this.updateState(properties.windowRect.width, properties.windowRect.height);
      });
    } catch (err) {
      console.error('Failed to get window:', err);
    }
  }

  private updateState(width: number, height: number) {
    const area = width * height;
    if (area < 600 * 600) {
      this.currentBreakpoint = 'sm';
      this.isFolded = true;
    } else if (area < 840 * 600) {
      this.currentBreakpoint = 'md';
      this.isFolded = false;
    } else {
      this.currentBreakpoint = 'lg';
      this.isFolded = false;
    }
  }

  build() {
    Column() {
      Text(`Breakpoint: ${this.currentBreakpoint}`)
      Text(`Folded: ${this.isFolded ? 'Yes' : 'No'}`)
    }
  }
}
