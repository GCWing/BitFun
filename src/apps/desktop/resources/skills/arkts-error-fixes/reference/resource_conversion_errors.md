# Resource 类型转换错误

## 错误描述

在 ArkTS 中，不能将 `Resource` 类型直接转换为 `string` 或 `number` 类型。Resource 是一个特殊的资源引用类型，需要通过特定的方式使用。

## 错误示例

```typescript
const message: string = $r('app.string.hello');
const fontSize: number = $r('app.float.title_font_size');
```

**错误信息：**
```
Conversion of type 'Resource' to type 'string'/'number' may be a mistake because neither type sufficiently overlaps with the other.
```

## 解决方案

### 方案1：直接在 UI 组件中使用（推荐）

Resource 类型可以直接作为属性值传递给 UI 组件，系统会自动处理资源解析。

```typescript
@Entry
@Component
struct MyComponent {
  build() {
    Column() {
      Text($r('app.string.hello'))
        .fontSize($r('app.float.title_font_size'))
        .width($r('app.float.layout_width'))
        .height($r('app.float.layout_height'))
        .backgroundColor($r('app.color.background_color'))
    }
  }
}
```

### 方案2：使用资源管理器获取字符串值

如果需要获取字符串的实际值，可以使用 `ResourceManager`。

```typescript
import { resourceManager } from '@kit.LocalizationKit';

async function getStringResource(context: Context, resourceId: number): Promise<string> {
  const manager = context.resourceManager;
  return await manager.getString(resourceId);
}
```

### 方案3：使用 getNumber 获取数值资源

对于数值资源，使用 `getNumber` 方法。

```typescript
import { resourceManager } from '@kit.LocalizationKit';

async function getNumberResource(context: Context, resourceId: number): Promise<number> {
  const manager = context.resourceManager;
  return await manager.getNumber(resourceId);
}
```

### 方案4：使用 getStringByName 根据名称获取

根据资源名称获取字符串值。

```typescript
import { resourceManager } from '@kit.LocalizationKit';

async function getStringByName(context: Context, name: string): Promise<string> {
  const manager = context.resourceManager;
  return await manager.getStringByName(name);
}
```

## 详细说明

Resource 类型的特点：

1. **延迟加载**：资源在需要时才被解析
2. **多语言支持**：根据系统语言自动选择对应的资源
3. **主题适配**：支持深色/浅色主题切换
4. **类型安全**：编译时检查资源引用的正确性

## 资源类型对照表

| 资源类型 | $r 语法 | 资源文件位置 | 示例 |
|---------|---------|-------------|------|
| 字符串 | `$r('app.string.name')` | `resources/base/element/string.json` | `$r('app.string.hello')` |
| 颜色 | `$r('app.color.name')` | `resources/base/element/color.json` | `$r('app.color.primary')` |
| 浮点数 | `$r('app.float.name')` | `resources/base/element/float.json` | `$r('app.float.title_font_size')` |
| 整数 | `$r('app.integer.name')` | `resources/base/element/integer.json` | `$r('app.integer.max_count')` |
| 布尔 | `$r('app.boolean.name')` | `resources/base/element/boolean.json` | `$r('app.boolean.is_enabled')` |
| 媒体 | `$r('app.media.name')` | `resources/base/media/` | `$r('app.media.icon')` |

## 简单示例

```typescript
@Entry
@Component
struct ResourceUsageExample {
  @State displayText: string = '';

  async loadStringResource() {
    try {
      const context = this.getUIContext().getHostContext();
      const manager = context.resourceManager;
      this.displayText = await manager.getString($r('app.string.hello').id);
    } catch (err) {
      console.error(`Failed to load string resource: ${JSON.stringify(err)}`);
    }
  }

  build() {
    Column() {
      Text('Resource 类型使用示例')
        .fontSize($r('app.float.title_font_size'))
        .fontWeight(FontWeight.Bold)
        .margin(20)

      Text($r('app.string.hello'))
        .fontSize($r('app.float.content_font_size'))
        .width($r('app.float.layout_width'))
        .padding($r('app.float.padding'))
        .backgroundColor($r('app.color.background_color'))
        .borderRadius($r('app.float.border_radius'))
        .margin(20)

      Button('加载字符串资源')
        .onClick(() => {
          this.loadStringResource();
        })
        .margin(20)

      if (this.displayText) {
        Text(`加载的字符串: ${this.displayText}`)
          .fontSize(14)
          .margin(20)
      }
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}
```

## 详细代码示例

> [ResourceConversionError.ets](../assets/ResourceConversionError.ets) - 完整的 Resource 类型转换错误示例和修复方案
