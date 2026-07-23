[中文](README.zh-CN.md) | **English**

# Component Library

BitFun component library for reusable UI building blocks and previewable demos.

## Scope

This directory contains:

- reusable components exported through [index.ts](index.ts)
- preview registrations in [registry.tsx](registry.tsx)
- component-specific docs such as [FlowChatCards README](FlowChatCards/README.md)

## Component areas

Current component folders include:

- **Actions & window controls**: `Button`, `IconButton`, `WindowControls`
- **Dialogs & overlays**: `Modal`, `InputDialog`, `ConfirmDialog`, `Tooltip`
- **Form inputs**: `Input`, `NumberInput`, `Search`, `Select`, `Checkbox`, `Switch`, `Textarea`
- **Display & content**: `Alert`, `Badge`, `Tag`, `Avatar`, `Card`, `Empty`, `Markdown`, `Tabs`
- **Editors & text**: `CodeEditor`, `StreamText`
- **Visual utilities**: `CubeLoading`, `CubeLogo`, `DotMatrixLoader`, `TextStrokeEffect`
- **Feature-specific UI**: `ConfigPage`, `FilterPill`, `FlowChatCards`

## Exports and Previews

- Add reusable exports in [index.ts](index.ts)
- Register preview entries in [registry.tsx](registry.tsx)
- Keep README examples aligned with actual exported components

## Usage

```tsx
import { Button, Card, Input, Markdown } from '@components';

function Example() {
  return (
    <Card>
      <Input placeholder="Search project files" />
      <Button variant="primary">Run</Button>
      <Markdown># Preview</Markdown>
    </Card>
  );
}
```

## Development

1. Create a component folder under `components/`
2. Implement the component and related styles
3. Export it through the component `index.ts` and `components/index.ts`
4. Add or update preview demos in `registry.tsx`
5. Update the nearest README when the public surface changes

## Notes

- Use existing SCSS patterns and naming conventions from nearby components
- Treat [registry.tsx](registry.tsx) as the source of truth for preview coverage
- If a component group grows large, add a local README in that subdirectory
