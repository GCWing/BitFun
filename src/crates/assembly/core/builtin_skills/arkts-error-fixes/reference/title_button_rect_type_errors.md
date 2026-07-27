# TitleButtonRect Type Error

## 错误描述

`window.getTitleButtonRect()` 方法返回的是 `window.TitleButtonRect` 类型，而不是 `window.Rect` 类型。如果函数返回类型声明为 `window.Rect`，会导致类型不匹配错误。

## 错误信息

### 错误 1: 类型不匹配
```
Argument of type 'TitleButtonRect' is not assignable to parameter of type 'Rect | PromiseLike<Rect>'.
  Property 'left' is missing in type 'TitleButtonRect' but required in type 'Rect'.
```

### 错误 2: 访问不存在的属性
```
Property 'left' does not exist on type 'TitleButtonRect'.
Property 'top' does not exist on type 'TitleButtonRect'.
```

**原因**：`TitleButtonRect` 类型只包含 `width` 和 `height` 属性，不包含 `left` 和 `top` 属性。

## 错误示例

```typescript
async function getTitleButtonRect(context: common.UIAbilityContext): Promise<window.Rect> {
  return new Promise((resolve, reject) => {
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        reject(new Error(err.message));
        return;
      }
      const titleButtonRect = win.getTitleButtonRect();
      resolve(titleButtonRect);
    });
  });
}
```

## 解决方案

将函数返回类型从 `window.Rect` 改为 `window.TitleButtonRect`。

```typescript
async function getTitleButtonRect(context: common.UIAbilityContext): Promise<window.TitleButtonRect> {
  return new Promise((resolve, reject) => {
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        reject(new Error(err.message));
        return;
      }
      const titleButtonRect = win.getTitleButtonRect();
      resolve(titleButtonRect);
    });
  });
}
```

## 类型说明

`window.TitleButtonRect` 和 `window.Rect` 是两个不同的类型：

- `window.Rect`: 包含 `left`, `top`, `width`, `height` 属性
- `window.TitleButtonRect`: **只包含** `width`, `height` 属性（不包含 `left` 和 `top` 属性）

**重要提示**：`TitleButtonRect` 类型只提供宽度和高度信息，不包含位置信息（left 和 top）。如果需要位置信息，需要使用其他 API 获取。

## 简单示例

```typescript
import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

async function getTitleButtonRect(context: common.UIAbilityContext): Promise<window.TitleButtonRect> {
  return new Promise((resolve, reject) => {
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        reject(new Error(err.message));
        return;
      }
      const titleButtonRect = win.getTitleButtonRect();
      resolve(titleButtonRect);
    });
  });
}

@Entry
@Component
struct TitleButtonRectExample {
  @State titleBarHeight: number = 0;
  @State titleBarWidth: number = 0;

  async aboutToAppear() {
    const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
    try {
      const titleButtonRect = await getTitleButtonRect(context);
      // TitleButtonRect 只包含 width 和 height 属性
      this.titleBarHeight = titleButtonRect.height;
      this.titleBarWidth = titleButtonRect.width;
      // ❌ 错误：不能访问 left 和 top 属性
      // this.titleBarLeft = titleButtonRect.left;  // Property 'left' does not exist
      // this.titleBarTop = titleButtonRect.top;     // Property 'top' does not exist
    } catch (err) {
      console.error('获取标题栏按钮区域失败:', err instanceof Error ? err.message : String(err));
    }
  }

  build() {
    Column() {
      Text(`标题栏高度: ${this.titleBarHeight}`)
      Text(`标题栏宽度: ${this.titleBarWidth}`)
    }
  }
}
```

## 详细代码示例

- [TitleButtonRectTypeError.ets](../assets/TitleButtonRectTypeError.ets) - TitleButtonRect 类型错误的完整示例，包含错误和正确的解决方案
- [StandaloneFunctionContext.ets](../assets/StandaloneFunctionContext.ets#L34-L47) - 完整的 TitleButtonRect 类型使用示例，包含错误处理
- [StandaloneFunctionError.ets](../assets/StandaloneFunctionError.ets#L34-L47) - 独立函数中的 TitleButtonRect 类型使用示例

## 相关类型

| 类型 | 说明 | 用途 |
|------|------|------|
| `window.Rect` | 通用矩形区域 | 窗口区域、避让区域等 |
| `window.TitleButtonRect` | 标题栏按钮区域 | 标题栏按钮的位置和大小 |
| `window.AvoidArea` | 避让区域 | 系统栏、导航栏等避让区域 |

## 最佳实践

1. **使用正确的返回类型**: 根据实际调用的 API 返回类型来声明函数返回类型
2. **查看 API 文档**: 使用窗口 API 时，仔细查看返回值类型和可用属性
3. **了解类型差异**: `TitleButtonRect` 只包含 `width` 和 `height`，不包含位置信息
4. **类型转换**: 如果需要在不同类型之间转换，创建新的对象而不是直接赋值
5. **避免访问不存在的属性**: 不要尝试访问 `TitleButtonRect` 的 `left` 和 `top` 属性
6. **使用 TypeScript 类型推断**: 在某些情况下，可以省略返回类型注解让编译器推断

## 常见错误

```typescript
// ❌ 错误：返回类型声明为 Rect
async function getTitleButtonRect(): Promise<window.Rect> {
  const win = await window.getLastWindow(context);
  return win.getTitleButtonRect();
}

// ✅ 正确：返回类型声明为 TitleButtonRect
async function getTitleButtonRect(): Promise<window.TitleButtonRect> {
  const win = await window.getLastWindow(context);
  return win.getTitleButtonRect();
}

// ✅ 正确：如果需要 Rect 类型，进行类型转换
// 注意：TitleButtonRect 不包含 left 和 top 属性，只能提供 width 和 height
async function getTitleButtonRectAsRect(): Promise<window.Rect> {
  const win = await window.getLastWindow(context);
  const titleButtonRect = win.getTitleButtonRect();
  return {
    left: 0,  // TitleButtonRect 不提供位置信息，需要从其他 API 获取
    top: 0,   // TitleButtonRect 不提供位置信息，需要从其他 API 获取
    width: titleButtonRect.width,
    height: titleButtonRect.height
  };
}
```
