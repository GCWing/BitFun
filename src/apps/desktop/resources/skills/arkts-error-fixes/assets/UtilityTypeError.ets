/**
 * ❌ 错误示例：使用 Parameters 工具类型
 * 
 * 错误原因：ArkTS 不支持 TypeScript 的 Parameters 工具类型
 * 错误信息：Some of utility types are not supported (arkts-no-utility-types)
 */
@Entry
@Component
struct UtilityTypeError {
  @State count: number = 0;

  // ❌ 错误：使用 Parameters 工具类型
  private handleClick = (event: ClickEvent) => {
    this.count++;
  };

  // ❌ 错误：使用 Parameters 工具类型
  private debounce<T extends (...args: any[]) => void>(func: T, delay: number): T {
    return ((...args: Parameters<T>) => {
      setTimeout(() => {
        func(...args);
      }, delay);
    }) as T;
  }

  build() {
    Column() {
      Text(`Count: ${this.count}`)
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Click Me')
        .onClick(this.handleClick)
    }
  }
}

/**
 * ✅ 正确示例：使用 Object[] 替代 Parameters 工具类型
 */
@Entry
@Component
struct UtilityTypeCorrect {
  @State count: number = 0;

  private handleClick = (event: ClickEvent) => {
    this.count++;
  };

  // ✅ 正确：使用 Object[] 替代 Parameters<T>
  private debounce(func: Function, delay: number): Function {
    return (...args: Object[]): void => {
      setTimeout(() => {
        func(...args);
      }, delay);
    };
  }

  build() {
    Column() {
      Text(`Count: ${this.count}`)
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Click Me')
        .onClick(this.handleClick)
    }
  }
}

/**
 * ✅ 正确示例：使用明确的类型定义
 */
@Entry
@Component
struct UtilityTypeExplicit {
  @State message: string = 'Hello';

  // ✅ 正确：使用明确的类型定义
  private handleClick = (event: ClickEvent): void => {
    this.message = 'Clicked!';
  };

  // ✅ 正确：使用明确的类型定义
  private handleInput = (value: string): void => {
    this.message = value;
  };

  // ✅ 正确：使用明确的类型定义
  private debounce(func: (event: ClickEvent) => void, delay: number): (event: ClickEvent) => void {
    let timeoutId: number = -1;
    return (event: ClickEvent): void => {
      clearTimeout(timeoutId);
      timeoutId = setTimeout(() => {
        func(event);
      }, delay);
    };
  }

  build() {
    Column() {
      Text(this.message)
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Click Me')
        .onClick(this.debounce(this.handleClick, 300))
    }
  }
}

/**
 * ✅ 正确示例：使用类型别名定义函数类型
 */
type EventHandler = (event: ClickEvent) => void;
type InputHandler = (value: string) => void;

@Entry
@Component
struct UtilityTypeInterface {
  @State message: string = 'Hello';

  // ✅ 正确：使用类型别名定义函数类型
  private handleClick: EventHandler = (event: ClickEvent): void => {
    this.message = 'Clicked!';
  };

  // ✅ 正确：使用类型别名定义函数类型
  private handleInput: InputHandler = (value: string): void => {
    this.message = value;
  };

  build() {
    Column() {
      Text(this.message)
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Click Me')
        .onClick(this.handleClick)
    }
  }
}

/**
 * ✅ 正确示例：使用类型别名
 */
type ClickHandler = (event: ClickEvent) => void;
type ValueHandler = (value: string) => void;

@Entry
@Component
struct UtilityTypeAlias {
  @State message: string = 'Hello';

  // ✅ 正确：使用类型别名
  private handleClick: ClickHandler = (event: ClickEvent): void => {
    this.message = 'Clicked!';
  };

  // ✅ 正确：使用类型别名
  private handleInput: ValueHandler = (value: string): void => {
    this.message = value;
  };

  build() {
    Column() {
      Text(this.message)
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Click Me')
        .onClick(this.handleClick)
    }
  }
}
