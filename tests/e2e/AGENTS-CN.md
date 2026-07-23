**中文** | [English](AGENTS.md)

# AGENTS-CN.md

## 适用范围

本文件适用于 `tests/e2e`。仓库级规则请看顶层 `AGENTS.md`。

## 这里最重要的内容

这里是桌面端 E2E 测试，使用 WebDriverIO 和 BitFun 内置 WebDriver。

根据 `E2E-TESTING-GUIDE.md`：

- L0：smoke tests
- L1：functional tests
- L2：已规划，但尚未实现

核心规则：

1. 测试真实用户工作流
2. 使用 `data-testid` 作为稳定选择器
3. 遵循 Page Object Model
4. 保持测试独立且幂等

## 命令

```bash
cargo build -p bitfun-desktop
pnpm --dir tests/e2e install
pnpm --dir tests/e2e run test:l0
pnpm --dir tests/e2e run test:l0:all
pnpm --dir tests/e2e run test:l1
pnpm --dir tests/e2e exec wdio run ./config/wdio.conf.ts --spec "./specs/<file>.spec.ts"
```

## 验证

优先运行最窄的相关 spec，必要时再扩大范围。
