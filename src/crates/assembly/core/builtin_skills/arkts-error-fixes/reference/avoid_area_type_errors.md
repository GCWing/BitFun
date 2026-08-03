# AvoidArea Type Error

## 错误描述

在使用 `window.AvoidArea` 类型时，如果缺少 `visible` 属性，会导致类型错误。

## 错误示例

```typescript
@State avoidArea: window.AvoidArea = {
  topRect: { left: 0, top: 0, width: 0, height: 0 },
  bottomRect: { left: 0, top: 0, width: 0, height: 0 },
  leftRect: { left: 0, top: 0, width: 0, height: 0 },
  rightRect: { left: 0, top: 0, width: 0, height: 0 }
};
```

**错误信息：**
```
Type '{ topRect: { left: number; top: number; width: number; height: number; }; bottomRect: { left: number; top: number; width: number; height: number; }; leftRect: { left: number; top: number; width: number; height: number; }; rightRect: { left: number; top: number; width: number; height: number; }; }' is missing the following properties from type 'AvoidArea': visible
```

## 解决方案

在 `AvoidArea` 对象中添加 `visible: false` 属性。

```typescript
@State avoidArea: window.AvoidArea = {
  topRect: { left: 0, top: 0, width: 0, height: 0 },
  bottomRect: { left: 0, top: 0, width: 0, height: 0 },
  leftRect: { left: 0, top: 0, width: 0, height: 0 },
  rightRect: { left: 0, top: 0, width: 0, height: 0 },
  visible: false
};
```

## 简单示例

```typescript
@Entry
@Component
struct SimpleExample {
  @State avoidArea: window.AvoidArea = {
    topRect: { left: 0, top: 0, width: 0, height: 0 },
    bottomRect: { left: 0, top: 0, width: 0, height: 0 },
    leftRect: { left: 0, top: 0, width: 0, height: 0 },
    rightRect: { left: 0, top: 0, width: 0, height: 0 },
    visible: false
  };

  build() {
    Text(`顶部高度: ${this.avoidArea.topRect.height}`)
  }
}
```

## 详细代码示例

- [AvoidAreaTypeError.ets](../assets/AvoidAreaTypeError.ets) - 完整的 AvoidArea 类型使用示例，包含窗口获取和避让区域监听
