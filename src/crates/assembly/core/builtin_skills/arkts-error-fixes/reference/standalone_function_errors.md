# Standalone Function `this` Usage Error

## 错误描述

在独立函数（非类方法）中不能直接使用 `this`，因为 `this` 在独立函数中没有上下文绑定。

## 错误示例

```typescript
async function getAvoidArea() {
  const win = await window.getLastWindow(this.getUIContext().getHostContext());
  return win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
}
```

**错误信息：**
```
Cannot find name 'this'
```

## 解决方案

将上下文作为参数传递给独立函数。

```typescript
async function getAvoidArea(context: common.UIAbilityContext): Promise<window.AvoidArea> {
  return new Promise((resolve, reject) => {
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        reject(err);
        return;
      }
      const avoidArea = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      resolve(avoidArea);
    });
  });
}
```

## 简单示例

```typescript
@Entry
@Component
struct Example {
  @State avoidAreaHeight: number = 0;

  async aboutToAppear() {
    const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
    try {
      const avoidArea = await getAvoidArea(context);
      this.avoidAreaHeight = avoidArea.topRect.height;
    } catch (err) {
      console.error('获取避让区域失败:', err);
    }
  }

  build() {
    Column() {
      Text(`避让区域高度: ${this.avoidAreaHeight}`)
    }
  }
}
```

## 详细代码示例

- [StandaloneFunctionError.ets](../assets/StandaloneFunctionError.ets) - 完整的独立函数上下文传递示例，包含多个独立函数的使用
