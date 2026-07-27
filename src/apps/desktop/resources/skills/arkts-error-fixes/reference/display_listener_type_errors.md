# Display Listener Type Errors

## Error: Display on/off method type error

### Error Message
```
Type 'void' is not assignable to type 'number'
Argument of type 'number' is not assignable to parameter of type 'Callback<Rect>'
```

### Cause
The `display.on()` and `display.off()` methods are module-level methods, not instance methods of the `Display` class. Attempting to call them on a `Display` object instance causes type errors.

### Solution
Use `display.on()` and `display.off()` at the module level for listening to display changes. Use `Display.on()` and `Display.off()` on Display instances for instance-specific events like `availableAreaChange`.

### Key Points
- `display.on('add'|'remove'|'change', callback)` - Module level methods
- `Display.on('availableAreaChange', callback)` - Instance level methods
- `Display.on('foldStatusChange', callback)` - Instance level methods
- `Display.on('captureStatusChange', callback)` - Instance level methods

### Module Level Listeners
```typescript
import { display } from '@kit.ArkUI';

// ✅ Correct: Module level listener
let listener: (data: number) => void = (data: number) => {
  console.info(`Display changed, ID: ${data}`);
};

display.on('change', listener);

// Unregister
display.off('change', listener);
```

### Instance Level Listeners
```typescript
import { display } from '@kit.ArkUI';

const displayClass = display.getDefaultDisplaySync();

// ✅ Correct: Instance level listener
let areaListener: (data: display.Rect) => void = (data: display.Rect) => {
  console.info(`Available area changed: ${JSON.stringify(data)}`);
};

displayClass.on('availableAreaChange', areaListener);

// Unregister
displayClass.off('availableAreaChange', areaListener);
```

### ❌ Wrong Usage
```typescript
const displayClass = display.getDefaultDisplaySync();

// ❌ Wrong: Calling on() on Display instance
let listener = displayClass.on('change', () => {
  console.info('Display changed');
});

// ❌ Wrong: Calling off() on Display instance
displayClass.off('change', listener);
```

### ✅ Correct Usage
```typescript
// ✅ Correct: Module level for display changes
let changeListener: (data: number) => void = (data: number) => {
  console.info(`Display changed, ID: ${data}`);
};
display.on('change', changeListener);

// ✅ Correct: Instance level for available area changes
const displayClass = display.getDefaultDisplaySync();
let areaListener: (data: display.Rect) => void = (data: display.Rect) => {
  console.info(`Available area changed: ${JSON.stringify(data)}`);
};
displayClass.on('availableAreaChange', areaListener);
```

### Available Listener Types

#### Module Level (display module)
- `on('add', callback: Callback<number>)` - Display added
- `on('remove', callback: Callback<number>)` - Display removed
- `on('change', callback: Callback<number>)` - Display changed

#### Instance Level (Display object)
- `on('availableAreaChange', callback: Callback<Rect>)` - Available area changed
- `on('foldStatusChange', callback: Callback<FoldStatus>)` - Fold status changed
- `on('captureStatusChange', callback: Callback<boolean>)` - Capture status changed

### Best Practices
1. **Use module level** for display add/remove/change events
2. **Use instance level** for display-specific events
3. **Always unregister** listeners in aboutToDisappear()
4. **Store listener references** for proper cleanup
5. **Use explicit types** for listener callbacks

### Component Integration
```typescript
@Entry
@Component
struct DisplayListenerExample {
  private changeListener?: (data: number) => void;
  private areaListener?: (data: display.Rect) => void;

  aboutToAppear() {
    // Register module level listener
    this.changeListener = (data: number) => {
      console.info(`Display changed: ${data}`);
    };
    display.on('change', this.changeListener);

    // Register instance level listener
    const displayClass = display.getDefaultDisplaySync();
    this.areaListener = (data: display.Rect) => {
      console.info(`Area changed: ${JSON.stringify(data)}`);
    };
    displayClass.on('availableAreaChange', this.areaListener);
  }

  aboutToDisappear() {
    // Unregister listeners
    if (this.changeListener) {
      display.off('change', this.changeListener);
      this.changeListener = undefined;
    }
    if (this.areaListener) {
      const displayClass = display.getDefaultDisplaySync();
      displayClass.off('availableAreaChange', this.areaListener);
      this.areaListener = undefined;
    }
  }

  build() {
    Column() {
      Text('Display Listener Example')
    }
  }
}
```

### Related Files
- [Code Example](../assets/DisplayListenerTypeError.ets)
- [Display API Documentation](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-display)
