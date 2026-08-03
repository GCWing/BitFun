# 多个 @Entry 装饰器错误

## 问题描述

在 ArkTS 中，一个 `.ets` 文件只能有一个 `@Entry` 装饰器。如果在同一个文件中使用多个 `@Entry` 装饰器，会导致编译错误。

### 错误信息

```
Duplicate entry annotation
```

或

```
More than one @Entry decorator in the same file
```

## 错误示例

```typescript
// ❌ 错误：一个文件中有两个 @Entry
@Entry
@Component
struct FirstPage {
  build() {
    Column() {
      Text('Page 1')
    }
  }
}

@Entry
@Component
struct SecondPage {
  build() {
    Column() {
      Text('Page 2')
    }
  }
}
```

## 解决方案

### 方案一：将子组件改为普通 @Component（推荐）

将多余的 `@Entry` 改为普通 `@Component`，避免重复的入口声明。

```typescript
// ✅ 正确：只有一个 @Entry，其他用 @Component
@Entry
@Component
struct MainPage {
  build() {
    Column() {
      Text('Main Page')
      ChildComponent()
    }
  }
}

@Component
struct ChildComponent {
  build() {
    Text('Child Component')
  }
}
```

### 方案二：拆分到不同文件

将每个页面组件拆分到独立的文件中。

```typescript
// mainPage.ets
@Entry
@Component
struct MainPage {
  build() {
    Column() {
      Text('Main Page')
    }
  }
}
```

```typescript
// subPage.ets - 单独的文件
@Entry
@Component
struct SubPage {
  build() {
    Column() {
      Text('Sub Page')
    }
  }
}
```

### 方案三：使用状态管理实现多页面

如果需要在多个视图之间切换，可以使用 `@State` 和条件渲染：

```typescript
@Entry
@Component
struct MultiViewPage {
  @State currentView: number = 0;

  build() {
    Column() {
      if (this.currentView === 0) {
        ViewOne()
      } else {
        ViewTwo()
      }

      Row() {
        Button('View 1')
          .onClick(() => this.currentView = 0)
        Button('View 2')
          .onClick(() => this.currentView = 1)
      }
    }
  }
}

@Component
struct ViewOne {
  build() {
    Text('View 1').fontSize(24)
  }
}

@Component
struct ViewTwo {
  build() {
    Text('View 2').fontSize(24)
  }
}
```

## 简单示例

```typescript
@Entry
@Component
struct PageExample {
  @State message: string = 'Hello';

  build() {
    Column() {
      Text(this.message)
        .fontSize(24)

      // 使用 @Component 装饰的子组件
      ContentSection()
    }
    .width('100%')
    .height('100%')
    .padding(20)
  }
}

@Component
struct ContentSection {
  @State count: number = 0;

  build() {
    Column() {
      Text(`Count: ${this.count}`)
        .fontSize(18)

      Button('Add')
        .onClick(() => {
          this.count++;
        })
    }
  }
}
```

## 详细代码示例

- [DuplicateEntryError.ets](../assets/DuplicateEntryError.ets) - 完整的多个 @Entry 错误修复示例

## 最佳实践

1. **每个文件一个 @Entry**：保持每个 `.ets` 文件只有一个入口组件
2. **使用 @Component 复用 UI**：使用 `@Component` 装饰器创建可复用的 UI 组件
3. **合理拆分文件**：将不同页面拆分到独立文件，便于管理和维护
4. **状态提升**：如果多个组件需要共享状态，将状态提升到共同的父组件中
