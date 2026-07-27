# 对象可能为 null 错误

## 错误描述

在 ArkTS 中，编译器会严格检查可能为 null 的对象访问。如果对象可能为 null，但直接访问其属性，会导致"Object is possibly 'null'"错误。

### 错误信息

```
Object is possibly 'null'
```

或

```
Variable 'xxx' is possibly null
```

## 错误示例

```typescript
// ❌ 错误：对象可能为 null
let display = display.getDefaultDisplaySync();
console.log(display.width); // 错误：Object is possibly 'null'

// ❌ 错误：条件判断不完整
if (display) {
  console.log(display.width); // 某些情况下仍可能为 null
}
```

## 解决方案

### 方案一：使用 !== null 检查

显式检查对象不等于 null：

```typescript
// ✅ 正确：使用 !== null 检查
let display = display.getDefaultDisplaySync();
if (display !== null) {
  console.log(display.width);
}
```

### 方案二：使用可选链和空值合并

```typescript
// ✅ 正确：使用可选链和空值合并
let display = display.getDefaultDisplaySync();
let width = display?.width ?? 0;
```

### 方案三：使用 let 声明可空类型

```typescript
// ✅ 正确：显式声明可空类型
let display: Display | null = display.getDefaultDisplaySync();
if (display !== null) {
  console.log(display.width);
}
```

### 方案四：非空断言（谨慎使用）

```typescript
// ✅ 正确（但需确保不会为 null）：使用非空断言
let display = display.getDefaultDisplaySync()!;
console.log(display.width);
```

## 简单示例

```typescript
import { display } from '@kit.ArkUI';

@Entry
@Component
struct NullCheckExample {
  private myDisplay: Display | null = null;

  aboutToAppear() {
    this.myDisplay = display.getDefaultDisplaySync();
  }

  build() {
    Column() {
      if (this.myDisplay !== null) {
        Text(`Width: ${this.myDisplay.width}`)
          .fontSize(24)
      }
    }
    .width('100%')
  }
}
```

## 详细代码示例

- [PossiblyNullError.ets](../assets/PossiblyNullError.ets) - 完整的 null 检查错误修复示例

## 最佳实践

1. **优先使用 !== null 检查**：最安全的方式，显式检查对象不为 null
2. **使用可选链**：`?.` 可以在对象为 null 时返回 undefined
3. **使用空值合并**：`??` 提供默认值
4. **避免非空断言**：除非确定对象不会为 null，否则不要使用 `!`
5. **类型注解**：对可能为 null 的变量显式声明联合类型

## 常见场景

### Display API

```typescript
// ❌ 错误
let display = display.getDefaultDisplaySync();
let width = display.width;

// ✅ 正确
let display = display.getDefaultDisplaySync();
if (display !== null) {
  let width = display.width;
}
```

### Window API

```typescript
// ❌ 错误
let window = window.getLastWindow(context);
window.setFullScreen(true);

// ✅ 正确
let window = await window.getLastWindow(context);
if (window !== null) {
  await window.setFullScreen(true);
}
```

### 可选属性

```typescript
// ❌ 错误
let config = { width: 100 };
let w = config.height; // height 不存在

// ✅ 正确
interface Config {
  width: number;
  height?: number;
}
let config: Config = { width: 100 };
if (config.height !== undefined) {
  let h = config.height;
}
```
