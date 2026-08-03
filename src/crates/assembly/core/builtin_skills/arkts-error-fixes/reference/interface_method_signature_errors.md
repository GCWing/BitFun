# Interface Method Signature Errors

## Error: Interface method signature mismatch

### Error Message
```
Object literal must correspond to some explicitly declared class or interface
```

### Cause
When implementing an interface with an object literal, method signatures in the object must match the interface exactly. Using method syntax (`method() {}`) instead of property syntax (`method: () => {}`) causes type errors.

### Solution
Use property syntax for methods in object literals that implement interfaces. Change `method() {}` to `method: () => {}`.

### Key Points
- Use property syntax for methods in object literals
- Change `method(): ReturnType {}` to `method: () => ReturnType {}`
- Ensure parameter types match to interface
- Ensure return types match to interface

### Basic Pattern
```typescript
// ❌ Wrong: Method syntax in object literal
interface MyInterface {
  updateWindowInfo(): void;
  destroy(): void;
}

const myObject: MyInterface = {
  updateWindowInfo(): void {
    console.log('Updated');
  },
  destroy(): void {
    console.log('Destroyed');
  }
};

// ✅ Correct: Property syntax in object literal
interface MyInterface {
  updateWindowInfo: () => void;
  destroy: () => void;
}

const myObject: MyInterface = {
  updateWindowInfo: () => {
    console.log('Updated');
  },
  destroy: () => {
    console.log('Destroyed');
  }
};
```

### Common Patterns
```typescript
// No parameters
interface SimpleInterface {
  doSomething: () => void;
}

const obj: SimpleInterface = {
  doSomething: () => {
    console.log('Doing something');
  }
};

// With parameters
interface ParameterInterface {
  calculate: (a: number, b: number) => number;
}

const obj: ParameterInterface = {
  calculate: (a: number, b: number): number => {
    return a + b;
  }
};

// With optional parameters
interface OptionalInterface {
  process: (data: string, options?: Options) => void;
}

const obj: OptionalInterface = {
  process: (data: string, options?: Options): void => {
    console.log(data, options);
  }
};
```

### Detailed Examples
For more detailed code examples, see:
- [Interface Definition](../assets/InterfaceMethodSignatureError.ets#L1-L6)
- [Object Literal Implementation](../assets/InterfaceMethodSignatureError.ets#L8-L24)
- [Usage Pattern](../assets/InterfaceMethodSignatureError.ets#L26-L42)

### Best Practices
1. **Use property syntax**: Always use `method: () => {}` syntax for object literals
2. **Match signatures**: Ensure method signatures match to interface exactly
3. **Add type annotations**: Add explicit parameter and return types
4. **Keep interfaces simple**: Avoid overly complex method signatures
5. **Document interfaces**: Add comments explaining interface purpose

### Common Mistakes
```typescript
// ❌ Wrong: Method syntax in object literal
interface MyInterface {
  updateWindowInfo(): void;
}

const obj: MyInterface = {
  updateWindowInfo(): void {
    console.log('Updated');
  }
};

// ❌ Wrong: Method syntax in object literal
interface MyInterface {
  updateWindowInfo: () => void;
}

const obj: MyInterface = {
  updateWindowInfo(): void {
    console.log('Updated');
  }
};

// ✅ Correct: Property syntax in object literal
interface MyInterface {
  updateWindowInfo: () => void;
}

const obj: MyInterface = {
  updateWindowInfo: () => {
    console.log('Updated');
  }
};
```

### Related Files
- [Code Example](../assets/InterfaceMethodSignatureError.ets)
- [Object Literal Interface Errors](./object_literal_interface_errors.md)
- [ArkTS Language Guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-get-started)
