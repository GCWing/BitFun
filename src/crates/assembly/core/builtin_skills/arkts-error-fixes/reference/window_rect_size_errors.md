# window.Rect 和 window.Size 类型错误

## 常见错误

### 1. window.Point 不存在

```
Property 'Point' does not exist on type 'Namespace window'
```

### 2. window.Size 不存在

```
Property 'size' does not exist on type 'WindowProperties'
```

### 3. window.Rect 属性访问错误

```
Property 'x' does not exist on type 'Rect'
Property 'y' does not exist on type 'Rect'
```

## 错误原因

- `window.Rect` 使用 `left/top` 而非 `x/y`
- `WindowProperties` 没有 `size` 属性，需要从 `windowRect` 获取
- `window.Point` 类型不存在

## 解决方案

### window.Rect 正确使用

```typescript
// window.Rect 有 left, top, width, height 属性
const rect: window.Rect = { left: 0, top: 0, width: 100, height: 100 };
```

### 从 WindowProperties 获取尺寸

```typescript
const properties = win.getWindowProperties();
const rect = properties.windowRect;
const width = rect.width;
const height = rect.height;
```

### windowSizeChange 事件回调

```typescript
win.on('windowSizeChange', (size: window.Size) => {
  // size 有 width 和 height 属性
  this.windowRect = {
    left: this.windowRect.left,
    top: this.windowRect.top,
    width: size.width,
    height: size.height
  };
});
```

## 详细代码示例

请参考 [WindowRectSizeError.ets](../assets/WindowRectSizeError.ets)

## 相关 API

- [WindowProperties](https://developer.harmonyos.com/cn/docs/documentation/reference/apis-arkui-window-0000001774129417)
- [window.Rect](https://developer.harmonyos.com/cn/docs/documentation/reference/apis-arkui-window-0000001774129417)
- [window.Size](https://developer.harmonyos.com/cn/docs/documentation/reference/apis-arkui-window-0000001774129417)
