# fontColor 属性错误

## 错误描述

`fontColor` 属性只能用于文本组件（如 `Text`、`Span`、`Button` 等），不能用于容器组件（如 `Column`、`Row`、`Stack` 等）。在容器组件上使用 `fontColor` 会导致编译错误。

## 错误示例

```typescript
@Entry
@Component
struct FontColorError {
  build() {
    Column() {
      Text('Hello World')
    }
    .fontColor('#FF0000')
  }
}
```

**错误信息：**
```
Property 'fontColor' does not exist on type 'ColumnAttribute'
```

## 解决方案

### 方案一：将 fontColor 应用到 Text 组件

将 `fontColor` 属性从容器组件移除，直接应用到文本组件上。

```typescript
@Entry
@Component
struct FontColorCorrect {
  build() {
    Column() {
      Text('Hello World')
        .fontColor('#FF0000')
    }
  }
}
```

## 简单示例

```typescript
@Entry
@Component
struct FontColorExample {
  build() {
    Column() {
      Text('标题')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .fontColor('#333333')
      
      Text('内容')
        .fontSize(16)
        .fontColor('#666666')
    }
    .width('100%')
    .padding(16)
  }
}
```

## 详细代码示例

- [FontColorPropertyError.ets](../assets/FontColorPropertyError.ets) - 完整的 fontColor 属性错误修复示例，包含正确和错误的用法对比

## 最佳实践

1. **仅在文本组件上使用 fontColor**：`fontColor` 属性应该只用于 `Text`、`Span`、`Button` 等文本组件
2. **使用十六进制颜色值**：推荐使用十六进制颜色值（如 `#FF0000`）而不是 `Color` 枚举
3. **定义颜色常量**：对于重复使用的颜色，建议定义常量
4. **使用资源引用**：对于主题相关的颜色，使用资源引用以便于主题切换

## 常见错误

```typescript
// ❌ 错误：在 Column 上使用 fontColor
Column() {
  Text('Hello')
}
.fontColor('#FF0000')

// ❌ 错误：在 Row 上使用 fontColor
Row() {
  Text('Hello')
}
.fontColor('#FF0000')

// ✅ 正确：在 Text 上使用 fontColor
Column() {
  Text('Hello')
    .fontColor('#FF0000')
}

// ✅ 正确：在 Button 上使用 fontColor
Button('Click Me')
  .fontColor('#FFFFFF')
```
