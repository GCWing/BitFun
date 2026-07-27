@Entry
@Component
struct ImplementationNotAllowedError {
  @State count: number = 0;

  build() {
    Column() {
      Text('实现不允许错误示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 16 })
      
      Text(`计数: ${this.count}`)
        .fontSize(16)
        .fontColor('#333333')
        .margin({ bottom: 16 })
      
      Button('增加')
        .onClick(() => {
          this.count++;
        })
        .margin({ bottom: 16 })
      
      Row() {
        Text('Row 内容')
          .fontSize(14)
          .fontColor('#666666')
      }
      .width('100%')
      .padding(12)
      .backgroundColor('#F5F5F5')
      .borderRadius(4)
    }
    .width('100%')
    .height('100%')
    .padding(16)
  }
}