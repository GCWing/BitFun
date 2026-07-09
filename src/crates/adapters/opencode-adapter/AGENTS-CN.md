**中文** | [English](AGENTS.md)

# OpenCode Adapter

本 crate 只拥有 projection-only 的 OpenCode-compatible 来源发现能力。它验证
`opencode.json` 和 `.opencode/plugins/*.js|ts` 等导入形态，并通过窄 Plugin Runtime Host
adapter 暴露来源事实。它不得拥有产品策略、Host 生命周期、sandbox、UI implementation
或 effect materialization。

## 产品来源边界

- BitFun plugin package/install sources 是生产插件加载入口。OpenCode config 是可选兼容导入源，
  不是主插件注册表或运行时状态。
- 导入 `opencode.json`、`.opencode/plugins/*.js|ts` 或未来 OpenCode 全局插件目录时，必须先生成 typed
  import facts、候选 BitFun plugin source records、manifest、hash、diagnostics 和 trust state，
  产品来源加载路径才能启用或执行任何内容。
- 用户本机是否安装 `opencode` CLI 与加载 OpenCode-compatible 插件无关。与已安装 OpenCode binary
  的 CLI/server 互操作属于 ACP/external-client 工作，不属于本 adapter 边界。

## 边界规则

- 依赖 `bitfun-runtime-ports` 等稳定合同和 `PluginHostAdapter` 边界 trait，不依赖
  `bitfun-core`、app crate、Tauri API、产品 UI 或 concrete service manager。
- OpenCode config JSON import 和 workspace plugin import parsing 保留在本 crate 内。跨 crate
  输出必须通过 `load_opencode_workspace_adapter` 和 Plugin Runtime Host DTO，不得把 OpenCode 原始 JSON
  或源码语法暴露为产品合同。
- 未支持的 OpenCode 能力必须显式返回 diagnostic 或 typed unsupported candidate，不得静默忽略。
- 当前 public API budget 只允许 `load_opencode_workspace_adapter`。新增公开符号必须同步预算、
  当前消费方和聚焦 host-path 测试。
- 本 crate 可以提供私有 OpenCode compatibility import projectors 和 contract fixtures 用于 adapter 验证，
  但不得实现 `PluginRuntimeClient`，不得声明 executable availability，也不得成为 runtime host。
  Product Assembly 只能通过评审后的 Plugin Runtime Host 路径决定 host binding。
- 在评审后的产品来源加载路径通过 Plugin Runtime Host 边界接入前，生产 crate 不得直接导入
  `bitfun_opencode_adapter`。

## 验证

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-opencode-adapter p0_c2_fixture`
- `node scripts/check-core-boundaries.mjs`
