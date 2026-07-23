**中文** | [English](README.md)

# FlowChat Cards

用于在 FlowChat 聊天流中展示工具执行过程、摘要结果与结构化输出的卡片组件。

## 范围

本目录主要包含：

- `BaseToolCard` 中的通用卡片骨架
- `ReadFileCard`、`SearchCard`、`TaskCard` 等工具专用卡片
- [index.ts](index.ts) 中的统一导出与配置注册
- `_shared-styles.scss` 中的共享样式

## 当前卡片

本目录当前包含的组件有：

- `BaseToolCard`：通用布局、头部、状态与结果区
- `ContextCompressionCard`：上下文压缩结果展示
- `ReadFileCard`：文件读取结果与内容预览
- `SearchCard`：搜索类结果展示，适用于 grep/glob 风格输出
- `SnapshotCard`：快照类结构化输出展示
- `TaskCard`：任务委托执行与结果摘要展示
- `TodoCard`：Todo 列表进度与状态展示
- `WebSearchCard`：网页搜索结果与来源摘要展示

## 工具配置

[index.ts](index.ts) 同时维护工具级展示配置，包括：

- 展示名称与图标
- 默认显示模式
- 结果展示类型
- 主色

当前配置覆盖的工具输出包括 `Read`、`Write`、`Edit`、`Delete`、`Grep`、`Glob`、`WebSearch`、`WebFetch`、`Task`、`TodoWrite` 和 `ContextCompression`。

## 使用方式

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

## 显示模式

- `compact`：聊天流中的快速展示
- `standard`：默认卡片布局
- `detailed`：展开显示输入与结果
- `terminal`：适用于终端类工具配置的展示模式

## 状态值

- `pending`
- `running`
- `streaming`
- `completed`
- `error`

## 扩展开发

1. 在 `FlowChatCards/` 下新增组件目录
2. 在 [index.ts](index.ts) 中导出
3. 在 `FLOWCHAT_CARD_CONFIGS` 中注册或更新工具配置
4. 保持 README 示例与实际导出的卡片、支持的工具名称一致

## 相关文档

- [组件库总览](../README.md)
- [样式指南](../../styles/README.md)
