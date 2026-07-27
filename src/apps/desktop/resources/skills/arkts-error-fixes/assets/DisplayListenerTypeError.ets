import { display } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';

/**
 * ❌ 错误示例：在 Display 对象上调用 on/off 方法
 * 
 * 错误原因：display.on() 和 display.off() 是 display 模块的方法，
 * 不是 Display 对象实例的方法
 */
@Entry
@Component
struct DisplayListenerTypeError {
  @State displayId: number = 0;
  private displayListener?: number;

  aboutToAppear() {
    try {
      const displayClass = display.getDefaultDisplaySync();
      this.displayId = displayClass.id;

      // ❌ 错误：在 Display 对象上调用 on 方法
      // Type 'void' is not assignable to type 'number'
      this.displayListener = displayClass.on('change', () => {
        hilog.info(0x0000, 'DisplayError', 'Display changed');
      });
    } catch (error) {
      hilog.error(0x0000, 'DisplayError', `Failed to register listener: ${JSON.stringify(error)}`);
    }
  }

  aboutToDisappear() {
    // ❌ 错误：在 Display 对象上调用 off 方法
    // Argument of type 'number' is not assignable to parameter of type 'Callback<Rect>'
    if (this.displayListener !== undefined) {
      const displayClass = display.getDefaultDisplaySync();
      displayClass.off('change', this.displayListener);
    }
  }

  build() {
    Column() {
      Text(`Display ID: ${this.displayId}`)
        .fontSize(20)
    }
  }
}

/**
 * ✅ 正确示例：在 display 模块上调用 on/off 方法
 */
@Entry
@Component
struct DisplayListenerCorrect {
  @State displayId: number = 0;
  private displayListener?: (data: number) => void;

  aboutToAppear() {
    try {
      const displayClass = display.getDefaultDisplaySync();
      this.displayId = displayClass.id;

      // ✅ 正确：在 display 模块上调用 on 方法
      this.displayListener = (data: number) => {
        hilog.info(0x0000, 'DisplayCorrect', `Display changed, ID: ${data}`);
      };
      display.on('change', this.displayListener);
    } catch (error) {
      hilog.error(0x0000, 'DisplayCorrect', `Failed to register listener: ${JSON.stringify(error)}`);
    }
  }

  aboutToDisappear() {
    // ✅ 正确：在 display 模块上调用 off 方法
    if (this.displayListener) {
      display.off('change', this.displayListener);
      this.displayListener = undefined;
    }
  }

  build() {
    Column() {
      Text(`Display ID: ${this.displayId}`)
        .fontSize(20)
    }
  }
}

/**
 * ✅ 正确示例：监听 Display 对象的 availableAreaChange
 */
@Entry
@Component
struct DisplayAvailableAreaCorrect {
  @State availableArea: string = 'Unknown';
  private areaListener?: (data: display.Rect) => void;

  aboutToAppear() {
    try {
      const displayClass = display.getDefaultDisplaySync();
      this.availableArea = `Width: ${displayClass.width}, Height: ${displayClass.height}`;

      // ✅ 正确：在 Display 对象上调用 on 方法监听 availableAreaChange
      this.areaListener = (data: display.Rect) => {
        hilog.info(0x0000, 'DisplayArea', `Available area changed: ${JSON.stringify(data)}`);
        this.availableArea = `Width: ${data.width}, Height: ${data.height}`;
      };
      displayClass.on('availableAreaChange', this.areaListener);
    } catch (error) {
      hilog.error(0x0000, 'DisplayArea', `Failed to register listener: ${JSON.stringify(error)}`);
    }
  }

  aboutToDisappear() {
    // ✅ 正确：在 Display 对象上调用 off 方法注销 availableAreaChange
    if (this.areaListener) {
      const displayClass = display.getDefaultDisplaySync();
      displayClass.off('availableAreaChange', this.areaListener);
      this.areaListener = undefined;
    }
  }

  build() {
    Column() {
      Text(`Available Area: ${this.availableArea}`)
        .fontSize(20)
        .margin({ top: 10 })
    }
  }
}
