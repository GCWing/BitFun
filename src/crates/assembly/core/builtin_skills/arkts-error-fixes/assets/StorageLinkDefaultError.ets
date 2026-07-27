import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

@Entry
@Component
struct StorageLinkDefaultError {
  @StorageLink('windowUtil') windowUtil?: WindowUtil = undefined;

  aboutToAppear() {
    AppStorage.setOrCreate('windowUtil', {
      width: 0,
      height: 0,
      density: 1.0,
      updateWindowInfo: () => {},
      destroy: () => {}
    });
  }

  build() {
    Column() {
      Text(`Window Util: ${this.windowUtil?.width || 0}`)
        .fontSize(20)
        .margin({ bottom: 20 })
      
      Button('Update Window')
        .onClick(() => {
          if (this.windowUtil) {
            this.windowUtil.width = 100;
          }
        })
    }
  }
}

interface WindowUtil {
  width: number;
  height: number;
  density: number;
  updateWindowInfo: () => void;
  destroy: () => void;
}
