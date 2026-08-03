@Entry
@Component
struct UnusedVariableWarningExample {
  @State scrollOffset: number = 0;
  private scroller: Scroller = new Scroller();

  build() {
    Column() {
      Text('未使用变量警告示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 20 })

      Column({ space: 12 }) {
        Text('❌ 错误示例: scrollState 未使用')
          .fontSize(14)
          .fontColor('#666666')

        Text('.onDidScroll((scrollOffset: number, scrollState: ScrollState) => {')
          .fontSize(12)
          .fontColor('#FF5722')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#FFF3E0')
          .borderRadius(4)

        Text('  this.scrollOffset = scrollOffset;')
          .fontSize(12)
          .fontColor('#FF5722')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#FFF3E0')
          .borderRadius(4)

        Text('})')
          .fontSize(12)
          .fontColor('#FF5722')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#FFF3E0')
          .borderRadius(4)

        Text('✅ 正确示例 1: 使用下划线前缀标记未使用参数')
          .fontSize(14)
          .fontColor('#666666')

        Text('.onDidScroll((scrollOffset: number, _scrollState: ScrollState) => {')
          .fontSize(12)
          .fontColor('#4CAF50')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#E8F5E9')
          .borderRadius(4)

        Text('  this.scrollOffset = scrollOffset;')
          .fontSize(12)
          .fontColor('#4CAF50')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#E8F5E9')
          .borderRadius(4)

        Text('})')
          .fontSize(12)
          .fontColor('#4CAF50')
          .fontFamily('monospace')
          .padding(8)
          .backgroundColor('#E8F5E9')
          .borderRadius(4)
      }
      .alignItems(HorizontalAlign.Start)
      .margin({ bottom: 20 })

      Text('实际使用示例:')
        .fontSize(14)
        .fontColor('#666666')
        .margin({ bottom: 8 })

      Scroll(this.scroller) {
        Column() {
          ForEach(Array.from({ length: 50 }), (_: Object, index: number) => {
            Text(`Item ${index + 1}`)
              .fontSize(16)
              .padding(12)
              .margin({ bottom: 8 })
              .backgroundColor('#F5F5F5')
              .borderRadius(4)
          }, (_: Object, index: number) => `${index}`)
        }
        .width('100%')
      }
      .scrollable(ScrollDirection.Vertical)
      .scrollBar(BarState.Auto)
      .onDidScroll((scrollOffset: number, _scrollState: ScrollState) => {
        this.scrollOffset = scrollOffset;
      })
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}
