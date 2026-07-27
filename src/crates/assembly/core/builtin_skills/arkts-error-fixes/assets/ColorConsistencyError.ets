@Entry
@Component
struct ColorConsistencyExample {
  @State useSystemColor: boolean = false;

  build() {
    Column() {
      Text('颜色一致性示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 20 })

      Text('使用硬编码颜色（不推荐）')
        .fontSize(14)
        .fontColor('#666666')
        .margin({ bottom: 8 })

      Column() {
        Text('白色背景')
          .fontSize(16)
          .padding(12)
          .margin({ bottom: 8 })
          .width('100%')
          .backgroundColor('#FFFFFF')
          .borderRadius(8)

        Text('灰色背景')
          .fontSize(16)
          .padding(12)
          .margin({ bottom: 8 })
          .width('100%')
          .backgroundColor('#F5F5F5')
          .borderRadius(8)

        Text('蓝色背景')
          .fontSize(16)
          .padding(12)
          .width('100%')
          .backgroundColor('#007DFF')
          .fontColor('#FFFFFF')
          .borderRadius(8)
      }
      .width('100%')
      .padding(16)
      .backgroundColor('#F0F0F0')
      .borderRadius(8)
      .margin({ bottom: 20 })

      Text('使用系统颜色资源（推荐）')
        .fontSize(14)
        .fontColor('#666666')
        .margin({ bottom: 8 })

      Column() {
        Text('系统背景色')
          .fontSize(16)
          .padding(12)
          .margin({ bottom: 8 })
          .width('100%')
          .backgroundColor($r('sys.color.ohos_id_color_background'))
          .borderRadius(8)

        Text('系统分割线色')
          .fontSize(16)
          .padding(12)
          .margin({ bottom: 8 })
          .width('100%')
          .backgroundColor($r('sys.color.ohos_id_color_list_separator'))
          .borderRadius(8)

        Text('系统主色调')
          .fontSize(16)
          .padding(12)
          .width('100%')
          .backgroundColor($r('sys.color.ohos_id_color_primary'))
          .fontColor($r('sys.color.ohos_id_color_text_primary'))
          .borderRadius(8)
      }
      .width('100%')
      .padding(16)
      .backgroundColor($r('sys.color.ohos_id_color_sub_background'))
      .borderRadius(8)
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}
