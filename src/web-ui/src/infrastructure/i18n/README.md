[中文](README.zh-CN.md) | **English**

# Web UI I18n

This README is the Web UI runtime entry point. The cross-surface contract,
resource ownership, key policy, and verification rules live in:

- `docs/architecture/i18n.md`
- `docs/development/i18n.md`

Keep this file small so local runtime examples do not drift from the shared
project rules.

## Runtime Usage

Use `useI18n(namespace)` or `useTranslation(namespace)` for route, scene, and
feature UI. This lets Web UI load non-bootstrap namespaces lazily.

```typescript
import { useI18n } from '@/infrastructure/i18n';

const { t } = useI18n('components');

t('dialog.confirm.ok');
t('session.title', { id: 123 });
```

Multiple namespaces are allowed when a component owns copy from more than one
namespace. Relative keys resolve through the first namespace, so prefer explicit
`namespace:key` strings when the key belongs to a later namespace.

```typescript
const { t } = useI18n(['components', 'common']);

t('components:dialog.confirm.ok');
t('common:actions.cancel');
```

Direct `i18nService.t('namespace:key')` calls are for non-React or
module-initialization paths only. The namespace must be in
`WEB_UI_BOOTSTRAP_NAMESPACES`.

```typescript
import { i18nService } from '@/infrastructure/i18n';

i18nService.t('common:actions.cancel');
```

## Resource Ownership

Web UI locale resources live under:

- `src/web-ui/src/locales/<locale>/**/*.json`

Supported locale directories are defined by the shared i18n contract and must
stay aligned with `ALL_NAMESPACES` in `presets/namespaceRegistry.ts`.

Stable product, feature, mode, tool, connection-method, and status labels should
come from the explicit `shared` namespace when the meaning is the same across
surfaces:

```typescript
t('shared:features.deepReview');
```

Feature workflow copy belongs in the nearest feature namespace. Do not import
Web UI locale files from mobile-web, installer, backend, or static pages.

## Checks

Run the smallest matching checks:

```bash
pnpm run i18n:audit          # Web UI locale JSON/resource-only changes
pnpm run i18n:contract:test  # generated contract, shared terms, or namespace-loading rules
pnpm run type-check:web      # Web UI i18n runtime, hooks, or TypeScript call sites
```

For runtime behavior changes, also run the nearest focused Web UI test. CI
covers broad builds and full suites.
