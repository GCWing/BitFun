interface WindowInfo {
  width: number;
  height: number;
}

@Entry
@Component
struct FunctionReturnTypeError {
  @State windowWidth: number = 0;
  @State windowHeight: number = 0;

  aboutToAppear() {
    this.updateWindowInfo();
  }

  private getWindowInfo(): WindowInfo {
    return {
      width: 1080,
      height: 2340
    };
  }

  private updateWindowInfo(): void {
    const info = this.getWindowInfo();
    this.windowWidth = info.width;
    this.windowHeight = info.height;
  }

  build() {
    Column() {
      Text(`Window: ${this.windowWidth} x ${this.windowHeight}`)
        .fontSize(20)
        .margin({ bottom: 20 })
      
      Button('Refresh')
        .onClick(() => {
          this.updateWindowInfo();
        })
    }
  }
}
