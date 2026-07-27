# 实现不允许错误

## 错误描述

在 ArkTS 中，UI 组件必须使用 `@Component` 装饰器装饰，并且必须在 `build()` 方法中返回 UI 结构。直接在文件中编写 UI 组件而不使用 `@Component` 装饰器会导致"实现不允许"错误。

## 错误示例

```typescript
// 错误：直接在文件中编写 UI 组件
Row() {
  Text('Hello')
}
```

**错误信息：**
```
Implementation not allowed
```

## 解决方案

### 方案一：使用 @Component 装饰器

将 UI 组件包装在 `@Component` 装饰器中，并添加 `build()` 方法。

```typescript
@Component
struct MyComponent {
  build() {
    Row() {
      Text('Hello')
    }
  }
}
```

### 方案二：使用 @Entry 装饰器（如果是页面入口）

如果是页面入口组件，使用 `@Entry` 装饰器。

```typescript
@Entry
@Component
struct MyPage {
  build() {
    Row() {
      Text('Hello')
    }
  }
}
```

## 简单示例

```typescript
@Entry
@Component
struct MyPage {
  @State count: number = 0;

  build() {
    Column() {
      Text(`计数: ${this.count}`)
        .fontSize(20)
        .margin({ bottom: 16 })
      
      Button('增加')
        .onClick(() => {
          this.count++;
        })
    }
    .padding(16)
  }
}
```

## 详细代码示例

- [ImplementationNotAllowedError.ets](../assets/ImplementationNotAllowedError.ets) - 完整的实现不允许错误修复示例，包含正确的组件结构

## 最佳实践

1. **使用 @Component 装饰器**：所有自定义组件都必须使用 `@Component` 装饰器
2. **使用 @Entry 装饰器**：页面入口组件必须使用 `@Entry` 装饰器
3. **实现 build() 方法**：所有组件都必须实现 `build()` 方法并返回 UI 结构
4. **遵循组件结构**：UI 组件必须遵循 ArkTS 的组件结构规范

## 常见错误

```typescript
// ❌ 错误：直接在文件中编写 UI 组件
Row() {
  Text('Hello')
}

// ❌ 错误：缺少 @Component 装饰器
struct MyComponent {
  build() {
    Row() {
      Text('Hello')
    }
  }
}

// ❌ 错误：缺少 build() 方法
@Component
struct MyComponent {
  // 缺少 build() 方法
}

// ✅ 正确：使用 @Component 装饰器和 build() 方法
@Component
struct MyComponent {
  build() {
    Row() {
      Text('Hello')
    }
  }
}

// ✅ 正确：页面入口使用 @Entry 装饰器
@Entry
@Component
struct MyPage {
  build() {
    Row() {
      Text('Hello')
    }
  }
}
```

## 组件结构规范

### 基本组件结构

```typescript
@Component
struct MyComponent {
  // 状态变量
  @State count: number = 0;
  
  // 私有属性
  private scroller: Scroller = new Scroller();
  
  // 生命周期方法
  aboutToAppear() {
    // 组件即将出现时调用
  }
  
  aboutToDisappear() {
    // 组件即将消失时调用
  }
  
  // 自定义方法
  private handleClick() {
    this.count++;
  }
  
  // UI 构建方法
  build() {
    Column() {
      Text('Hello')
    }
  }
}
```

### 页面入口组件结构

```typescript
@Entry
@Component
struct MyPage {
  // 状态变量
  @State title: string = 'My Page';
  
  // 生命周期方法
  aboutToAppear() {
    // 页面即将出现时调用
  }
  
  // UI 构建方法
  build() {
    Column() {
      Text(this.title)
    }
  }
}
```
