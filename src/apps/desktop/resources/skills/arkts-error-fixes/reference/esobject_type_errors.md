# ESObject 类型限制错误

## 错误描述

在 ArkTS 中，`ESObject` 类型的使用受到限制，不能直接用作变量类型声明。这是为了确保类型安全和避免运行时错误。

## 错误示例

```typescript
let nfcModule: ESObject = null;
```

**错误信息：**
```
Usage of "ESObject" type is restricted (arkts-limited-esobj)
```

## 解决方案

### 方案1：使用动态导入（推荐）

对于需要动态导入的模块，使用 `import()` 函数并使用 `Promise` 或 `ESModule` 类型。

```typescript
let nfcModule: ESModule | null = null;
try {
  nfcModule = import('@kit.ConnectivityKit');
} catch (err) {
  console.error(`Failed to import NFC module: ${JSON.stringify(err)}`);
}
```

### 方案2：使用具体类型

如果知道模块的具体类型，使用具体的接口或类。

```typescript
import { nfcController } from '@kit.ConnectivityKit';

let nfcControllerInstance: nfcController.NfcController | null = null;
```

### 方案3：使用 unknown 类型

如果不确定模块类型，可以使用 `unknown` 类型，但需要在使用时进行类型检查。

```typescript
let module: unknown = null;
try {
  module = import('@kit.ConnectivityKit');
  if (module !== null && module !== undefined) {
    // 使用前进行类型检查
    console.info('Module imported successfully');
  }
} catch (err) {
  console.error(`Failed to import module: ${JSON.stringify(err)}`);
}
```

## 详细说明

ArkTS 限制 `ESObject` 类型的使用是为了：

1. **类型安全**：避免运行时类型错误
2. **编译时检查**：确保类型使用的正确性
3. **模块系统规范**：遵循 ES 模块标准

## 简单示例

```typescript
@Entry
@Component
struct DynamicImportExample {
  @State moduleLoaded: boolean = false;
  @State moduleName: string = '';

  async loadNFCModule() {
    try {
      const nfcModule = await import('@kit.ConnectivityKit');
      this.moduleLoaded = true;
      this.moduleName = 'NFC';
      console.info('NFC module imported successfully');
    } catch (err) {
      console.error(`Failed to import NFC module: ${JSON.stringify(err)}`);
      this.moduleLoaded = false;
      this.moduleName = 'NFC';
    }
  }

  build() {
    Column() {
      Button('Load NFC Module')
        .onClick(() => {
          this.loadNFCModule();
        })
        .margin(20)

      if (this.moduleLoaded) {
        Text(`${this.moduleName} module loaded successfully`)
          .fontSize(16)
          .margin(20)
      }
    }
  }
}
```

## 详细代码示例

> [ESObjectTypeError.ets](../assets/ESObjectTypeError.ets) - 完整的ESObject类型错误示例和修复方案
