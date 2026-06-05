**中文** | [English](AGENTS.md)

# Product Domains Agent 指南

适用范围：`src/crates/contracts/product-domains`。

`bitfun-product-domains` 承载可脱离完整 core runtime 编译的平台无关产品领域契约。这里应聚焦纯状态、DTO、策略和窄
ports；具体 runtime 行为不属于本 crate。

## 护栏

- 不要让 `bitfun-product-domains` 依赖 `bitfun-core`。
- 保持 default feature 轻量。默认构建不得引入 runtime、service、desktop、network、process、AI 或 tool-runtime 依赖。
- 本 crate 可以承载纯 DTO、枚举、序列化契约、搜索计划、命令选择决策、storage-shape parser、领域策略和产品领域 port trait。
- 真正执行 IO、进程、AI 调用、Git service 调用、平台集成、tool exposure 或 desktop/Tauri 工作的 concrete adapter 属于本 crate 外部。
- 在下游调用点被有意迁移前，用 re-export 或 wrapper facade 保持既有 core import path。
- 新增 feature-gated 内容必须保持窄边界。`miniapp`、`function-agents` 和 `product-full` 只应启用已声明的产品领域 feature 组。

## 归属边界

- `miniapp` 可以拥有 MiniApp 数据形态、纯生命周期决策、metadata/import policy、built-in bundle identity、embedded source assets、
  seed-plan facts、marker wire format、host primitive call plan 和窄 port。
- `function-agents` 可以拥有 function-agent DTO、prompt/domain policy、response parsing/repair rule、file-shape analysis
  和 Git/AI port trait。
- Core 仍拥有 filesystem writes、marker IO、worker/host side effect、compile orchestration、`PathManager` integration、
  concrete Git/AI service、provider acquisition 和 transport error mapping。

## 验证

按改动范围选择最小验证：

```bash
cargo test -p bitfun-product-domains --no-default-features
cargo test -p bitfun-product-domains --features product-full
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

仅改文档时运行 `git diff --check`。
