[中文](E2E-TESTING-GUIDE.zh-CN.md) | **English**

# BitFun E2E Testing Guide

Complete guide for E2E testing in BitFun project using WebDriverIO + BitFun embedded WebDriver.

## Table of Contents

- [Testing Philosophy](#testing-philosophy)
- [Test Levels](#test-levels)
- [Getting Started](#getting-started)
- [Test Structure](#test-structure)
- [Writing Tests](#writing-tests)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Testing Philosophy

BitFun E2E tests focus on **user journeys** and **critical paths** to ensure the desktop application works correctly from the user's perspective. We use a layered testing approach to balance coverage and execution speed.

### Key Principles

1. **Test real user workflows**, not implementation details
2. **Use data-testid attributes** for stable selectors
3. **Follow Page Object Model** for maintainability
4. **Keep tests independent** and idempotent
5. **Fail fast** with clear error messages

## Test Levels

BitFun uses a 3-tier test classification system:

### L0 - Smoke Tests (Critical Path)

**Purpose**: Verify basic app functionality; must pass before any release.

**Characteristics**:
- Run time: 1-2 minutes
- No AI interaction or workspace required
- Can run in CI/CD
- Tests verify UI elements exist and are accessible

**When to run**: Every commit, before merge, pre-release

**Test Files**:

| Test File | Verification |
|-----------|--------------|
| `l0-smoke.spec.ts` | App startup, DOM structure, Header visibility, no critical JS errors |
| `l0-open-workspace.spec.ts` | Workspace state detection (startup page vs workspace), startup page interaction |
| `l0-open-settings.spec.ts` | Settings button visibility, settings panel open/close |
| `l0-navigation.spec.ts` | Sidebar exists when workspace open, nav items visible and clickable |
| `l0-tabs.spec.ts` | Tab bar exists when files open, tabs display correctly |
| `l0-theme.spec.ts` | Theme attributes on root element, theme CSS variables, theme system functional |
| `l0-i18n.spec.ts` | Language configuration, i18n system functional, translated content |
| `l0-notification.spec.ts` | Notification service available, notification entry visible in header |
| `l0-observe.spec.ts` | Manual observation test - keeps app window open for 60 seconds for inspection |

### L1 - Functional Tests (Feature Validation)

**Purpose**: Validate major features work end-to-end with real UI interactions.

**Characteristics**:
- Run time: 3-5 minutes
- Workspace is automatically opened (tests run with actual workspace context)
- No AI model required (tests UI behavior, not AI responses)
- Tests verify actual user interactions and state changes

**When to run**: Before feature merge, nightly builds, pre-release

**Test Files**:

| Test File | Verification |
|-----------|--------------|
| `l1-ui-navigation.spec.ts` | Header component, window controls (minimize/maximize/close), window state toggling |
| `l1-workspace.spec.ts` | Workspace state detection, startup page vs workspace UI, window state management |
| `l1-chat-input.spec.ts` | Chat input typing, multiline input (Shift+Enter), send button state, message clearing |
| `l1-navigation.spec.ts` | Navigation panel structure, clicking nav items to switch views, active item highlighting |
| `l1-file-tree.spec.ts` | File tree display, folder expand/collapse, file selection, open file in editor |
| `l1-editor.spec.ts` | Monaco editor display, file content, tab bar, multi-tab switch/close, unsaved marker |
| `l1-terminal.spec.ts` | Terminal container, xterm.js display, keyboard input, terminal output |
| `l1-git-panel.spec.ts` | Git panel display, branch name, changed files list, commit input, diff viewing |
| `l1-settings.spec.ts` | Settings button, panel open/close, settings tabs, configuration inputs |
| `l1-session.spec.ts` | Session scene, session list in sidebar, new session button, session switching |
| `l1-dialog.spec.ts` | Modal overlay, confirm dialogs, input dialogs, dialog close (ESC/backdrop) |
| `l1-chat.spec.ts` | Message list display, message sending, stop button, code block rendering, streaming indicator |

### L2 - Integration Tests (Full System)

**Purpose**: Validate complete workflows with real AI integration.

**Characteristics**:
- Run time: 15-60 minutes
- Requires AI provider configuration

**When to run**: Pre-release, manual validation

**Current Status**: L2 tests are not yet implemented

**Planned Test Files**:

| Test File | Verification | Status |
|-----------|--------------|--------|
| `l2-ai-conversation.spec.ts` | Complete AI conversation flow | Not implemented |
| `l2-tool-execution.spec.ts` | Tool execution (Read, Write, Bash) | Not implemented |
| `l2-multi-step.spec.ts` | Multi-step user journeys | Not implemented |

## Getting Started

### 1. Prerequisites

Install required dependencies:

```bash
# Install E2E test dependencies
cd tests/e2e
pnpm install

# Build the application (from project root)
cd ../..
cargo build -p bitfun-desktop
```

### 2. Verify Installation

Check that the app binary exists:

**Windows**: `target/debug/bitfun-desktop.exe`
**Linux/macOS**: `target/debug/bitfun-desktop`

### 3. Run Tests

```bash
# From tests/e2e directory

# Run L0 smoke tests (fastest)
pnpm run test:l0

# Run all L0 tests
pnpm run test:l0:all

# Run L1 functional tests
pnpm run test:l1

# Run specific test file
pnpm test -- --spec ./specs/l0-smoke.spec.ts
```

### 4. Test Running Mode (Release vs Dev)

The default test framework runs in debug/dev mode. Performance runs can use
`release-fast` so startup/session timings are closer to a production bundle
while still enabling the embedded WebDriver through the `devtools` feature.

#### Debug Mode (Default)
- **Application Path**: `target/debug/bitfun-desktop.exe`
- **Characteristics**: Includes debug symbols, requires dev server (port 1422)
- **Use Case**: Local development, rapid iteration

#### Release-Fast Performance Mode
- **Application Path**: `target/release-fast/bitfun-desktop.exe`
- **Characteristics**: Production web bundle, release-like Rust profile, embedded WebDriver enabled by `--features devtools`
- **Use Case**: Startup and historical-session performance measurements

**How to Identify Current Mode**:

When running tests, check the first few lines of output:

```bash
# Debug Mode Output
application: <PROJECT_ROOT>\target\debug\bitfun-desktop.exe
Debug build detected, checking dev server...
```

**Core Principle**: Functional E2E still defaults to `target/debug/bitfun-desktop.exe`.
Performance E2E should explicitly set `BITFUN_E2E_APP_MODE=release-fast` after
building `pnpm run desktop:build:release-fast`.

### 5. Startup and Long-Session Performance E2E

Generate a long-session fixture in the workspace you want to measure:

```bash
pnpm --dir tests/e2e run fixture:long-session -- --workspace <workspace-path> --session-count 80 --long-turns 80
```

Build and run the release-like performance spec:

```bash
pnpm run desktop:build:release-fast
cross-env E2E_TEST_WORKSPACE=<workspace-path> BITFUN_E2E_PERF_SESSION_ID=perf-long-session-000 pnpm run e2e:test:perf:release-fast
```

For debug-only comparison, build the debug binary and run:

```bash
cargo build -p bitfun-desktop
cross-env E2E_TEST_WORKSPACE=<workspace-path> BITFUN_E2E_PERF_SESSION_ID=perf-long-session-000 pnpm run e2e:test:perf:debug
```

The spec writes JSON reports under `tests/e2e/reports/performance/`. It records
startup milestones, Tauri API aggregates, first-open session hydration timings,
and background full-hydrate timings when they occur. Optional threshold gates can
be enabled with `BITFUN_E2E_PERF_MAX_INTERACTIVE_MS` and
`BITFUN_E2E_PERF_MAX_SESSION_FRAME_MS`.

## Test Structure

```
tests/e2e/
├── specs/                          # Test specifications
│   ├── l0-smoke.spec.ts           # L0: Basic smoke tests
│   ├── l0-open-workspace.spec.ts  # L0: Workspace detection
│   ├── l0-open-settings.spec.ts   # L0: Settings interaction
│   ├── l0-navigation.spec.ts      # L0: Navigation sidebar
│   ├── l0-tabs.spec.ts            # L0: Tab bar
│   ├── l0-theme.spec.ts           # L0: Theme system
│   ├── l0-i18n.spec.ts            # L0: Internationalization
│   ├── l0-notification.spec.ts    # L0: Notification system
│   ├── l0-observe.spec.ts         # L0: Manual observation
│   ├── l1-ui-navigation.spec.ts   # L1: Window controls
│   ├── l1-workspace.spec.ts       # L1: Workspace management
│   ├── l1-chat-input.spec.ts      # L1: Chat input
│   ├── l1-navigation.spec.ts      # L1: Navigation panel
│   ├── l1-file-tree.spec.ts       # L1: File tree operations
│   ├── l1-editor.spec.ts          # L1: Editor functionality
│   ├── l1-terminal.spec.ts        # L1: Terminal
│   ├── l1-git-panel.spec.ts       # L1: Git panel
│   ├── l1-settings.spec.ts        # L1: Settings panel
│   ├── l1-session.spec.ts         # L1: Session management
│   ├── l1-dialog.spec.ts          # L1: Dialog components
│   └── l1-chat.spec.ts            # L1: Chat functionality
├── page-objects/                   # Page Object Model
│   ├── BasePage.ts                # Base class with common methods
│   ├── ChatPage.ts                # Chat view page object
│   ├── StartupPage.ts             # Startup screen page object
│   ├── index.ts                   # Page object exports
│   └── components/                 # Reusable components
│       ├── Header.ts              # Header component
│       └── ChatInput.ts           # Chat input component
├── helpers/                        # Utility functions
│   ├── index.ts                   # Helper exports
│   ├── screenshot-utils.ts        # Screenshot capture
│   ├── tauri-utils.ts             # Tauri-specific helpers
│   ├── wait-utils.ts              # Wait and retry logic
│   ├── workspace-helper.ts        # Workspace operations
│   └── workspace-utils.ts         # Workspace utilities
├── fixtures/                       # Test data
│   └── test-data.json
└── config/                         # Configuration
    ├── wdio.conf.ts               # WebDriverIO base config
    ├── wdio.conf_l0.ts            # L0 test configuration
    ├── wdio.conf_l1.ts            # L1 test configuration
    └── capabilities.ts            # Platform capabilities
```

## Writing Tests

### 1. Test File Naming

Follow this convention:

```
{level}-{feature}.spec.ts

Examples:
- l0-smoke.spec.ts
- l1-chat-input.spec.ts
- l2-ai-conversation.spec.ts
```

### 2. Use Page Objects

**Bad**:
```typescript
it('should send message', async () => {
  const input = await $('[data-testid="chat-input-textarea"]');
  await input.setValue('Hello');
  const btn = await $('[data-testid="chat-input-send-btn"]');
  await btn.click();
});
```

**Good**:
```typescript
import { ChatPage } from '../page-objects/ChatPage';

it('should send message', async () => {
  const chatPage = new ChatPage();
  await chatPage.sendMessage('Hello');
});
```

### 3. Test Structure Template

```typescript
/**
 * L1 Feature name spec: description of what this test validates.
 */

import { browser, expect } from '@wdio/globals';
import { SomePage } from '../page-objects/SomePage';

describe('Feature Name', () => {
  const page = new SomePage();

  before(async () => {
    // Setup - runs once before all tests
    await browser.pause(3000);
    await page.waitForLoad();
  });

  describe('Sub-feature 1', () => {
    it('should do something', async () => {
      // Arrange
      const initialState = await page.getState();
      
      // Act
      await page.performAction();
      
      // Assert
      const newState = await page.getState();
      expect(newState).not.toEqual(initialState);
    });
  });

  afterEach(async function () {
    // Capture screenshot on failure (handled by config)
  });

  after(async () => {
    // Cleanup
  });
});
```

### 4. data-testid Naming Convention

Format: `{module}-{component}-{element}`

**Examples**:
```html
<!-- Startup page -->
<div data-testid="startup-container">
  <button data-testid="startup-open-folder-btn">Open Folder</button>
  <div data-testid="startup-recent-projects">...</div>
</div>

<!-- Chat -->
<div data-testid="chat-input-container">
  <textarea data-testid="chat-input-textarea"></textarea>
  <button data-testid="chat-input-send-btn">Send</button>
</div>

<!-- Header -->
<header data-testid="header-container">
  <button data-testid="header-minimize-btn">_</button>
  <button data-testid="header-maximize-btn">□</button>
  <button data-testid="header-close-btn">×</button>
</header>
```

### 5. Assertions

Use clear, specific assertions:

```typescript
// Good: Specific expectations
expect(await header.isVisible()).toBe(true);
expect(messages.length).toBeGreaterThan(0);
expect(await input.getValue()).toBe('Expected text');

// Avoid: Vague assertions
expect(true).toBe(true); // meaningless
```

### 6. Waits and Retries

Use built-in wait utilities:

```typescript
import { waitForElementStable, waitForStreamingComplete } from '../helpers/wait-utils';

// Wait for element to become stable
await waitForElementStable('[data-testid="message-list"]', 500, 10000);

// Wait for streaming to complete
await waitForStreamingComplete('[data-testid="model-response"]', 2000, 30000);
```

## Best Practices

### Do's

1. **Keep tests focused** - One test, one assertion concept
2. **Use meaningful test names** - Describe the expected behavior
3. **Test user behavior** - Not implementation details
4. **Handle async properly** - Always await async operations
5. **Clean up after tests** - Reset state when needed
6. **Log progress** - Use console.log for debugging
7. **Use environment settings** - Centralize timeouts and retries

### Don'ts

1. **Don't use hard-coded waits** - Use `waitForElement` instead of `pause`
2. **Don't share state between tests** - Each test should be independent
3. **Don't test internal implementation** - Focus on user-visible behavior
4. **Don't ignore flaky tests** - Fix or mark as skipped with reason
5. **Don't use complex selectors** - Prefer data-testid
6. **Don't test third-party code** - Only test BitFun functionality
7. **Don't mix test levels** - Keep L0/L1/L2 separate

### Conditional Tests

```typescript
it('should test feature when workspace is open', async function () {
  const startupVisible = await startupPage.isVisible();
  
  if (startupVisible) {
    console.log('[Test] Skipping: workspace not open');
    this.skip();
    return;
  }
  
  // Test continues...
});
```

## Troubleshooting

### Common Issues

#### 1. Embedded WebDriver not reachable

**Symptom**: session creation or `/status` checks fail against `http://127.0.0.1:4445`

**Solution**:
```bash
# Build the debug desktop app
cargo build -p bitfun-desktop

# Run tests in debug mode so the embedded driver starts inside BitFun
BITFUN_E2E_APP_MODE=debug pnpm --dir tests/e2e run test:l0:protocol

# Verify the app process is allowed to bind 127.0.0.1:4445
```

#### 2. App not built

**Symptom**: `Application not found at target/debug/bitfun-desktop.exe`

**Solution**:
```bash
# Build the app (from project root)
cargo build -p bitfun-desktop

# Verify binary exists
# Windows
dir target\debug\bitfun-desktop.exe
# Linux/macOS
ls -la target/debug/bitfun-desktop
```

#### 3. Test timeouts

**Symptom**: Tests fail with "timeout" errors

**Causes**:
- Slow app startup (debug builds are slower)
- Element not visible yet
- Network delays

**Solutions**:
```typescript
// Increase timeout for specific operation
await page.waitForElement(selector, 30000);

// Add strategic waits
await browser.pause(1000); // After clicking
```

#### 4. Element not found

**Symptom**: `Element with selector '[data-testid="..."]' not found`

**Debug steps**:
```typescript
// 1. Check if element exists
const exists = await page.isElementExist('[data-testid="my-element"]');
console.log('Element exists:', exists);

// 2. Capture page source
const html = await browser.getPageSource();
console.log('Page HTML:', html.substring(0, 1000));

// 3. Take screenshot
await browser.saveScreenshot('./reports/screenshots/debug.png');

// 4. Verify data-testid in frontend code
// Check src/web-ui/src/... for the component
```

#### 5. Flaky tests

**Symptoms**: Tests pass sometimes, fail other times

**Common causes**:
- Race conditions
- Timing issues
- State pollution between tests

**Solutions**:
```typescript
// Use waitForElement instead of pause
await page.waitForElement(selector);

// Ensure test independence
beforeEach(async () => {
  await page.resetState();
});
```

### Debug Mode

Run tests with debugging enabled:

```bash
# Enable WebDriverIO debug logs
pnpm test -- --spec ./specs/l0-smoke.spec.ts --log-level=debug
```

### Screenshot Analysis

Screenshots are automatically saved to `tests/e2e/reports/screenshots/` on test failure.

## Adding New Tests

### Step-by-Step Guide

1. **Identify the test level** (L0/L1/L2)
2. **Create test file** in `specs/` directory
3. **Add data-testid to UI elements** (if needed)
4. **Create or update Page Objects** in `page-objects/`
5. **Write test following template**
6. **Run test locally** to verify
7. **Add pnpm script** to `package.json` (optional)
8. **Update config** to include new spec file

### Example: Adding L1 File Tree Test

1. Create `tests/e2e/specs/l1-file-tree.spec.ts`
2. Add data-testid to file tree component:
   ```tsx
   <div data-testid="file-tree-container">
     <div data-testid="file-tree-item" data-path={path}>
   ```
3. Create `page-objects/FileTreePage.ts`:
   ```typescript
   export class FileTreePage extends BasePage {
     async getFiles() { ... }
     async clickFile(name: string) { ... }
   }
   ```
4. Write test:
   ```typescript
   describe('L1 File Tree', () => {
     it('should display workspace files', async () => {
       const files = await fileTree.getFiles();
       expect(files.length).toBeGreaterThan(0);
     });
   });
   ```
5. Run: `pnpm test -- --spec ./specs/l1-file-tree.spec.ts`
6. Update `config/wdio.conf_l1.ts` to include the new spec

## CI/CD Integration

### Recommended Test Strategy

```yaml
# .github/workflows/e2e.yml (example)
name: E2E Tests

on: [push, pull_request]

jobs:
  l0-tests:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 10.15.0
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '20'
          cache: 'pnpm'
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Build app
        run: cargo build -p bitfun-desktop
      - name: Install test dependencies
        run: cd tests/e2e && pnpm install
      - name: Run L0 tests
        run: cd tests/e2e && BITFUN_E2E_APP_MODE=debug pnpm run test:l0:all
        
  l1-tests:
    runs-on: windows-latest
    needs: l0-tests
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v3
      - name: Build app
        run: cargo build -p bitfun-desktop
      - name: Run L1 tests
        run: cd tests/e2e && BITFUN_E2E_APP_MODE=debug pnpm run test:l1
```

### Test Execution Matrix

| Event | L0 | L1 | L2 |
|-------|----|----|-----|
| Every commit | Yes | No | No |
| Pull request | Yes | Yes | No |
| Nightly build | Yes | Yes | Yes |
| Pre-release | Yes | Yes | Yes |

## Available pnpm Scripts

| Script | Description |
|--------|-------------|
| `pnpm run test` | Run all tests with default config |
| `pnpm run test:l0` | Run L0 smoke test only |
| `pnpm run test:l0:all` | Run all L0 tests |
| `pnpm run test:l1` | Run all L1 tests |
| `pnpm run test:l0:workspace` | Run workspace test |
| `pnpm run test:l0:settings` | Run settings test |
| `pnpm run test:l0:navigation` | Run navigation test |
| `pnpm run test:l0:tabs` | Run tabs test |
| `pnpm run test:l0:theme` | Run theme test |
| `pnpm run test:l0:i18n` | Run i18n test |
| `pnpm run test:l0:notification` | Run notification test |
| `pnpm run test:l0:observe` | Run observation test (60s) |
| `pnpm run clean` | Clean reports directory |

## Resources

- [WebDriverIO Documentation](https://webdriver.io/)
- [Tauri Testing Guide](https://tauri.app/v1/guides/testing/)
- [Page Object Model Pattern](https://webdriver.io/docs/pageobjects/)
- [BitFun Project Structure](../../AGENTS.md)

## Contributing

When adding tests:

1. Follow the existing structure and conventions
2. Use Page Object Model
3. Add data-testid to new UI elements
4. Keep tests at appropriate level (L0/L1/L2)
5. Update this guide if introducing new patterns

## Support

For issues or questions:

1. Check [Troubleshooting](#troubleshooting) section
2. Review existing test files for examples
3. Open an issue with test logs and screenshots
