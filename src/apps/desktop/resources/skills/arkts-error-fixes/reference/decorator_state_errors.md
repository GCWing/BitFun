# @State 装饰器错误

## 错误描述

在 ArkTS 中，`@State` 装饰器只能用于 `@Component` 修饰的 struct 中，不能用于普通 class。

## 错误示例

```typescript
// ❌ AI 经常生成的错误代码
class FoldableStateManager {
  @State currentBreakpoint: string = 'sm';
  @State isFolded: boolean = true;

  aboutToAppear() {
    // ...
  }
}
```

**错误信息：**
```
The '@State' decorator can only be used with 'struct'.
```

## 原因分析

`@State` 是 ArkUI 框架的状态管理装饰器，用于在组件内部管理状态。它只能与 `@Component` 一起使用，不能用于普通类。

## 解决方案

### 方案1：移除 @State 装饰器（推荐用于非组件类）

如果类不需要响应式状态管理，移除 `@State` 装饰器：

```typescript
// ✅ 正确的代码
class FoldableStateManager {
  currentBreakpoint: string = 'sm';
  isFolded: boolean = true;

  aboutToAppear() {
    // ...
  }
}
```

### 方案2：使用 @Component struct（推荐用于状态管理）

如果需要状态管理，将类改为 @Component struct：

```typescript
// ✅ 正确的代码
@Component
struct FoldableStateManager {
  @State currentBreakpoint: string = 'sm';
  @State isFolded: boolean = true;

  aboutToAppear() {
    // ...
  }

  build() {
    // UI 组件
  }
}
```

## 简单示例

### 用于工具类

```typescript
// ✅ 用于工具类时移除 @State
class DisplayManager {
  currentBreakpoint: string = 'sm';

  updateBreakpoint(width: number): string {
    if (width < 600) return 'sm';
    if (width < 840) return 'md';
    return 'lg';
  }
}
```

### 用于 @Component struct

```typescript
// ✅ 用于组件时保留 @State
@Component
struct AdaptiveLayout {
  @State currentBreakpoint: string = 'sm';

  build() {
    Column() {
      Text(`Current: ${this.currentBreakpoint}`)
    }
  }
}
```

## 详细代码示例

> [DecoratorStateError.ets](../assets/DecoratorStateError.ets) - 完整的 @State 装饰器错误示例和修复方案

## 相关文档

- [ArkTS 状态管理](./state_migration.md)
- [HarmonyOS 官方文档](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-state-management-overview)
