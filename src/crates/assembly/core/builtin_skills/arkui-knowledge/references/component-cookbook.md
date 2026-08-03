# ArkUI Component Cookbook

This file keeps high-frequency ArkUI patterns close to the `arkui-knowledge` skill. Prefer project-local code style, but use these shapes to avoid common API mistakes.

## Tabs and TabContent

Use `Tabs` for tabbed pages. Direct children should be `TabContent`.

```typescript
Tabs({ barPosition: BarPosition.End }) {
  TabContent() {
    Column() {
      Text('首页')
    }
  }
  .tabBar('首页')

  TabContent() {
    Column() {
      Text('发现')
    }
  }
  .tabBar('发现')
}
```

Key rules:

- `TabContent()` has no object parameter.
- Put tab label content in `.tabBar(...)`.
- Keep each tab body inside the `TabContent` block.
- Use `.onChange((index: number) => { ... })` when selected tab state must refresh other UI.

## List, Grid, and ForEach

Use `ForEach` for ordinary in-memory arrays and small lists.

```typescript
interface CardItem {
  id: string
  title: string
}

@State cards: CardItem[] = [
  { id: 'card-1', title: '卡片 1' },
  { id: 'card-2', title: '卡片 2' }
]

Grid() {
  ForEach(this.cards, (item: CardItem) => {
    GridItem() {
      Text(item.title)
    }
  }, (item: CardItem) => item.id)
}
```

Key rules:

- The key function returns a stable string.
- Do not use a key callback that returns no value.
- Use `ListItem` inside `List`.
- Use `GridItem` inside `Grid`.
- Use `LazyForEach` only with a proper data source when lazy rendering is needed.

## TextInput, Button, and state refresh

Use state variables for user input and validation messages.

```typescript
@State userName: string = ''
@State errorText: string = ''

Column({ space: 12 }) {
  TextInput({ placeholder: '请输入用户名', text: this.userName })
    .onChange((value: string) => {
      this.userName = value
    })

  Button('注册')
    .onClick(() => {
      this.errorText = this.userName.length === 0 ? '请填写完整信息' : '注册成功'
    })

  Text(this.errorText)
    .fontColor(this.errorText === '注册成功' ? Color.Green : Color.Red)
}
```

Key rules:

- State that drives visible UI should use the existing component decorator family.
- `TextInput` changes should update state in `.onChange(...)`.
- Required validation messages must be rendered in the page, not only logged.

## Dialog and Toast

For simple confirmation, use alert dialog APIs already used by the project. Button entries use `value` for the visible label.

```typescript
this.getUIContext().showAlertDialog({
  title: '确认进入订座？',
  message: '确认进入订座？',
  primaryButton: {
    value: '去订座',
    action: () => {
      this.getUIContext().getPromptAction().showToast({ message: '已打开订座' })
    }
  },
  secondaryButton: {
    value: '再想想',
    action: () => {}
  }
})
```

Key rules:

- Use `value` for alert dialog button text.
- Do not invent `text` fields for alert dialog buttons.
- Prefer the project's current dialog/toast pattern when it already exists.
- Keep cancel actions side-effect free when the requirement says cancel only closes the dialog.

## Navigation and NavDestination

Follow the existing navigation architecture. Do not replace project routing only to add one page.

For projects already using `Navigation`, register or route to `NavDestination` through the existing `NavPathStack` or project router wrapper. For projects using a custom app router, follow that wrapper and existing page registration pattern.

Key rules:

- Preserve the existing navigation mechanism.
- Keep page names and builder exports consistent with nearby pages.
- Pass only the parameters required by the target page.
- Verify the first visible screen and the new navigation path.
