@Entry
@Component
struct FontColorPropertyError {
  @State message: string = '点击按钮测试';
  @State showToast: boolean = false;

  build() {
    Column() {
      Text('fontColor 属性错误示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 16 })
      
      Text('正确：Text 组件使用 fontColor')
        .fontSize(16)
        .fontColor('#333333')
        .margin({ bottom: 8 })
      
      Text('正确：使用 fontColor 替代 textColor')
        .fontSize(16)
        .fontColor('#666666')
        .margin({ bottom: 8 })
      
      Button('点击提示')
        .onClick(() => {
          this.showToast = true;
          setTimeout(() => {
            this.showToast = false;
          }, 2000);
        })
      
      if (this.showToast) {
        Text(this.message)
          .fontSize(14)
          .fontColor('#666666')
          .margin({ top: 8 })
      }
    }
    .width('100%')
    .padding(16)
  }
}