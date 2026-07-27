import { display } from '@kit.ArkUI';

@Entry
@Component
struct PossiblyNullError {
  private myDisplay: display.Display | null = null;

  aboutToAppear() {
    this.myDisplay = display.getDefaultDisplaySync();
  }

  build() {
    Column() {
      if (this.myDisplay !== null) {
        Text(`Width: ${this.myDisplay.width}`)
          .fontSize(24)
      } else {
        Text('Display not available')
          .fontSize(24)
      }

      Button('Check Display')
        .onClick(() => {
          if (this.myDisplay !== null) {
            console.log(`Display: ${this.myDisplay.width}x${this.myDisplay.height}`);
          }
        })
    }
    .width('100%')
    .height('100%')
  }
}
