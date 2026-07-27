# BreakpointType 类型错误

## 错误描述

在使用 GridRow 组件时，直接使用 `BreakpointType` 类型会导致类型不匹配错误。`BreakpointType` 是一个枚举类型，不能直接与字符串类型进行类型转换。

## 错误示例

```typescript
import { mediaquery, UIContext } from '@kit.ArkUI';

@Entry
@Component
struct BreakpointTypeError {
  @State currentBreakpoint: string = 'sm';
  private smListener: mediaquery.MediaQueryListener | null = null;
  private mdListener: mediaquery.MediaQueryListener | null = null;
  private lgListener: mediaquery.MediaQueryListener | null = null;

  aboutToAppear() {
    const mediaQuery = this.getUIContext().getMediaQuery();
    
    this.smListener = mediaQuery.matchMediaSync('(0vp<=width<600vp)');
    this.mdListener = mediaQuery.matchMediaSync('(600vp<=width<840vp)');
    this.lgListener = mediaQuery.matchMediaSync('(840vp<=width)');
    
    const smCallback = (result: mediaquery.MediaQueryResult): void => {
      if (result.matches) {
        this.currentBreakpoint = 'sm';
      }
    };
    const mdCallback = (result: mediaquery.MediaQueryResult): void => {
      if (result.matches) {
        this.currentBreakpoint = 'md';
      }
    };
    const lgCallback = (result: mediaquery.MediaQueryResult): void => {
      if (result.matches) {
        this.currentBreakpoint = 'lg';
      }
    };
    
    if (this.smListener) {
      this.smListener.on('change', smCallback);
    }
    if (this.mdListener) {
      this.mdListener.on('change', mdCallback);
    }
    if (this.lgListener) {
      this.lgListener.on('change', lgCallback);
    }
  }

  aboutToDisappear() {
    if (this.smListener) {
      this.smListener.off('change');
    }
    if (this.mdListener) {
      this.mdListener.off('change');
    }
    if (this.lgListener) {
      this.lgListener.off('change');
    }
  }

  build() {
    Column() {
      Text(`当前断点: ${this.currentBreakpoint}`)
    }
  }
}
```

## 解决方案

### 方案一：使用字符串类型

直接使用字符串类型来表示断点，而不是使用 `BreakpointType` 枚举类型。

```typescript
@State currentBreakpoint: string = 'sm';
```

### 方案二：使用 SimpleBreakpointType

如果需要使用类型定义，可以使用 `SimpleBreakpointType` 类型别名。

```typescript
type SimpleBreakpointType = 'sm' | 'md' | 'lg';

@State currentBreakpoint: SimpleBreakpointType = 'sm';
```

## 简单示例

```typescript
@Entry
@Component
struct BreakpointExample {
  @State currentBreakpoint: string = 'sm';

  build() {
    if (this.currentBreakpoint === 'lg') {
      Row() {
        Text('lg断点布局')
      }
    } else if (this.currentBreakpoint === 'md') {
      Column() {
        Text('md断点布局')
      }
    } else {
      Column() {
        Text('sm断点布局')
      }
    }
  }
}
```

## 详细代码示例

- [BreakpointTypeError.ets](../assets/BreakpointTypeError.ets) - 完整的断点类型错误修复示例，包含媒体查询监听和断点判断逻辑
