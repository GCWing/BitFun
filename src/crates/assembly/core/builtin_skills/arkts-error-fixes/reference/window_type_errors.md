# Window API Type Errors

## Error: `window.getLastWindow` type inference issue

### Error Message
```
Type 'void & Promise<Window>' errors
```

### Cause
Using async/await pattern with `window.getLastWindow()` causes type inference issues in ArkTS. The function signature doesn't properly resolve to a Promise type when used with await.

### Solution
Use the callback pattern instead of async/await for `window.getLastWindow()`.

### Key Points
- Use callback pattern: `window.getLastWindow(context, (err, win) => { ... })`
- Always check error code: `if (err.code !== 0)`
- Handle errors gracefully with console logging
- Use explicit type annotation for size: `(size: window.Size) => void`

### Callback Pattern
```typescript
window.getLastWindow(context, (err, win) => {
  if (err.code !== 0) {
    console.error('Failed to get window:', err);
    return;
  }
  
  // Use window instance
  const properties = win.getWindowProperties();
});
```

### Window Properties
```typescript
interface WindowProperties {
  windowRect: Rect;      // Window position and size
  type: WindowType;        // Window type
  mode: WindowMode;         // Window mode
  brightness: number;       // Brightness (0.0-1.0)
  isPrivacyMode: boolean;   // Privacy mode status
  isFullScreen: boolean;    // Full screen status
  layoutMode: LayoutMode;   // Layout mode
}

interface Rect {
  left: number;   // Left position
  top: number;    // Top position
  width: number;  // Width
  height: number; // Height
}
```

### Window Size Change Event
```typescript
win.on('windowSizeChange', (size: window.Size) => {
  console.info(`New size: ${size.width}x${size.height}`);
});

interface Size {
  width: number;  // New width
  height: number; // New height
}
```

### Common Window Events
```typescript
win.on('windowSizeChange', (size: window.Size) => { });
win.on('systemBarTintChange', (region: Region) => { });
win.on('windowEvent', (data: WindowEvent) => { });
win.on('avoidAreaChange', (data: AvoidArea) => { });
```

### Error Handling
```typescript
// Check for specific error codes
if (err.code === 1300001) {
  // Invalid parameter
} else if (err.code === 1300002) {
  // Window not found
} else if (err.code === 1300003) {
  // Window operation failed
}
```

### Best Practices
1. **Always check error code**: Never assume success
2. **Log errors**: Use console.error for debugging
3. **Clean up listeners**: Remove event listeners in `aboutToDisappear()`
4. **Use type annotations**: Explicit types prevent inference issues
5. **Handle window state**: Account for window resize and orientation changes

### Related Files
- [Code Example](../assets/WindowTypeError.ets)
