interface WindowUtilDensity {
  onDensityUpdate: () => void;
  updateWindowInfo: () => void;
  destroy: () => void;
}

export const windowUtilDensityExample: WindowUtilDensity = {
  onDensityUpdate: () => {
    console.log('Density updated');
  },

  updateWindowInfo: () => {
    console.log('Window info updated');
  },

  destroy: () => {
    console.log('Window util destroyed');
  }
};

@Entry
@Component
struct InterfaceMethodSignatureError {
  @State windowWidth: number = 0;
  @State windowHeight: number = 0;

  aboutToAppear() {
    windowUtilDensityExample.updateWindowInfo();
  }

  build() {
    Column() {
      Text(`Window: ${this.windowWidth} x ${this.windowHeight}`)
        .fontSize(20)
        .margin({ bottom: 20 })
      
      Button('Update Info')
        .onClick(() => {
          windowUtilDensityExample.updateWindowInfo();
        })
    }
  }
}
