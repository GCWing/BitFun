[中文](README.zh-CN.md) | **English**

# FlowChat Cards

FlowChat card components for rendering tool execution progress, summaries, and structured results in the chat stream.

## Scope

This directory contains:

- shared card layout in `BaseToolCard`
- tool-specific cards such as `ReadFileCard`, `SearchCard`, and `TaskCard`
- shared registration and configuration in [index.ts](index.ts)
- shared styles in `_shared-styles.scss`

## Current cards

Current component folders in this directory:

- `BaseToolCard`: shared layout, header, status, and result sections
- `ContextCompressionCard`: context compression summary and result display
- `ReadFileCard`: file read output and content preview
- `SearchCard`: grep/glob-style search results
- `SnapshotCard`: snapshot-style structured output
- `TaskCard`: delegated task execution and result summaries
- `TodoCard`: todo list progress and status display
- `WebSearchCard`: web search results and source summaries

## Tool Config

[index.ts](index.ts) also maintains tool-level display configuration such as:

- display name and icon
- default display mode
- result display type
- primary color

The config currently covers cards and tool outputs for `Read`, `Write`, `Edit`, `Delete`, `Grep`, `Glob`, `WebSearch`, `WebFetch`, `Task`, `TodoWrite`, and `ContextCompression`.

## Usage

```tsx
import {
  ReadFileCard,
  SearchCard,
  TaskCard,
  getFlowChatCardConfig,
} from '@component-library/components/FlowChatCards';

const config = getFlowChatCardConfig('Read');

<ReadFileCard displayMode="compact" status="completed" />;
```

## Display modes

- `compact`: quick view in the chat stream
- `standard`: default card layout
- `detailed`: expanded input and result view
- `terminal`: terminal-oriented display for tool configs that need it

## Status Values

- `pending`
- `running`
- `streaming`
- `completed`
- `error`

## Extending

1. Add a component folder under `FlowChatCards/`
2. Export it in [index.ts](index.ts)
3. Register or update tool config in `FLOWCHAT_CARD_CONFIGS`
4. Keep README examples aligned with the exported cards and supported tool names

## Related

- [Component library](../README.md)
- [Styles](../../styles/README.md)
