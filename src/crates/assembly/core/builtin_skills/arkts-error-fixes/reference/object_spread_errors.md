# Object Spread Type Errors

## Error: Object spread type inference issue

### Error Message
```
Type inference errors when using object spread
```

### Cause
ArkTS has strict type inference rules for object spread operations. Without explicit type annotations, the compiler cannot properly infer the resulting type.

### Solution
Explicitly type objects or use proper interface definitions before using spread operations.

### Key Points
- Define interfaces for object shapes
- Use explicit type annotations for spread results
- Ensure spread and target types are compatible
- Use optional properties for partial updates

### Interface Definition
```typescript
interface Config {
  width: number;
  height: number;
  color?: string;
  opacity?: number;
}
```

### Spread Pattern
```typescript
const baseConfig: Config = { width: 100, height: 100 };
const newConfig: Config = { ...baseConfig, color: 'red' };
```

### Partial Updates
```typescript
interface User {
  id: number;
  name: string;
  email?: string;
  phone?: string;
}

const baseUser: User = { id: 1, name: 'John' };
const updatedUser: User = { 
  ...baseUser, 
  email: 'john@example.com' 
};
```

### Nested Objects
```typescript
interface NestedConfig {
  layout: {
    width: number;
    height: number;
  };
  style: {
    color: string;
    opacity: number;
  };
}

const base: NestedConfig = {
  layout: { width: 100, height: 100 },
  style: { color: 'red', opacity: 1.0 }
};

const updated: NestedConfig = {
  ...base,
  layout: { ...base.layout, width: 200 }
};
```

### Common Patterns
```typescript
// Configuration update
const config: Config = { ...defaultConfig, ...userConfig };

// State update
const newState: State = { ...currentState, ...updates };

// Merge defaults
const merged: Options = { ...defaultOptions, ...userOptions };

// Conditional spread
const result: Result = {
  ...base,
  ...(condition ? { extra: 'value' } : {})
};
```

### Best Practices
1. **Define interfaces**: Always define interfaces for object shapes
2. **Use explicit types**: Annotate spread results with interface types
3. **Optional properties**: Use optional properties for partial updates
4. **Type compatibility**: Ensure spread and target types are compatible
5. **Avoid deep spread**: Be careful with nested object spreads

### Related Files
- [Code Example](../assets/ObjectSpreadError.ets)
- [ArkTS Language Guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-get-started)
