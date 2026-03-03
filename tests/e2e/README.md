[中文](README.zh-CN.md) | **English**

# BitFun E2E Tests

E2E test framework using WebDriverIO + tauri-driver.

> For complete documentation, see [E2E-TESTING-GUIDE.md](E2E-TESTING-GUIDE.md)

## Quick Start

### 1. Install Dependencies

```bash
# Install tauri-driver
cargo install tauri-driver --locked

# Build the app
npm run desktop:build

# Install test dependencies
cd tests/e2e && npm install
```

### 2. Run Tests

```bash
cd tests/e2e

# L0 smoke tests (fastest)
npm run test:l0
npm run test:l0:all

# L1 functional tests
npm run test:l1

# Run all tests
npm test
```

## Test Levels

| Level | Purpose | Run Time | AI Required |
|-------|---------|----------|-------------|
| L0 | Smoke tests - verify basic functionality | < 1 min | No |
| L1 | Functional tests - validate features | 5-15 min | No (mocked) |
| L2 | Integration tests - full system validation | 15-60 min | Yes |

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

### tauri-driver not found

```bash
cargo install tauri-driver --locked
```

### App not built

```bash
npm run desktop:build
```

### Test timeout

Debug builds are slower. Adjust timeouts in config if needed.

## More Information

- [Complete Testing Guide](E2E-TESTING-GUIDE.md) - Test writing guidelines, best practices, test plan
- [BitFun Project Structure](../../AGENTS.md)
