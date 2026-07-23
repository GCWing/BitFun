[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `tests/e2e`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

Desktop E2E tests built with WebDriverIO plus BitFun's embedded WebDriver.

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
cargo build -p bitfun-desktop
pnpm --dir tests/e2e install
pnpm --dir tests/e2e run test:l0
pnpm --dir tests/e2e run test:l0:all
pnpm --dir tests/e2e run test:l1
pnpm --dir tests/e2e exec wdio run ./config/wdio.conf.ts --spec "./specs/<file>.spec.ts"
```

## Verification

Prefer the narrowest relevant spec first, then broaden only if needed.
