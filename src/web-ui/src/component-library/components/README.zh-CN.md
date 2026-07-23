**中文** | [English](README.md)

# 组件库

BitFun 组件库目录，承载可复用 UI 组件与可预览的演示注册。

## 范围

本目录主要包含：

- 通过 [index.ts](index.ts) 对外导出的可复用组件
- 在 [registry.tsx](registry.tsx) 中注册的组件预览
- 组件分组自己的说明文档，例如 [FlowChatCards README](FlowChatCards/README.md)

## 组件分区

当前组件目录主要包括：

- **操作与窗口控件**：`Button`、`IconButton`、`WindowControls`
- **弹窗与浮层**：`Modal`、`InputDialog`、`ConfirmDialog`、`Tooltip`
- **表单输入**：`Input`、`NumberInput`、`Search`、`Select`、`Checkbox`、`Switch`、`Textarea`
- **展示与内容**：`Alert`、`Badge`、`Tag`、`Avatar`、`Card`、`Empty`、`Markdown`、`Tabs`
- **编辑与文本**：`CodeEditor`、`StreamText`
- **视觉辅助**：`CubeLoading`、`CubeLogo`、`DotMatrixLoader`、`TextStrokeEffect`
- **业务型 UI**：`ConfigPage`、`FilterPill`、`FlowChatCards`

## 导出与预览

- 在 [index.ts](index.ts) 中维护对外导出
- 在 [registry.tsx](registry.tsx) 中注册预览入口
- README 中的示例应与实际导出的组件保持一致

## 使用方式

```tsx
import { Button, Card, Input, Markdown } from '@components';

function Example() {
  return (
    <Card>
      <Input placeholder="搜索项目文件" />
      <Button variant="primary">运行</Button>
      <Markdown># 预览</Markdown>
    </Card>
  );
}
```

## 开发指南

1. 在 `components/` 下创建组件目录
2. 实现组件及相关样式
3. 在组件 `index.ts` 与 `components/index.ts` 中导出
4. 在 `registry.tsx` 中补充或更新预览演示
5. 若公共接口发生变化，同步更新就近 README

## 注意事项

- 优先沿用相邻组件已有的 SCSS 组织方式与命名习惯
- 组件预览覆盖情况以 [registry.tsx](registry.tsx) 为准
- 当某个组件分组变复杂时，在该子目录补充本地 README
