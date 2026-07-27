# ArkUI Common Mistakes

Check this file before implementing ArkUI tabs, lists, state decorators, dialogs, navigation, or custom builders.

## Tabs and TabContent

Avoid: passing an object parameter with a builder callback into `TabContent`.

Correct:

```typescript
TabContent() {
  Column() {
    Text('首页')
  }
}
.tabBar('首页')
```

Rules:

- `Tabs` direct children should be `TabContent`.
- `TabContent` label content belongs in `.tabBar(...)`.
- Do not place unrelated components directly under `Tabs`.

## ForEach and LazyForEach

Avoid: a key generator callback that opens a block but does not return the key string.

Correct:

```typescript
ForEach(this.items, (item: ItemInfo) => {
  Text(item.title)
}, (item: ItemInfo) => item.id)
```

Rules:

- The key generator must return a stable string.
- Avoid index keys for business data because item order can change.
- `LazyForEach` requires a real lazy data source and should be placed in supported scroll containers.

## State decorators

Avoid: placing `@State` on top-level variables, local variables, plain classes, or component inputs.

Correct:

```typescript
@Component
struct MessagePanel {
  @State message: string = ''

  build() {
    Text(this.message)
  }
}
```

Rules:

- State decorators belong on component member declarations.
- `@State` is for component-local V1 state.
- `@Prop` is for parent-to-child input.
- `@Link` is for two-way binding in V1 components.
- `@Local` and `@Param` belong to V2 components.
- Do not mix V1 and V2 decorator families in one component.

## Component attributes

Avoid: shorthand modifier names borrowed from other UI systems.

Correct:

```typescript
Text('标题')
  .backgroundColor('#FFFFFF')
  .fontSize(16)
```

Rules:

- Use real ArkUI modifier names.
- Match modifiers to the component that owns them.
- Do not borrow CSS, Android, other UI framework, or web shorthand.

## Dialog buttons

Avoid: alert dialog button entries that use a label field named `text`.

Correct:

```typescript
AlertDialog.show({
  message: '确认删除？',
  primaryButton: { value: '确定', action: () => {} }
})
```

Rules:

- Alert dialog button label field is `value`.
- Keep cancel behavior limited to closing the dialog unless the requirement says otherwise.
- Follow the current project dialog or toast wrapper when one exists.

## UI visibility and interaction

Common failures:

- Required text is rendered under an overlay.
- Clickable text is used when the requirement requires `Button`.
- A tab label exists in code but is not visible on the tab bar.
- State changes happen in code but the updated text is not rendered.
- New UI is added outside the current launch page or reachable navigation path.

Guardrail:

- After writing UI, trace the visible path from the launch page to each required element and each required click result.
