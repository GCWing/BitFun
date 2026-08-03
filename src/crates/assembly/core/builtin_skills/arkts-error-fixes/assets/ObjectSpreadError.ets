interface Config {
  width: number;
  height: number;
  color?: string;
}

@Entry
@Component
struct ObjectSpreadError {
  @State config: Config = { width: 100, height: 100 };

  build() {
    Column() {
      Text(`Width: ${this.config.width}, Height: ${this.config.height}`)
        .fontSize(20)
        .margin({ bottom: 20 })
      
      Button('Update Config')
        .onClick(() => {
          const newConfig: Config = {
            width: this.config.width,
            height: this.config.height,
            color: 'red'
          };
          this.config = newConfig;
        })
    }
  }
}
