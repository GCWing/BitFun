[中文](README.zh-CN.md) | **English**

# BitFun E2E Tests

E2E test framework using WebDriverIO + the embedded BitFun WebDriver.

> For complete documentation, see [E2E-TESTING-GUIDE.md](E2E-TESTING-GUIDE.md)

## Quick Start

### 1. Install Dependencies

```bash
# Build the debug app
cargo build -p bitfun-desktop

# Install test dependencies
pnpm --dir tests/e2e install
```

### 2. Run Tests

```bash
# L0 smoke tests (fastest)
pnpm --dir tests/e2e run test:l0
pnpm --dir tests/e2e run test:l0:all

# L1 functional tests
pnpm --dir tests/e2e run test:l1

# Run all tests
pnpm --dir tests/e2e test
```

## Test Levels

| Level | Purpose | Run Time | AI Required |
|-------|---------|----------|-------------|
| L0 | Smoke tests - verify basic functionality | < 1 min | No |
| L1 | Functional tests - validate features | 5-15 min | No (mocked) |
| L2 | Planned, not implemented yet | N/A | N/A |

## Directory Structure

```
tests/e2e/
├── specs/           # Test specifications
├── page-objects/    # Page Object Model
├── helpers/         # Utility functions
├── fixtures/        # Test data
└── config/          # Configuration
```

## Troubleshooting

### Embedded WebDriver not ready

The test runner starts BitFun directly and waits for the embedded WebDriver service on `127.0.0.1:4445`.

### App not built

```bash
cargo build -p bitfun-desktop
```

### Test timeout

Debug builds are slower. Adjust timeouts in config if needed.

## More Information

- [Complete Testing Guide](E2E-TESTING-GUIDE.md) - Test writing guidelines, best practices, test plan
- [BitFun Project Structure](../../AGENTS.md)
