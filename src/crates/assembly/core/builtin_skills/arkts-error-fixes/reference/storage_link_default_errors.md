# @StorageLink Default Value Errors

## Error: `The '@StorageLink' property must be specified a default value`

### Error Message
```
The '@StorageLink' property must be specified a default value
```

### Cause
ArkTS requires all `@StorageLink` decorated properties to have a default value. This is a strict type safety requirement to ensure the property always has a valid initial state.

### Solution
Add a default value to the `@StorageLink` property, typically `= undefined` for optional types or a specific default value for required types.

### Key Points
- Always provide a default value for `@StorageLink` properties
- Use `= undefined` for optional types
- Use specific default values for required types (e.g., `= 0`, `= ''`, `= false`)
- Initialize the actual value in `aboutToAppear()` using `AppStorage.setOrCreate()`

### Basic Pattern
```typescript
@Entry
@Component
struct MyComponent {
  @StorageLink('myKey') myValue: number = 0;

  aboutToAppear() {
    AppStorage.setOrCreate('myKey', 0);
  }

  build() {
    Text(`Value: ${this.myValue}`)
  }
}
```

### Optional Type Pattern
```typescript
interface WindowUtil {
  width: number;
  height: number;
  density: number;
  updateWindowInfo: () => void;
  destroy: () => void;
}

@Entry
@Component
struct MyComponent {
  @StorageLink('windowUtil') windowUtil?: WindowUtil = undefined;

  aboutToAppear() {
    AppStorage.setOrCreate('windowUtil', {
      width: 0,
      height: 0,
      density: 1.0,
      updateWindowInfo: () => {},
      destroy: () => {}
    });
  }

  build() {
    Text(`Width: ${this.windowUtil?.width || 0}`)
  }
}
```

### Common Default Values
```typescript
// Number types
@StorageLink('counter') count: number = 0;
@StorageLink('width') width: number = 100;
@StorageLink('opacity') opacity: number = 1.0;

// String types
@StorageLink('userName') userName: string = '';
@StorageLink('title') title: string = 'Default Title';

// Boolean types
@StorageLink('isDarkMode') isDarkMode: boolean = false;
@StorageLink('isLoading') isLoading: boolean = true;

// Array types
@StorageLink('items') items: Array<string> = [];
@StorageLink('numbers') numbers: number[] = [1, 2, 3];

// Optional types
@StorageLink('user') user?: User = undefined;
@StorageLink('settings') settings?: Settings = undefined;
```

### Detailed Examples
For more detailed code examples, see:
- [Optional Type Pattern](../assets/StorageLinkDefaultError.ets#L8-L31)
- [Initialization Pattern](../assets/StorageLinkDefaultError.ets#L11-L23)
- [Usage Pattern](../assets/StorageLinkDefaultError.ets#L25-L39)

### Best Practices
1. **Always provide default value**: Never leave `@StorageLink` without a default
2. **Use appropriate defaults**: Choose defaults that make sense for your use case
3. **Initialize in aboutToAppear**: Set the actual value when component appears
4. **Use optional types carefully**: Only use `?` when the value can legitimately be undefined
5. **Document default values**: Add comments explaining why a specific default was chosen

### Common Mistakes
```typescript
// ❌ Wrong: No default value
@StorageLink('myValue') myValue: number;

// ❌ Wrong: Using null instead of undefined
@StorageLink('myValue') myValue: number = null;

// ✅ Correct: With default value
@StorageLink('myValue') myValue: number = 0;

// ✅ Correct: Optional type with undefined
@StorageLink('myValue') myValue?: number = undefined;
```

### Related Files
- [Code Example](../assets/StorageLinkDefaultError.ets)
- [AppStorage Type Errors](./appstorage_errors.md)
