<div align="center">

![taiji-quant](./png/taiji-icon-128.png)

# taiji-quant

**太极量化交易 + 自媒体工厂开源基座**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow?style=flat-square)](./LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square)](https://github.com/1688mengdie/taiji-quant)

</div>

基于 [BitFun](https://github.com/GCWing/BitFun) 社区构建的开源量化交易与自媒体系统。MIT License，完全开源。

## 架构

taiji-quant 是 BitFun 生态的独立发行版，包含完整的桌面客户端、CLI、Web UI 和量化交易引擎。

核心模块：
- `src/crates/taiji/` — 太极量化引擎：回测、实时交易、策略生成、情绪分析、订单流、知识图谱
- `src/crates/adapters/` — AI 适配器：多模型接入
- `src/crates/execution/` — 执行原语：Agent 运行时、工具契约、流式处理
- `src/apps/` — 桌面客户端 + CLI + 服务端

## 快速开始

```bash
# 安装依赖
pnpm install

# 开发模式（全热重载）
pnpm run desktop:dev

# 编译验证
cargo check --workspace
```

## 上游同步

本仓库通过 `scripts/sync-upstream.ps1` 与 [GCWing/BitFun](https://github.com/GCWing/BitFun) 主分支保持同步。

```powershell
.\scripts\sync-upstream.ps1
```

## 许可证

MIT License — 详见 [LICENSE](./LICENSE)

## 致谢

基于 [BitFun](https://github.com/GCWing/BitFun) 社区构建。感谢所有贡献者。
