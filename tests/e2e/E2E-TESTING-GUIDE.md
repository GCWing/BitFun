[中文](E2E-TESTING-GUIDE.zh-CN.md) | **English**

# BitFun E2E Testing Guide

Complete guide for E2E testing in BitFun project using WebDriverIO + tauri-driver.

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
- Run time: 2-5 minutes
- No AI interaction or workspace required (but may detect workspace state)
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
| `l0-observe.spec.ts` | Manual observation test - keeps app window open for inspection |

### L1 - Functional Tests (Feature Validation)

**Purpose**: Validate major features work end-to-end with real UI interactions.

**Characteristics**:
- Run time: 3-5 minutes
- Workspace is automatically opened (tests run with actual workspace context)
- No AI model required (tests UI behavior, not AI responses)
- Tests verify actual user interactions and state changes

**When to run**: Before feature merge, nightly builds, pre-release

**Test Files**:

| Test File | Verification | Status |
|-----------|--------------|--------|
| `l1-ui-navigation.spec.ts` | Header component, window controls (minimize/maximize/close), window state toggling | 11 passing |
| `l1-workspace.spec.ts` | Workspace state detection, startup page vs workspace UI, window state management | 9 passing |
| `l1-chat-input.spec.ts` | Chat input typing, multiline input, send button state, message clearing | 14 passing |
| `l1-navigation.spec.ts` | Navigation panel structure, clicking nav items to switch views, active item highlighting | 9 passing |
| `l1-file-tree.spec.ts` | File tree display, folder expand/collapse, file selection, git status indicators | 6 passing |
| `l1-editor.spec.ts` | Monaco editor display, file content, tab bar, multi-tab switch, unsaved marker | 6 passing |
| `l1-terminal.spec.ts` | Terminal container, xterm.js display, keyboard input, terminal output | 5 passing |
| `l1-git-panel.spec.ts` | Git panel display, branch name, changed files list, commit input, diff viewing | 9 passing |
| `l1-settings.spec.ts` | Settings button, panel open/close, settings tabs, configuration inputs | 9 passing |
| `l1-session.spec.ts` | Session scene, session list in sidebar, new session button, session switching | 11 passing |
| `l1-dialog.spec.ts` | Modal overlay, confirm dialogs, input dialogs, dialog close (ESC/backdrop) | 13 passing |
| `l1-chat.spec.ts` | Message list display, message sending, stop button, code block rendering, streaming indicator | 14 passing, 1 failing |

### L2 - Integration Tests (Full System)

**Purpose**: Validate complete workflows with real AI integration.

**Characteristics**:
- Run time: 15-60 minutes
- Requires AI provider configuration

**When to run**: Pre-release, manual validation

**Test Files**:

| Test File | Verification |
|-----------|--------------|
| `l2-ai-conversation.spec.ts` | Complete AI conversation flow |
| `l2-tool-execution.spec.ts` | Tool execution (Read, Write, Bash) |
| `l2-multi-step.spec.ts` | Multi-step user journeys |

## Getting Started

### 1. Prerequisites

Install required dependencies:

```bash
# Install tauri-driver
cargo install tauri-driver --locked

# Build the application
npm run desktop:build

# Install E2E test dependencies
cd tests/e2e
npm install
```

### 2. Verify Installation

Check that the app binary exists:

**Windows**: `src/apps/desktop/target/release/BitFun.exe`  
**Linux/macOS**: `src/apps/desktop/target/release/bitfun`

### 3. Run Tests

```bash
# From tests/e2e directory

# Run L0 smoke tests (fastest)
npm run test:l0

# Run all L0 tests
npm run test:l0:all

# Run L1 functional tests
npm run test:l1

# Run specific test file
npm test -- --spec ./specs/l0-smoke.spec.ts
```

### 4. Identify Test Running Mode (Release vs Dev)

The test framework supports two running modes:

#### Release Mode (Default)
- **Application Path**: `target/release/bitfun-desktop.exe`
- **Characteristics**: Optimized build, fast startup, production-ready
- **Use Case**: CI/CD, formal testing

#### Dev Mode
- **Application Path**: `target/debug/bitfun-desktop.exe`
- **Characteristics**: Includes debug symbols, requires dev server (port 1422)
- **Use Case**: Local development, rapid iteration

**How to Identify Current Mode**:

When running tests, check the first few lines of output:

```bash
# Release Mode Output Example
application: C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe
[0-0] Application: C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe
                                          ^^^^^^^^

# Dev Mode Output Example
application: C:\Users\wuxiao\BitFun\target\debug\bitfun-desktop.exe
                                        ^^^^^
Debug build detected, checking dev server...    ← Dev mode specific
Dev server is already running on port 1422      ← Dev mode specific
[0-0] Application: C:\Users\wuxiao\BitFun\target\debug\bitfun-desktop.exe
```

**Quick Check Command**:

```powershell
# Check which mode will be used
if (Test-Path "target/release/bitfun-desktop.exe") {
    Write-Host "Will use: RELEASE MODE"
} elseif (Test-Path "target/debug/bitfun-desktop.exe") {
    Write-Host "Will use: DEV MODE"
}
```

**Force Dev Mode**:

Using convenient scripts (recommended):

```bash
# Switch to Dev mode
cd tests/e2e
./switch-to-dev.ps1

# Run tests
npm run test:l0:all

# Switch back to Release mode
./switch-to-release.ps1
```

Or manual operation:

```bash
# 1. Start dev server (optional but recommended)
npm run dev

# 2. Rename release build
cd target/release
ren bitfun-desktop.exe bitfun-desktop.exe.bak

# 3. Run tests (will automatically use debug build)
cd ../../tests/e2e
npm run test:l0

# 4. Restore release build
cd ../../target/release
ren bitfun-desktop.exe.bak bitfun-desktop.exe
```

**Core Principle**: The test framework prioritizes `target/release/bitfun-desktop.exe`. If it doesn't exist, it automatically uses `target/debug/bitfun-desktop.exe`. Simply delete or rename the release build to switch to dev mode.

## Test Structure

```
tests/e2e/
├── specs/                          # Test specifications
│   ├── l0-smoke.spec.ts           # L0: Basic smoke tests
│   ├── l0-open-workspace.spec.ts  # L0: Workspace opening
│   ├── l0-open-settings.spec.ts   # L0: Settings interaction
│   ├── l1-chat-input.spec.ts      # L1: Chat input validation
│   ├── l1-file-tree.spec.ts       # L1: File tree operations
│   ├── l1-workspace.spec.ts       # L1: Workspace management
│   ├── startup/                    # Startup-related tests
│   │   └── app-launch.spec.ts
│   └── chat/                       # Chat-related tests
│       └── basic-chat.spec.ts
├── page-objects/                   # Page Object Model
│   ├── BasePage.ts                # Base class with common methods
│   ├── ChatPage.ts                # Chat view page object
│   ├── StartupPage.ts             # Startup screen page object
│   └── components/                 # Reusable components
│       ├── Header.ts
│       ├── ChatInput.ts
│       └── MessageList.ts
├── helpers/                        # Utility functions
│   ├── screenshot-utils.ts        # Screenshot capture
│   ├── tauri-utils.ts             # Tauri-specific helpers
│   └── wait-utils.ts              # Wait and retry logic
├── fixtures/                       # Test data
│   └── test-data.json
└── config/                         # Configuration
    ├── wdio.conf.ts               # WebDriverIO config
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

**Bad** ❌:
```typescript
it('should send message', async () => {
  const input = await $('[data-testid="chat-input-textarea"]');
  await input.setValue('Hello');
  const btn = await $('[data-testid="chat-input-send-btn"]');
  await btn.click();
});
```

**Good** ✅:
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
import { saveScreenshot, saveFailureScreenshot } from '../helpers/screenshot-utils';

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
    // Capture screenshot on failure
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(this.currentTest.title);
    }
  });

  after(async () => {
    // Cleanup
    await saveScreenshot('feature-complete');
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

// Use retry for flaky operations
await page.withRetry(async () => {
  await page.clickSend();
  expect(await page.getMessageCount()).toBeGreaterThan(0);
});
```

## Best Practices

### Do's ✅

1. **Keep tests focused** - One test, one assertion concept
2. **Use meaningful test names** - Describe the expected behavior
3. **Test user behavior** - Not implementation details
4. **Handle async properly** - Always await async operations
5. **Clean up after tests** - Reset state when needed
6. **Add screenshots on failure** - Use afterEach hook
7. **Log progress** - Use console.log for debugging
8. **Use environment settings** - Centralize timeouts and retries

### Don'ts ❌

1. **Don't use hard-coded waits** - Use `waitForElement` instead of `pause`
2. **Don't share state between tests** - Each test should be independent
3. **Don't test internal implementation** - Focus on user-visible behavior
4. **Don't ignore flaky tests** - Fix or mark as skipped with reason
5. **Don't use complex selectors** - Prefer data-testid
6. **Don't test third-party code** - Only test BitFun functionality
7. **Don't mix test levels** - Keep L0/L1/L2 separate

### Error Handling

```typescript
it('should handle errors gracefully', async () => {
  try {
    await page.performRiskyAction();
  } catch (error) {
    // Capture context
    await saveFailureScreenshot('error-context');
    const pageSource = await browser.getPageSource();
    console.error('Page state:', pageSource.substring(0, 500));
    throw error; // Re-throw to fail the test
  }
});
```

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

#### 1. tauri-driver not found

**Symptom**: `Error: spawn tauri-driver ENOENT`

**Solution**:
```bash
# Install or update tauri-driver
cargo install tauri-driver --locked

# Verify installation
tauri-driver --version

# Ensure ~/.cargo/bin is in PATH
echo $PATH  # macOS/Linux
echo %PATH% # Windows
```

#### 2. App not built

**Symptom**: `Binary not found at target/release/BitFun.exe`

**Solution**:
```bash
# Build the app
npm run desktop:build

# Verify binary exists
ls src/apps/desktop/target/release/
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

// Use environment settings
import { environmentSettings } from '../config/capabilities';
await page.waitForElement(selector, environmentSettings.pageLoadTimeout);

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
await page.takeScreenshot('debug-element-not-found');

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

// Add retry logic
await page.withRetry(async () => {
  await page.clickButton();
  expect(await page.isActionComplete()).toBe(true);
});

// Ensure test independence
beforeEach(async () => {
  await page.resetState();
});
```

### Debug Mode

Run tests with debugging enabled:

```bash
# Enable WebDriverIO debug logs
npm test -- --spec ./specs/l0-smoke.spec.ts --log-level=debug

# Keep browser open on failure
# (Modify wdio.conf.ts: bail: 1)
```

### Screenshot Analysis

Screenshots are saved to `tests/e2e/reports/screenshots/`:

```typescript
// Manual screenshot
await page.takeScreenshot('my-debug-point');

// Auto-capture on failure (add to test)
afterEach(async function () {
  if (this.currentTest?.state === 'failed') {
    await saveFailureScreenshot(this.currentTest.title);
  }
});
```

## Adding New Tests

### Step-by-Step Guide

1. **Identify the test level** (L0/L1/L2)
2. **Create test file** in appropriate directory
3. **Add data-testid to UI elements** (if needed)
4. **Create or update Page Objects**
5. **Write test following template**
6. **Run test locally**
7. **Add to CI/CD pipeline** (for L0/L1)

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
5. Run: `npm test -- --spec ./specs/l1-file-tree.spec.ts`
6. Update `package.json`:
   ```json
   "test:l1:filetree": "wdio run ./config/wdio.conf.ts --spec ./specs/l1-file-tree.spec.ts"
   ```

## CI/CD Integration

### Recommended Test Strategy

```yaml
# .github/workflows/e2e.yml (example)
name: E2E Tests

on: [push, pull_request]

jobs:
  l0-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build app
        run: npm run desktop:build
      - name: Install tauri-driver
        run: cargo install tauri-driver --locked
      - name: Run L0 tests
        run: cd tests/e2e && npm run test:l0:all
        
  l1-tests:
    runs-on: ubuntu-latest
    needs: l0-tests
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v3
      - name: Build app
        run: npm run desktop:build
      - name: Run L1 tests
        run: cd tests/e2e && npm run test:l1
```

### Test Execution Matrix

| Event | L0 | L1 | L2 |
|-------|----|----|---- |
| Every commit | ✅ | ❌ | ❌ |
| Pull request | ✅ | ✅ | ❌ |
| Nightly build | ✅ | ✅ | ✅ |
| Pre-release | ✅ | ✅ | ✅ |

## Test Execution Results

### Latest Test Results (2026-03-03)

**L0 Tests (Smoke Tests)**:
- Passed: 8/8 (100%)
- Run time: ~1.5 minutes
- Status: All passing ✅

**L1 Tests (Functional Tests)**:
- Test Files: 11 passed, 1 failed, 12 total
- Test Cases: 116 passing, 1 failing
- Run time: ~3.5 minutes
- Pass Rate: 99.1%

**L1 Detailed Results by Test File**:

| Test File | Passing | Failing | Notes |
|-----------|---------|---------|-------|
| l1-ui-navigation.spec.ts | 11 | 0 | Header, window controls working ✅ |
| l1-workspace.spec.ts | 9 | 0 | Workspace state detection working ✅ |
| l1-chat-input.spec.ts | 14 | 0 | All input interactions passing ✅ |
| l1-navigation.spec.ts | 9 | 0 | All navigation tests passing ✅ |
| l1-file-tree.spec.ts | 6 | 0 | File tree tests passing ✅ |
| l1-editor.spec.ts | 6 | 0 | Editor tests passing ✅ |
| l1-terminal.spec.ts | 5 | 0 | Terminal tests passing ✅ |
| l1-git-panel.spec.ts | 9 | 0 | Git panel fully working ✅ |
| l1-settings.spec.ts | 9 | 0 | All settings tests passing ✅ |
| l1-session.spec.ts | 11 | 0 | Session management fully working ✅ |
| l1-dialog.spec.ts | 13 | 0 | All dialog tests passing ✅ |
| l1-chat.spec.ts | 14 | 1 | Chat display mostly working ⚠️ |

**Fixed Issues** (2026-03-03 fixes):
1. ✅ l1-chat-input: Multiline input handling - Using Shift+Enter for newlines
2. ✅ l1-chat-input: Send button state detection - Enhanced state detection logic
3. ✅ l1-navigation: Element interactability - Added scroll and retry logic
4. ✅ l1-file-tree: File tree visibility - Enhanced selectors and view switching
5. ✅ l1-settings: Settings button finding - Expanded selector coverage
6. ✅ l1-session: Mode attribute validation - Fixed test logic to allow null
7. ✅ l1-ui-navigation: Focus management - Added focus acquisition retry logic

**Remaining Issues**:
1. ⚠️ l1-chat: Input clearing timing after message send (edge case related to AI response processing)

**L2 Tests (Integration Tests)**:
- Status: Not yet implemented (0%)
- Test Files: None

**Improvements**:

1. **L0 tests 100% passing**: Application startup and basic UI structure verified ✅
2. **L1 tests 99.1% pass rate**: Improved from 91.7% (98/107) to 99.1% (116/117)
3. **Fixed 7 core issues**: Input handling, navigation interaction, element detection
4. **Test stability significantly improved**: Reduced 17 skipped tests, all tests now execute properly

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
