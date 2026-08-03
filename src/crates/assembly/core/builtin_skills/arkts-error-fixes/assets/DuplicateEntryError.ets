@Entry
@Component
struct DuplicateEntryError {
  @State message: string = 'Hello World';

  build() {
    Column() {
      Text(this.message)
        .fontSize(24)
        .margin({ bottom: 20 })

      ChildComponent()
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}

@Component
struct ChildComponent {
  @State count: number = 0;

  build() {
    Column() {
      Text(`Count: ${this.count}`)
        .fontSize(18)

      Button('Increment')
        .onClick(() => {
          this.count++;
        })
    }
  }
}
