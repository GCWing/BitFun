// 错误示例 - LazyForEach 使用 string[] 直接报错
@Entry
@Component
struct IDataSourceErrorExample {
  private data: string[] = ['Item 1', 'Item 2', 'Item 3'];

  build() {
    Column() {
      List() {
        LazyForEach(this.data, (item: string) => {
          ListItem() {
            Text(item)
          }
        }, (item: string) => item)
      }
      .width('100%')
      .height('100%')
    }
  }
}

// 解决方案 - 实现 IDataSource 接口
class MyDataSource {
  data: string[] = [];
  private listeners: DataChangeListener[] = [];

  totalCount(): number {
    return this.data.length;
  }

  getData(index: number): string {
    return this.data[index];
  }

  registerDataChangeListener(listener: DataChangeListener): void {
    this.listeners.push(listener);
  }

  unregisterDataChangeListener(listener: DataChangeListener): void {
    const index = this.listeners.indexOf(listener);
    if (index > -1) {
      this.listeners.splice(index, 1);
    }
  }

  pushData(data: string): void {
    this.data.push(data);
    this.listeners.forEach((listener: DataChangeListener) => {
      listener.onDataAdd?.(this.data.length - 1);
    });
  }
}

@Entry
@Component
struct IDataSourceCorrectExample {
  private data: MyDataSource = new MyDataSource();

  aboutToAppear() {
    for (let i = 0; i < 100; i++) {
      this.data.pushData(`Item ${i}`);
    }
  }

  build() {
    Column() {
      List() {
        LazyForEach(this.data, (item: string) => {
          ListItem() {
            Text(item).fontSize(20).padding(10)
          }
        }, (item: string) => item)
      }
      .cachedCount(5)
      .width('100%')
      .height('100%')
    }
  }
}
