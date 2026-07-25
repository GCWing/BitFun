# bitfun-codex-adapter

**描述**: Codex 静态源码适配器。

**包名**: `bitfun-codex-adapter` | lib: `bitfun_codex_adapter`

## 核心模块

| 模块 | 说明 |
|------|------|
| `agent_source` | 子代理 Provider |
| `hook_source` | Hook Provider |
| `mcp_source` | MCP Provider |

## 关键类型/功能

- `CodexSubagentProvider` / `CodexSubagentProviderOptions`
- `CodexHookProvider` / `CodexHookProviderOptions`
- `CodexMcpProvider` / `CodexMcpProviderOptions`

## 设计要点

- 运行时无关，不执行 Codex CLI
- 解析本地 `.codex/` 目录下的配置文件
- 依赖于 `bitfun-static-hook-support` 用于有限文件读取和 Hook 解析
- 使用 TOML 格式解析 Codex 配置

## 一句话总结

从本地 Codex 配置文件中静态读取子代理、Hook 和 MCP Server 声明。
