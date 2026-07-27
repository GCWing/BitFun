# IDataSource 类型错误

## 错误信息

```
Argument of type 'string[]' is not assignable to parameter of type 'IDataSource'.
Type 'string[]' is missing the following properties from type 'IDataSource': 
  totalCount, getData, registerDataChangeListener, unregisterDataChangeListener
```

## 错误原因

在 ArkTS 中，`LazyForEach` 要求数据源必须实现 `IDataSource` 接口，直接使用 `string[]` 会导致类型错误。

## 错误示例

```typescript
// 错误 - 直接使用 string[]
private data: string[] = ['Item 1', 'Item 2', 'Item 3'];

build() {
  List() {
    LazyForEach(this.data, (item: string) => {
      ListItem() {
        Text(item)
      }
    }, (item: string) => item)
  }
}
```

## 解决方案

实现 `IDataSource` 接口：

```typescript
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
      listener.onDataAdd(this.data.length - 1);
    });
  }
}
```

## 详细代码示例

请参考 [IDataSourceError.ets](../assets/IDataSourceError.ets)

## 相关 API

- [LazyForEach](https://developer.harmonyos.com/cn/docs/documentation/doc-guides/arkts-rendering-control-000000177275贼299)
- [IDataSource](https://developer.harmonyos.com/cn/docs/documentation/doc-references/arkts-common-0000001774129201)
- [DataChangeListener](https://developer.harmonyos.com/cn/docs/documentation/doc-references/arkts-common-0000001774129201)
