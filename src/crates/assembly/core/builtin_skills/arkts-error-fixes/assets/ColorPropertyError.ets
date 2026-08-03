@Entry
@Component
struct ColorPropertyError {
  @State isDarkMode: boolean = false;

  build() {
    Column() {
      Text('Color Property Example')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 20 })
      
      Text('Primary Color')
        .fontSize(16)
        .fontColor('#007DFF')
        .margin({ bottom: 10 })
      
      Text('Secondary Color')
        .fontSize(16)
        .fontColor('#FF6B00')
        .margin({ bottom: 10 })
      
      Text('Background Color')
        .fontSize(16)
        .backgroundColor('#F5F5F5')
        .padding(10)
        .margin({ bottom: 20 })
      
      Button('Toggle Theme')
        .onClick(() => {
          this.isDarkMode = !this.isDarkMode;
        })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(this.isDarkMode ? '#1A1A1A' : '#FFFFFF')
    .padding(20)
  }
}
