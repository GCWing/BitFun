**中文** | [English](README.md)

# BitFun 组件库

## 简介

这是 BitFun 的组件预览系统，用于快速查看和测试 UI 组件。

## 快速开始

### 启动预览服务

```bash
pnpm run preview-components
```

这会启动开发服务器，并在 `http://localhost:3000` 打开预览页面。

### 构建预览站点

```bash
pnpm run build-components
```

构建产物输出到 `dist-preview` 目录。

## 目录结构

```
src/component-library/
├── components/          # 组件定义
│   ├── index.ts        # 组件导出
│   └── registry.tsx    # 组件注册表
├── preview/            # 预览系统
│   ├── PreviewApp.tsx  # 预览主应用
│   ├── preview.css     # 预览样式
│   └── main.tsx        # 预览入口
├── types/              # 类型定义
│   └── index.ts
└── index.ts            # 组件库入口
```

## 添加新组件

### 1. 在 `registry.tsx` 中注册

```tsx
import { YourNewComponent } from '@/shared/ui/YourNewComponent';

export const componentRegistry: ComponentCategory[] = [
  {
    id: 'your-category',
    name: '你的分类',
    description: '分类描述',
    components: [
      {
        id: 'your-component-id',
        name: 'YourComponent',
        description: '组件描述',
        category: 'your-category',
        component: () => <YourNewComponent prop1="value" />,
      },
    ],
  },
];
```

### 2. 可选：在 `components/index.ts` 中导出

```tsx
export { YourNewComponent } from '@/shared/ui/YourNewComponent';
```

### 3. 查看预览

运行 `pnpm run preview-components`，新组件会出现在预览系统中。

## 组件分类

当前支持：

- **基础组件** - 按钮、输入框等基础 UI 元素
- **布局组件** - 卡片、面板等布局容器
- **反馈组件** - 加载、模态框等状态反馈

可按需扩展分类。

## 自定义样式

预览样式位于 `preview/preview.css`。

## 使用建议

1. **组件开发** - 在预览系统中测试不同状态和属性
2. **视觉检查** - 用于视觉回归检查
3. **文档参考** - 作为组件使用示例参考

## 技术栈

- React 19
- TypeScript
- Vite
- CSS Modules

## 注意事项

- 预览系统使用独立的 Vite 配置：`src/web-ui/vite.config.preview.ts`
- 预览入口是 `preview.html`
- 预览系统不会影响主应用构建
