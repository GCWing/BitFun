**中文** | [English](AGENTS.md)

# OpenCode Adapter

本 crate 只拥有 fixture-only OpenCode source projection 合同。它基于真实
OpenCode config 条目和本地 `.opencode/plugins/*.js|ts` 源文件事实，投影到
BitFun plugin runtime 合同用于验证；不得拥有产品策略、Host 生命周期、沙箱、
UI implementation 或 effect materialization。

## 边界规则

- 依赖 `bitfun-runtime-ports` 等稳定合同，不依赖 `bitfun-core`、app crate、
  Tauri API、产品 UI 或 concrete service manager。
- OpenCode config JSON 和本地插件源解析只能停留在本 crate 的 fixture 测试中。
  一旦引入评审后的生产 consumer，跨 crate 输出必须是 typed
  `PluginRuntimeReadResponse`、`PluginResponseEnvelope`、diagnostic、
  permission prompt 和 effect candidate。
- 未支持的 OpenCode 能力必须显式返回 diagnostic 或 typed unsupported
  candidate，不得静默忽略。
- 当前 public API budget 为空。在评审后的 Plugin Runtime Host integration 引入真实
  consumer 前，本 crate 只拥有 fixture-scoped projection 测试。
- 本 crate 可以提供私有 OpenCode config/source projector 和 contract fixture
  用于 adapter 验证，但不得实现 `PluginRuntimeClient`，不得声明 executable
  availability，也不得成为 runtime host。Product Assembly 只能通过评审后的
  Plugin Runtime Host 路径决定 host binding。
- 在 host integration PR 通过评审并移除临时边界规则前，生产 crate 不得直接导入
  `bitfun_opencode_adapter`。

## 验证

- `cargo test -p bitfun-opencode-adapter opencode_fixture_contracts`
- `node scripts/check-core-boundaries.mjs`
