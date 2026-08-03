# Color Property Errors

## Error: `Property 'XXX' does not exist on type 'typeof Color'`

### Error Message
```
Property 'LightBlue' does not exist on type 'typeof Color'
```

### Cause
The `Color` class in ArkTS does not have all of the color properties that might be expected. Some color names like `LightBlue`, `DarkGray`, etc. are not available. Using these properties will cause compilation errors.

### Solution
Use hex color values (`#RRGGBB` or `#AARRGGBB`) instead of non-existent `Color` properties.

### Key Points
- Use hex color values instead of non-existent Color properties
- Hex format: `#RRGGBB` for RGB, `#AARRGGBB` for ARGB
- Common colors: `#FF0000` (red), `#00FF00` (green), `#0000FF` (blue)
- Use layered parameters for theme-aware colors

### Basic Pattern
```typescript
// ❌ Wrong: Using non-existent Color property
Text('Hello')
  .fontColor(Color.LightBlue)

// ✅ Correct: Using hex color value
Text('Hello')
  .fontColor('#ADD8E6')
```

### Common Color Replacements
```typescript
// Light colors
Color.LightBlue  -> '#ADD8E6'
Color.LightGreen -> '#90EE90'
Color.LightGray  -> '#D3D3D3'
Color.LightCyan  -> '#E0FFFF'

// Dark colors
Color.DarkBlue   -> '#00008B'
Color.DarkGreen  -> '#006400'
Color.DarkGray   -> '#A9A9A9'
Color.DarkCyan   -> '#008B8B'

// Other colors
Color.Pink       -> '#FFC0CB'
Color.Orange     -> '#FFA500'
Color.Purple     -> '#800080'
Color.Brown      -> '#A52A2A'
Color.Gold       -> '#FFD700'
Color.Silver     -> '#C0C0C0'
```

### Common Hex Colors
```typescript
// Primary colors
'#FF0000'  // Red
'#00FF00'  // Green
'#0000FF'  // Blue

// Secondary colors
'#FFFF00'  // Yellow
'#FF00FF'  // Magenta
'#00FFFF'  // Cyan

// Grayscale
'#FFFFFF'  // White
'#F5F5F5'  // Light gray
'#CCCCCC'  // Medium gray
'#999999'  // Dark gray
'#666666'  // Darker gray
'#333333'  // Very dark gray
'#000000'  // Black

// UI colors
'#007DFF'  // Primary blue
'#FF6B00'  // Orange
'#00C853'  // Green
'#FF1744'  // Red
'#651FFF'  // Purple
'#00B0FF'  // Light blue
```

### Color Properties
```typescript
// Text color
.fontColor('#FF0000')

// Background color
.backgroundColor('#00FF00')

// Border color
.borderColor('#0000FF')

// Shadow color
.shadow({ radius: 10, color: '#FF0000' })
```

### Detailed Examples
For more detailed code examples, see:
- [Basic Color Usage](../assets/ColorPropertyError.ets#L8-L12)
- [Text Color Pattern](../assets/ColorPropertyError.ets#L14-L18)
- [Background Color Pattern](../assets/ColorPropertyError.ets#L20-L24)
- [Theme Switching](../assets/ColorPropertyError.ets#L26-L36)

### Best Practices
1. **Use hex values**: Always use hex color values instead of non-existent Color properties
2. **Define constants**: Create color constants for reuse
3. **Use resources**: Use resource references for theme-aware colors
4. **Document colors**: Add comments explaining color choices
5. **Test contrast**: Ensure text has sufficient contrast with background

### Common Mistakes
```typescript
// ❌ Wrong: Using non-existent Color property
Text('Hello')
  .fontColor(Color.LightBlue)

// ❌ Wrong: Using non-existent Color property
Text('Hello')
  .fontColor(Color.DarkGray)

// ✅ Correct: Using hex color value
Text('Hello')
  .fontColor('#ADD8E6')

// ✅ Correct: Using hex color value
Text('Hello')
  .fontColor('#A9A9A9')
```

### Related Files
- [Code Example](../assets/ColorPropertyError.ets)
- [ArkTS UI Components](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-uicomponent)
