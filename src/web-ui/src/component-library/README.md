[中文](README.zh-CN.md) | **English**

# BitFun Component Library

## Overview

This is the BitFun component preview system for quickly viewing and testing UI components.

## Quick Start

### Start the preview server

```bash
pnpm run preview-components
```

Starts a dev server and opens the preview page at `http://localhost:3000`.

### Build the preview site

```bash
pnpm run build-components
```

Build output is written to `dist-preview`.

## Directory Structure

```
src/component-library/
├── components/          # Component definitions
│   ├── index.ts        # Component exports
│   └── registry.tsx    # Component registry
├── preview/            # Preview system
│   ├── PreviewApp.tsx  # Preview app
│   ├── preview.css     # Preview styles
│   └── main.tsx        # Preview entry
├── types/              # Type definitions
│   └── index.ts
└── index.ts            # Component library entry
```

## Adding a New Component

### 1. Register in `registry.tsx`

```tsx
import { YourNewComponent } from '@/shared/ui/YourNewComponent';

export const componentRegistry: ComponentCategory[] = [
  {
    id: 'your-category',
    name: 'Your Category',
    description: 'Category description',
    components: [
      {
        id: 'your-component-id',
        name: 'YourComponent',
        description: 'Component description',
        category: 'your-category',
        component: () => <YourNewComponent prop1="value" />,
      },
    ],
  },
];
```

### 2. Optional export in `components/index.ts`

```tsx
export { YourNewComponent } from '@/shared/ui/YourNewComponent';
```

### 3. Preview it

Run `pnpm run preview-components` and the component will appear in the preview app.

## Component Categories

Currently supported:

- **Basic Components** - Buttons, inputs, and other core UI elements
- **Layout Components** - Cards, panels, and layout containers
- **Feedback Components** - Loading states, modals, and status indicators

Add more categories as needed.

## Custom Styles

Preview styles live in `preview/preview.css`.

## Usage Notes

1. **Component Development** - Test states and props in the preview system
2. **Visual Checks** - Use the preview app for visual regression checks
3. **Docs Reference** - Use previews as usage examples

## Tech Stack

- React 19
- TypeScript
- Vite
- CSS Modules

## Notes

- The preview system uses a separate Vite config: `src/web-ui/vite.config.preview.ts`
- The preview entry is `preview.html`
- The preview system does not affect the main app build