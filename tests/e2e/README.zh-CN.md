**中文** | [English](README.md)

# BitFun E2E 测试

使用 WebDriverIO + tauri-driver 的 E2E 测试框架。

> 完整文档请参阅 [E2E-TESTING-GUIDE.zh-CN.md](E2E-TESTING-GUIDE.zh-CN.md)

## 快速开始

### 1. 安装依赖

```bash
# 安装 tauri-driver
cargo install tauri-driver --locked

# 构建应用
npm run desktop:build

# 安装测试依赖
cd tests/e2e && npm install
```

### 2. 运行测试

```bash
cd tests/e2e

# L0 冒烟测试 (最快)
npm run test:l0
npm run test:l0:all

# L1 功能测试
npm run test:l1

# 运行所有测试
npm test
```

## 测试级别

| 级别 | 目的 | 运行时间 | AI需求 |
|------|------|----------|--------|
| L0 | 冒烟测试 - 验证基本功能 | < 1分钟 | 不需要 |
| L1 | 功能测试 - 验证功能特性 | 5-15分钟 | 不需要(mock) |
| L2 | 集成测试 - 完整系统验证 | 15-60分钟 | 需要 |

## 目录结构

```
tests/e2e/
├── specs/           # 测试用例
├── page-objects/    # Page Object 模型
├── helpers/         # 辅助工具
├── fixtures/        # 测试数据
└── config/          # 配置文件
```

## 常见问题

### tauri-driver 找不到

```bash
cargo install tauri-driver --locked
```

### 应用未构建

```bash
npm run desktop:build
```

### 测试超时

Debug 构建启动较慢，可在配置中调整超时时间。

## 更多信息

- [完整测试指南](E2E-TESTING-GUIDE.zh-CN.md) - 测试编写规范、最佳实践、测试计划
- [BitFun 项目结构](../../AGENTS.md)
