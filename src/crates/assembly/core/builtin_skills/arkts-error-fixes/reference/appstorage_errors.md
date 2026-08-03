# AppStorage Type Errors

## Error: `AppStorage.get()` type parameter issue

### Error Message
```
Type inference errors with `AppStorage.get()`
```

### Cause
`AppStorage.get()` requires explicit type parameters or should be replaced with `setOrCreate` for proper type inference. ArkTS has strict type checking for generic functions.

### Solution
Use `AppStorage.setOrCreate()` for initialization or provide explicit type parameters with `@StorageLink`.

### Key Points
- Use `@StorageLink('key')` decorator for type-safe state binding
- Initialize with `AppStorage.setOrCreate('key', value)` in `aboutToAppear()`
- The decorator automatically handles type inference
- State is synchronized across all components using the same key

### @StorageLink Pattern
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

### AppStorage Methods
```typescript
// Set or create a value
AppStorage.setOrCreate<T>(key: string, value: T): T

// Get a value (requires type parameter)
AppStorage.get<T>(key: string): T | undefined

// Set a value
AppStorage.set<T>(key: string, value: T): void

// Delete a value
AppStorage.delete(key: string): void

// Check if key exists
AppStorage.has(key: string): boolean
```

### State Decorators
```typescript
// Two-way binding with AppStorage
@StorageLink('key') value: Type = defaultValue;

// One-way binding with AppStorage
@StorageProp('key') value: Type = defaultValue;

// Two-way binding with LocalStorage
@LocalStorageLink('key') value: Type = defaultValue;

// One-way binding with LocalStorage
@LocalStorageProp('key') value: Type = defaultValue;
```

### Best Practices
1. **Use @StorageLink**: Prefer decorators over direct AppStorage access
2. **Initialize properly**: Always call `setOrCreate` in `aboutToAppear()`
3. **Use unique keys**: Ensure keys are unique across the application
4. **Type safety**: Let the decorator handle type inference
5. **Clean up**: Delete values when no longer needed

### Common Patterns
```typescript
// Counter pattern
@StorageLink('counter') count: number = 0;

// User data pattern
@StorageLink('userName') userName: string = '';

// Settings pattern
@StorageLink('isDarkMode') isDarkMode: boolean = false;

// Object pattern
@StorageLink('userSettings') settings: UserSettings = defaultSettings;
```

### Related Files
- [Code Example](../assets/AppStorageError.ets)
