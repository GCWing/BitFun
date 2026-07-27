import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

@Entry
@Component
struct AvoidAreaTypeError {
  @State avoidArea: window.AvoidArea = {
    topRect: { left: 0, top: 0, width: 0, height: 0 },
    bottomRect: { left: 0, top: 0, width: 0, height: 0 },
    leftRect: { left: 0, top: 0, width: 0, height: 0 },
    rightRect: { left: 0, top: 0, width: 0, height: 0 },
    visible: false
  };

  aboutToAppear() {
    const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
    
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        console.error('获取窗口失败:', err);
        return;
      }
      
      this.avoidArea = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      
      win.on('avoidAreaChange', (data) => {
        if (data.type === window.AvoidAreaType.TYPE_SYSTEM) {
          this.avoidArea = data.area;
        }
      });
    });
  }

  build() {
    Column() {
      Text(`Top height: ${this.avoidArea.topRect.height}`)
      Text(`Bottom height: ${this.avoidArea.bottomRect.height}`)
      Text(`Left width: ${this.avoidArea.leftRect.width}`)
      Text(`Right width: ${this.avoidArea.rightRect.width}`)
      Text(`Visible: ${this.avoidArea.visible}`)
    }
  }
}
