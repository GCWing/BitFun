**中文** | [English](AGENTS.md)

# ACP 协议入口指南

适用范围：`src/crates/interfaces/acp`。

`bitfun-acp` 负责基于已组装产品 runtime 的 Agent Client Protocol 入口与 ACP client 行为。ACP protocol / client 细节留在这里或应用入口 adapter 中；跨层只共享稳定 capability facts。

## 护栏

- Remote ACP workspace 复用本地 ACP client 配置。修改 ACP client 行为时，必须保持 manager、remote shell probing、remote capability store 和 workspace menu availability 语义。
- ACP config persistence、remote probing、timeout policy 和 workspace surface selection 属于 ACP / app-surface 行为，不要移动到 `core-types`、`runtime-ports` 或 `agent-tools`。
- ACP external-agent tool 的命名、schema、validation、presentation 和 result shape 属于 `bitfun-agent-tools` 的 portable contract；ACP 应调用这些 helper，不要在本层重复定义。
- 如果未来需要 contract，只表达观测事实：environment identity、capability facts、request / response DTO。

## 验证

```bash
cargo check -p bitfun-acp
cargo test -p bitfun-acp
```
