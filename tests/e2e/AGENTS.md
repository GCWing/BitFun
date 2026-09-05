[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `tests/e2e`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

Desktop E2E tests built with WebDriverIO plus OpenBitFun's embedded WebDriver.

Levels from `E2E-TESTING-GUIDE.md`:

- L0: smoke tests
- L1: functional tests
- L2: planned, not implemented yet

Core rules:

1. Test real user workflows
2. Use `data-testid` for stable selectors
3. Follow the Page Object Model
4. Keep tests independent and idempotent

## Commands

```bash
cargo build -p openbitfun-desktop
pnpm --dir tests/e2e install
pnpm --dir tests/e2e run test:l0
pnpm --dir tests/e2e run test:l0:all
pnpm --dir tests/e2e run test:l1
pnpm --dir tests/e2e exec wdio run ./config/wdio.conf.ts --spec "./specs/<file>.spec.ts"
```

## Verification

Prefer the narrowest relevant spec first, then broaden only if needed.

Markdown editor browser interaction tests (no desktop binary required):

```bash
pnpm --dir tests/e2e exec wdio run ./config/wdio.markdown-browser.ts
```

This focused runner mounts the production file editor with temporary file IO
through a test adapter; it does not replace desktop or remote transport coverage.
See `src/web-ui/src/tools/editor/AGENTS.md` for scope and output locations.

For the real desktop Markdown workflow, build the desktop and current frontend,
then run `pnpm --dir tests/e2e exec wdio run ./config/wdio.markdown-native.ts`
from the repository root. This focused runner uses packaged frontend assets and
a fresh temporary application profile; it does not use another checkout's dev server.
