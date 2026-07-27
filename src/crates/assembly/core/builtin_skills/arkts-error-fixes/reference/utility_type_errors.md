# Utility Type Errors

## Error: Utility types not supported

### Error Message
```
Some of utility types are not supported (arkts-no-utility-types)
```

### Cause
ArkTS does not support TypeScript's utility types like `Parameters<T>`, `ReturnType<T>`, `Partial<T>`, etc. These utility types are part of TypeScript's advanced type system but are not compatible with ArkTS's stricter type system.

### Solution
Use explicit type definitions, interfaces, or type aliases instead of utility types. For function parameters, use `Object[]` or define explicit parameter types.

### Key Points
- ArkTS does not support TypeScript utility types
- Use explicit type definitions instead
- Use interfaces or type aliases for complex types
- Use `Object[]` for rest parameters
- Define function types explicitly

### ❌ Wrong Usage
```typescript
// ❌ Wrong: Using Parameters<T>
function debounce<T extends (...args: any[]) => void>(func: T, delay: number): T {
  return ((...args: Parameters<T>) => {
    setTimeout(() => func(...args), delay);
  }) as T;
}

// ❌ Wrong: Using ReturnType<T>
type Handler = () => string;
type Result = ReturnType<Handler>;

// ❌ Wrong: Using Partial<T>
interface Config {
  width: number;
  height: number;
  color?: string;
}
const partial: Partial<Config> = { width: 100 };
```

### ✅ Correct Usage
```typescript
// ✅ Correct: Using Object[] for rest parameters
function debounce(func: Function, delay: number): Function {
  return (...args: Object[]): void => {
    setTimeout(() => func(...args), delay);
  };
}

// ✅ Correct: Using explicit type
type Handler = () => string;
type Result = string;

// ✅ Correct: Using optional properties
interface Config {
  width: number;
  height: number;
  color?: string;
}
const partial: Config = { width: 100, height: 0, color: undefined };
```

### Function Type Definitions

#### Using Explicit Types
```typescript
// ✅ Correct: Explicit function type
private handleClick = (event: ClickEvent): void => {
  this.count++;
};

// ✅ Correct: Explicit return type
private getValue(): string {
  return 'Hello';
}
```

#### Using Interfaces
```typescript
// ✅ Correct: Interface for function type
interface EventHandler {
  (event: ClickEvent): void;
}

private handleClick: EventHandler = (event: ClickEvent): void => {
  console.info('Clicked');
};
```

#### Using Type Aliases
```typescript
// ✅ Correct: Type alias for function type
type ClickHandler = (event: ClickEvent) => void;

private handleClick: ClickHandler = (event: ClickEvent): void => {
  console.info('Clicked');
};
```

### Rest Parameters

#### ❌ Wrong
```typescript
// ❌ Wrong: Using Parameters<T>
function wrapper<T extends (...args: any[]) => void>(func: T): void {
  return (...args: Parameters<T>) => {
    func(...args);
  };
}
```

#### ✅ Correct
```typescript
// ✅ Correct: Using Object[]
function wrapper(func: Function): Function {
  return (...args: Object[]): void => {
    func(...args);
  };
}

// ✅ Correct: Using explicit types
function wrapper(func: (x: number, y: number) => void): (x: number, y: number) => void {
  return (x: number, y: number): void => {
    func(x, y);
  };
}
```

### Debounce Implementation

#### ❌ Wrong
```typescript
class DebounceUtil {
  private timeoutId: number = -1;

  debounce<T extends (...args: any[]) => void>(func: T, delay: number): T {
    return ((...args: Parameters<T>) => {
      clearTimeout(this.timeoutId);
      this.timeoutId = setTimeout(() => {
        func(...args);
      }, delay);
    }) as T;
  }
}
```

#### ✅ Correct
```typescript
class DebounceUtil {
  private timeoutId: number = -1;

  debounce(func: Function, delay: number): Function {
    return (...args: Object[]): void => {
      clearTimeout(this.timeoutId);
      this.timeoutId = setTimeout(() => {
        func(...args);
      }, delay);
    };
  }
}

// ✅ Correct: With explicit types
class DebounceUtil {
  private timeoutId: number = -1;

  debounce(func: (event: ClickEvent) => void, delay: number): (event: ClickEvent) => void {
    return (event: ClickEvent): void => {
      clearTimeout(this.timeoutId);
      this.timeoutId = setTimeout(() => {
        func(event);
      }, delay);
    };
  }
}
```

### Generic Functions

#### ❌ Wrong
```typescript
// ❌ Wrong: Using utility types
function wrap<T extends (...args: any[]) => R, R>(func: T): T {
  return func;
}
```

#### ✅ Correct
```typescript
// ✅ Correct: Using explicit types
function wrap(func: (x: number, y: number) => number): (x: number, y: number) => number {
  return func;
}

// ✅ Correct: Using interfaces
interface BinaryFunction {
  (x: number, y: number): number;
}

function wrap(func: BinaryFunction): BinaryFunction {
  return func;
}
```

### Common Patterns

#### Event Handlers
```typescript
// ✅ Correct: Event handler type
type ClickHandler = (event: ClickEvent) => void;
type TouchHandler = (event: TouchEvent) => void;
type ScrollHandler = (event: ScrollEvent) => void;

@Component
struct EventHandlers {
  private onClick: ClickHandler = (event: ClickEvent): void => {
    console.info('Clicked');
  };

  private onTouch: TouchHandler = (event: TouchEvent): void => {
    console.info('Touched');
  };

  build() {
    Column() {
      Button('Click')
        .onClick(this.onClick)
    }
    .onTouch(this.onTouch)
  }
}
```

#### Callback Functions
```typescript
// ✅ Correct: Callback type
interface SuccessCallback {
  (data: string): void;
}

interface ErrorCallback {
  (error: Error): void;
}

class DataFetcher {
  fetchData(success: SuccessCallback, error: ErrorCallback): void {
    try {
      success('Data loaded');
    } catch (e) {
      error(e as Error);
    }
  }
}
```

#### Utility Functions
```typescript
// ✅ Correct: Utility function types
type Mapper<T, R> = (item: T, index: number) => R;
type Filter<T> = (item: T, index: number) => boolean;
type Reducer<T, R> = (acc: R, item: T, index: number) => R;

class ArrayUtils {
  static map<T, R>(array: T[], mapper: Mapper<T, R>): R[] {
    return array.map(mapper);
  }

  static filter<T>(array: T[], filter: Filter<T>): T[] {
    return array.filter(filter);
  }

  static reduce<T, R>(array: T[], reducer: Reducer<T, R>, initial: R): R {
    return array.reduce(reducer, initial);
  }
}
```

### Best Practices
1. **Use explicit types**: Always define function types explicitly
2. **Use interfaces**: Define interfaces for complex function types
3. **Use type aliases**: Create type aliases for reusable function types
4. **Avoid utility types**: Don't use TypeScript utility types
5. **Use Object[]**: Use `Object[]` for rest parameters

### Related Files
- [Code Example](../assets/UtilityTypeError.ets)
- [ArkTS Type System](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-type-system)
