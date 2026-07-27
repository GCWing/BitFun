# Object Literal Type Declaration Error

## 错误描述

在 ArkTS 中，不能使用对象字面量作为类型声明。必须使用 `interface` 或 `class` 来定义类型。

## 错误示例

```typescript
function getWindowSize(): { width: number; height: number } {
  return { width: 100, height: 200 };
}
```

**错误信息：**
```
Object literals cannot be used as type declarations (arkts-no-obj-literals-as-types)
```

## 解决方案

### 方案1：使用 interface（推荐）

```typescript
interface WindowSize {
  width: number;
  height: number;
}

function getWindowSize(): WindowSize {
  return { width: 100, height: 200 };
}
```

### 方案2：使用 type 别名

```typescript
type WindowSize = {
  width: number;
  height: number;
};

function getWindowSize(): WindowSize {
  return { width: 100, height: 200 };
}
```

### 方案3：使用 class

```typescript
class WindowSize {
  width: number = 0;
  height: number = 0;
}

function getWindowSize(): WindowSize {
  const size = new WindowSize();
  size.width = 100;
  size.height = 200;
  return size;
}
```

## 详细说明

ArkTS 禁止使用对象字面量作为类型声明，这是为了：

1. **提高代码可读性**：使用命名的类型更清晰
2. **支持类型复用**：interface 和 type 可以在多个地方使用
3. **增强类型检查**：明确的类型定义可以提供更好的类型推断

## 简单示例

```typescript
interface WindowSize {
  width: number;
  height: number;
}

@Entry
@Component
struct WindowSizeExample {
  @State windowSize: WindowSize = { width: 0, height: 0 };

  aboutToAppear() {
    this.windowSize = this.getWindowSize();
  }

  private getWindowSize(): WindowSize {
    const windowStage = this.getUIContext().getHostContext() as common.UIAbilityContext;
    const windowClass = windowStage.getMainWindowSync();
    const windowProperties = windowClass.getWindowProperties();
    return {
      width: windowProperties.windowRect.width,
      height: windowProperties.windowRect.height
    };
  }

  build() {
    Column() {
      Text(`宽度: ${this.windowSize.width}`)
      Text(`高度: ${this.windowSize.height}`)
    }
  }
}
```

## 详细代码示例

> [ObjectLiteralTypeError.ets](../assets/ObjectLiteralTypeError.ets) - 完整的对象字面量类型声明错误示例和修复方案
