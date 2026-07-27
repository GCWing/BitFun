@Entry
@Component
struct AppStorageError {
  @StorageLink('myValue') myValue: number = 0;

  aboutToAppear() {
    AppStorage.setOrCreate('myValue', 0);
  }

  build() {
    Column() {
      Text(`Value: ${this.myValue}`)
        .fontSize(20)
        .margin({ bottom: 20 })
      
      Button('Increment')
        .onClick(() => {
          this.myValue += 1;
        })
    }
  }
}
