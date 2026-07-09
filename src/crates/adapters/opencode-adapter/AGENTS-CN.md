**中文** | [English](AGENTS.md)

# OpenCode Adapter

本 crate 只拥有 fixture-only 的 OpenCode-compatible import projection 合同。它验证
`opencode.json`、`.opencode/plugins/*.js|ts` 和 OpenCode 全局插件目录等导入形态，
并将其投影为测试使用的 BitFun plugin runtime 合同。它不得拥有产品策略、Host 生命周期、
sandbox、UI implementation 或 effect materialization。

## 产品来源边界

- BitFun plugin package/install sources 是生产插件加载入口。OpenCode config 是可选兼容导入源，
  不是主插件注册表或运行时状态。
- 导入 `opencode.json`、`.opencode/plugins/*.js|ts` 或 OpenCode 全局插件目录时，必须先生成 typed
  import facts、候选 BitFun plugin source records、manifest、hash、diagnostics 和 trust state，
  生产 consumer 才能使用。
- 用户本机是否安装 `opencode` CLI 与加载 OpenCode-compatible 插件无关。与已安装 OpenCode binary
  的 CLI/server 互操作属于 ACP/external-client 工作，不属于本 adapter 边界。

## 边界规则

- 依赖 `bitfun-runtime-ports` 等稳定合同，不依赖 `bitfun-core`、app crate、Tauri API、产品 UI
  或 concrete service manager。
- OpenCode config JSON import、workspace plugin import 和 global plugin import parsing 只能停留在本
  crate 的 fixture 测试中。一旦引入评审后的生产 consumer，跨 crate 输出必须是 typed
  `PluginRuntimeReadResponse`、`PluginResponseEnvelope`、diagnostics、permission prompts
  和 effect candidates。
- 未支持的 OpenCode 能力必须显式返回 diagnostic 或 typed unsupported candidate，不得静默忽略。
- 当前 public API budget 为空。在评审后的 Plugin Runtime Host integration 引入真实 consumer 前，
  本 crate 只拥有 fixture-scoped projection 测试。
- 本 crate 可以提供私有 OpenCode compatibility import projectors 和 contract fixtures 用于 adapter 验证，
  但不得实现 `PluginRuntimeClient`，不得声明 executable availability，也不得成为 runtime host。
  Product Assembly 只能通过评审后的 Plugin Runtime Host 路径决定 host binding。
- 在 host integration PR 通过评审并移除临时边界规则前，生产 crate 不得直接导入
  `bitfun_opencode_adapter`。

## 验证

- `cargo test -p bitfun-opencode-adapter opencode_fixture_contracts`
- `node scripts/check-core-boundaries.mjs`
