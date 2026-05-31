**中文** | [English](README.md)

# Web UI 国际化

本文档只作为 Web UI 运行时入口。跨形态语言契约、资源归属、key 规范和校验规则以以下文档为准：

- `docs/architecture/i18n.md`
- `docs/development/i18n.md`

这里保持精简，避免本地示例和项目级规范分叉。

## 运行时用法

路由、场景和功能 UI 应优先使用 `useI18n(namespace)` 或
`useTranslation(namespace)`，这样非启动命名空间可以按需懒加载。

```typescript
import { useI18n } from '@/infrastructure/i18n';

const { t } = useI18n('components');

t('dialog.confirm.ok');
t('session.title', { id: 123 });
```

组件确实拥有多个命名空间的文案时，可以传入 namespace 数组。相对 key 会按第一个 namespace 解析，因此引用后续 namespace 时应写完整 `namespace:key`。

```typescript
const { t } = useI18n(['components', 'common']);

t('components:dialog.confirm.ok');
t('common:actions.cancel');
```

直接 `i18nService.t('namespace:key')` 只用于非 React 或模块初始化路径。对应 namespace 必须在 `WEB_UI_BOOTSTRAP_NAMESPACES` 中。

```typescript
import { i18nService } from '@/infrastructure/i18n';

i18nService.t('common:actions.cancel');
```

## 资源归属

Web UI 的 locale 资源位于：

- `src/web-ui/src/locales/<locale>/**/*.json`

支持的 locale 目录由统一 i18n contract 决定，并且必须和
`presets/namespaceRegistry.ts` 中的 `ALL_NAMESPACES` 保持一致。

产品名、功能名、模式名、工具名、连接方式和状态等稳定概念，如果跨形态语义一致，应显式读取 `shared` namespace：

```typescript
t('shared:features.deepReview');
```

功能流程文案应放在最近的功能 namespace 中。不要从 mobile-web、installer、backend 或静态页面导入 Web UI locale 文件。

## 校验

按变更类型选择最小校验：

```bash
pnpm run i18n:audit          # Web UI locale JSON/resource-only 变更
pnpm run i18n:contract:test  # 生成契约、shared terms 或 namespace 加载规则变更
pnpm run type-check:web      # Web UI i18n runtime、hooks 或 TypeScript 调用点变更
```

运行时行为变更还应运行最近的 Web UI 聚焦测试。广泛构建和完整套件由 CI 覆盖。
