**中文** | [English](AGENTS.md)

# Product Domains Agent 指南

适用范围：`src/crates/product-domains`。

`bitfun-product-domains` 负责可以脱离完整 core runtime 编译的低风险产品领域契约。
这里的抽取必须保持行为等价与平台无关；在所有下游调用点被有意迁移前，
`bitfun-core` 可以继续保留兼容 re-export 或 wrapper facade。

## 护栏

- 不要让 `bitfun-product-domains` 依赖 `bitfun-core`。
- 保持 default feature 轻量。默认构建不应引入 runtime、service、desktop、
  network、process、AI 或 tool-runtime 依赖。
- 本 crate 可以承载纯 DTO、枚举、序列化契约、搜索计划、命令选择决策、
  host-routing string rule、storage-shape parser、小型 helper，以及只依赖 `std` 或窄 feature 轻量依赖的
  文件形态分析器。
- 本 crate 可以定义面向后续 runtime 迁移的产品领域 port trait，但真正执行 IO、
  进程、AI 调用、Git service 调用或平台集成的 concrete adapter 仍不能放进这里。
- 不要在没有明确评审、port/provider 设计和等价性测试的情况下，把 runtime
  执行、文件系统写入、shell/network 行为、config/path manager、AI client、
  Git service 行为、tool manifest、`ToolUseContext`、tool exposure 或
  desktop/Tauri adapter 移到这里。
- 在下游调用点被有意迁移前，用 re-export 或 wrapper facade 保持既有 core
  import path。
- 新增 feature-gated 依赖必须保持窄边界。`miniapp` 只放 MiniApp 专属依赖，
  `function-agents` 只放 function-agent 专属依赖，`product-full` 只聚合已有
  产品领域 feature 组。

## 当前归属

- `miniapp` 拥有 MiniApp DTO、compiler/bridge helper、storage/draft/import
  文件形态、fallback payload、runtime search plan、worker install 命令选择、
  lifecycle/revision 与 manager state-transition helper、host-routing string
  policy、customization metadata policy、built-in update/decline 决策、
  built-in bundle/hash/marker seed plan 与 marker wire helper、built-in
  source/placeholder payload contract、port trait，以及 storage-backed runtime
  state facade。
- `function-agents` 拥有纯 DTO、prompt template 与 assembly、commit prompt
  preparation、AI response JSON extraction 与 domain error mapping policy、
  diff truncation policy、JSON string 到领域 DTO 的解析 helper、本地文件形态分析、
  Git/AI port trait，以及 port-backed runtime facade orchestration。
- Core 仍拥有 MiniApp filesystem IO、worker process、host dispatch、built-in
  asset include/seeding、marker IO、recompile orchestration、source-hash lookup、
  `PathManager` 集成、function-agent Git/AI service adapter、AI client 调用、
  provider acquisition 和 AI transport error mapping；core 侧 product-domain
  runtime 绑定集中在 `src/crates/core/src/product_domain_runtime.rs`。

## 验证

按改动范围选择最小验证：

```bash
cargo test -p bitfun-product-domains --no-default-features
cargo test -p bitfun-product-domains --features product-full
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

仅改文档时，也运行 `git diff --check`。
