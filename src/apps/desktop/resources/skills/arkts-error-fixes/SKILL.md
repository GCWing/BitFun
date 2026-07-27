---
name: arkts-error-fixes
description: Solutions for ArkTS compilation errors and type mismatches. Load this skill when compilation fails or when fixing ArkTS type errors.
---

# Harmony Error Fixes

This skill provides solutions for common ArkTS compilation errors and type mismatches encountered during HarmonyOS development.

## Error Categories

| Category | Description |
|-----------|-------------|
| Notification API Type Errors | ContentType type incompatibility |
| Window API Type Errors | Type inference issues with `window.getLastWindow` |
| AppStorage Type Errors | Type inference errors with `AppStorage.get()` |
| Object Spread Type Errors | Type inference issues with object spread |
| @StorageLink Default Value Errors | Missing default values for `@StorageLink` properties |
| Object Literal Interface Errors | Object literals without explicit interfaces |
| Object Literal Type Errors | Using object literal types in return type annotations |
| Function Return Type Errors | Limited return type inference |
| Arrow Function Conversion Errors | Using function expressions instead of arrow functions |
| Color Property Errors | Non-existent `Color` properties |
| Interface Method Signature Errors | Method signature mismatches in object literals |
| AvoidArea Type Errors | Missing `visible` property in `AvoidArea` type |
| Standalone Function `this` Errors | Using `this` in standalone functions |
| TitleButtonRect Type Errors | Incorrect return type for `getTitleButtonRect`; accessing non-existent properties (left, top) |
| Catch Clause Type Errors | Type annotations in catch clauses |
| ESObject Type Errors | Restricted usage of ESObject type |
| Resource Conversion Errors | Resource to string/number conversion errors |
| Unused Variable Warnings | Declared but never used variables |
| IDataSource Type Errors | LazyForEach requires IDataSource implementation |
| Duplicate Entry Errors | Multiple @Entry decorators in same file |
| Possibly Null Errors | Object possibly null when accessing properties |

## Quick Reference

| Error Type | Solution |
|------------|-----------|
| Notification type error | Cast to `number` type |
| Window type error | Use callback pattern for `getLastWindow` |
| AppStorage type error | Use `@StorageLink` with `LocalStorage` or `AppStorage.setAndLink` (avoid `setOrCreate`) |
| Object spread error | Explicitly type objects |
| @StorageLink default value error | Add `= undefined` or specific default value |
| Object literal interface error | Define interface before using object literal |
| Object literal type error | Define interface and use it as return type |
| Function return type error | Add explicit return type annotation |
| Arrow function conversion error | Convert `function` to arrow function `=>` |
| Color property error | Use hex color values instead of non-existent Color properties |
| Interface method signature error | Use property syntax `method: () => {}` instead of method syntax |
| AvoidArea type error | Add `visible: false` property to AvoidArea object |
| Standalone function `this` error | Pass context as parameter: `function foo(context: Context)` |
| TitleButtonRect type error | Use `window.TitleButtonRect` instead of `window.Rect`; only `width` and `height` properties available |
| Catch clause type error | Remove type annotation or use `any`/`unknown` |
| ESObject type error | Use `ESModule` or specific types instead of `ESObject` |
| Resource conversion error | Use Resource directly in UI components or use ResourceManager |
| Unused variable warning | Use console.info/hilog or delete unused variable |
| IDataSource type error | Implement IDataSource interface for LazyForEach |
| Duplicate Entry error | Remove extra @Entry, use @Component for child components |
| Possibly Null error | Use !== null check or optional chaining |

## Detailed Error Solutions

### Notification API Type Errors
- [Notification Type Error](./reference/notification_errors.md)
- [Code Example](./assets/NotificationError.ets)

### Window API Type Errors
- [Window Type Inference Error](./reference/window_type_errors.md)
- [Code Example](./assets/WindowTypeError.ets)

### AppStorage Type Errors
- [AppStorage Type Error](./reference/appstorage_errors.md)
- [Code Example](./assets/AppStorageError.ets)

### Object Spread Type Errors
- [Object Spread Type Error](./reference/object_spread_errors.md)
- [Code Example](./assets/ObjectSpreadError.ets)

### @StorageLink Default Value Errors
- [@StorageLink Default Value Error](./reference/storage_link_default_errors.md)
- [Code Example](./assets/StorageLinkDefaultError.ets)

### Object Literal Interface Errors
- [Object Literal Interface Error](./reference/object_literal_interface_errors.md)
- [Code Example](./assets/ObjectLiteralInterfaceError.ets)

### Object Literal Type Errors
- [Object Literal Type Error](./reference/object_literal_type_errors.md)
- [Code Example](./assets/ObjectLiteralTypeError.ets)

### Function Return Type Errors
- [Function Return Type Error](./reference/function_return_type_errors.md)
- [Code Example](./assets/FunctionReturnTypeError.ets)

### Arrow Function Conversion Errors
- [Arrow Function Conversion Error](./reference/arrow_function_conversion_errors.md)
- [Code Example](./assets/ArrowFunctionConversionError.ets)

### Color Property Errors
- [Color Property Error](./reference/color_property_errors.md)
- [Code Example](./assets/ColorPropertyError.ets)

### Interface Method Signature Errors
- [Interface Method Signature Error](./reference/interface_method_signature_errors.md)
- [Code Example](./assets/InterfaceMethodSignatureError.ets)

### AvoidArea Type Errors
- [AvoidArea Type Error](./reference/avoid_area_type_errors.md)
- [Code Example](./assets/AvoidAreaTypeError.ets)

### Standalone Function `this` Errors
- [Standalone Function `this` Error](./reference/standalone_function_errors.md)
- [Code Example](./assets/StandaloneFunctionError.ets)

### TitleButtonRect Type Errors
- [TitleButtonRect Type Error](./reference/title_button_rect_type_errors.md)
- [Code Example](./assets/TitleButtonRectTypeError.ets)

### Catch Clause Type Errors
- [Catch Clause Type Error](./reference/catch_clause_type_errors.md)
- [Code Example](./assets/CatchClauseTypeError.ets)

### ESObject Type Errors
- [ESObject Type Error](./reference/esobject_type_errors.md)
- [Code Example](./assets/ESObjectTypeError.ets)

### Resource Conversion Errors
- [Resource Conversion Error](./reference/resource_conversion_errors.md)
- [Code Example](./assets/ResourceConversionError.ets)

### Unused Variable Warnings
- [Unused Variable Warning](./reference/unused_variable_warnings.md)
- [Code Example](./assets/UnusedVariableWarning.ets)

### IDataSource Type Errors
- [IDataSource Type Error](./reference/idata_source_errors.md)
- [Code Example](./assets/IDataSourceError.ets)

### Duplicate Entry Errors
- [Duplicate Entry Error](./reference/duplicate_entry_errors.md)
- [Code Example](./assets/DuplicateEntryError.ets)

### Possibly Null Errors
- [Possibly Null Error](./reference/possibly_null_errors.md)
- [Code Example](./assets/PossiblyNullError.ets)

### Window Rect/Size Type Errors
- [Window Rect/Size Type Error](./reference/window_rect_size_errors.md)
- [Code Example](./assets/WindowRectSizeError.ets)
