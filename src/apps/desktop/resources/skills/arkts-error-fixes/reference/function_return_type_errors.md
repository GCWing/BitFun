# Function Return Type Errors

## Error: `Object literals cannot be used as type declarations`

### Error Message
```
Object literals cannot be used as type declarations (arkts-no-obj-literals-as-types)
```

### Cause
ArkTS does not allow object literal types (like `{ width: number; height: number }`) to be used directly as return type annotations. You must define an interface or type alias first.

### Solution
Define an interface for the object shape, then use that interface as the return type.

### Basic Pattern
```typescript
// ❌ Wrong: Object literal type in return annotation
private getWindowInfo(): { width: number; height: number } {
  return {
    width: 1080,
    height: 2340
  };
}

// ✅ Correct: Define interface first
interface WindowInfo {
  width: number;
  height: number;
}

private getWindowInfo(): WindowInfo {
  return {
    width: 1080,
    height: 2340
  };
}
```

### Simple Example
```typescript
interface WindowInfo {
  width: number;
  height: number;
}

@Entry
@Component
struct WindowInfoExample {
  @State windowWidth: number = 0;
  @State windowHeight: number = 0;

  private getWindowInfo(): WindowInfo {
    return {
      width: 1080,
      height: 2340
    };
  }

  build() {
    Column() {
      Text(`Window: ${this.windowWidth} x ${this.windowHeight}`)
    }
  }
}
```

### Detailed Code Examples
- [FunctionReturnTypeError.ets](../assets/FunctionReturnTypeError.ets) - 完整的函数返回类型示例，包含接口定义和使用

---

## Error: `Function return type inference is limited`

### Error Message
```
Function return type inference is limited
```

### Cause
ArkTS has limited type inference for function return types, especially for complex types or when the return type cannot be easily inferred from the function body. This is a strict type safety requirement to ensure code clarity and prevent type errors.

### Solution
Add explicit return type annotations to functions that return complex types or when the compiler cannot infer the return type.

### Key Points
- Add explicit return type annotations for complex return types
- Use interface or type aliases for object return types
- Annotate functions that return arrays, objects, or unions
- Keep return type annotations simple and clear

### Basic Pattern
```typescript
// ❌ Wrong: No explicit return type
private getWindowInfo() {
  return {
    width: 1080,
    height: 2340
  };
}

// ✅ Correct: With explicit return type
private getWindowInfo(): { width: number; height: number } {
  return {
    width: 1080,
    height: 2340
  };
}
```

### Common Patterns
```typescript
// Object return types
private getConfig(): Config {
  return { width: 100, height: 100, color: 'red' };
}

// Array return types
interface Item {
  id: number;
  name: string;
}

private getItems(): Item[] {
  return [
    { id: 1, name: 'Item 1' },
    { id: 2, name: 'Item 2' }
  ];
}

// Union return types
private getValue(): string | number {
  if (Math.random() > 0.5) {
    return 'string';
  }
  return 42;
}

// Optional return types
private findUser(id: number): User | undefined {
  return this.users.find(user => user.id === id);
}

// Promise return types
private async fetchData(): Promise<Data> {
  const response = await fetch('https://api.example.com/data');
  return await response.json();
}
```

### Detailed Examples
For more detailed code examples, see:
- [Object Return Type](../assets/FunctionReturnTypeError.ets#L14-L19)
- [Void Return Type](../assets/FunctionReturnTypeError.ets#L21-L26)
- [Usage Pattern](../assets/FunctionReturnTypeError.ets#L28-L35)

### Best Practices
1. **Add explicit types**: Always add return type annotations for complex functions
2. **Use interfaces**: Define interfaces for object return types
3. **Keep types simple**: Avoid overly complex return type expressions
4. **Be consistent**: Use the same style throughout your codebase
5. **Document types**: Add comments explaining complex return types

### Common Mistakes
```typescript
// ❌ Wrong: No explicit return type
private getConfig() {
  return { width: 100, height: 100 };
}

// ❌ Wrong: Using 'any' type
private getConfig(): any {
  return { width: 100, height: 100 };
}

// ✅ Correct: With explicit return type
private getConfig(): { width: number; height: number } {
  return { width: 100, height: 100 };
}

// ✅ Correct: Using interface
interface Config {
  width: number;
  height: number;
}

private getConfig(): Config {
  return { width: 100, height: 100 };
}
```

### Related Files
- [Code Example](../assets/FunctionReturnTypeError.ets)
- [ArkTS Language Guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-get-started)
