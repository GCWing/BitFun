**中文** | [English](AGENTS.md)

# ACP Agent 指南

适用范围：`src/crates/acp`。

`bitfun-acp` 负责 Agent Client Protocol 集成和 ACP client 行为。ACP protocol /
client 细节应留在这里或 app surface adapter；contract crate 只共享稳定 capability facts。

## 护栏

- Remote ACP workspace 复用本地 ACP client config。修改 ACP client 行为时，必须保留
  manager、remote shell probing、remote capability store 与 workspace menu availability 语义。
- ACP config persistence、remote probing、timeout policy 和 workspace surface selection
  属于 ACP / app surface 行为，不要下沉到 `core-types`、`runtime-ports` 或 `agent-tools`。
- 如果后续需要 contract，只记录 observational 信息：environment identity、capability facts
  与 request/response DTO。

## 验证

```bash
cargo check -p bitfun-acp
cargo test -p bitfun-acp
```
