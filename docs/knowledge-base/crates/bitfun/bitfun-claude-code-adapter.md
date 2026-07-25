# bitfun-claude-code-adapter

**描述**: Claude Code 静态源码适配器。

**包名**: `bitfun-claude-code-adapter` | lib: `bitfun_claude_code_adapter`

## 核心模块

| 模块 | 说明 |
|------|------|
| `agent_source` | 子代理 Provider |
| `command_source` | 命令 Provider |
| `hook_source` | Hook Provider |
| `mcp_source` | MCP Provider |

## 关键类型/功能

- `ClaudeCodeSubagentProvider` / `ClaudeCodeSubagentProviderOptions`
- `ClaudeCodeCommandProvider` / `ClaudeCodeCommandProviderOptions`
- `ClaudeCodeHookProvider` / `ClaudeCodeHookProviderOptions`
- `ClaudeCodeMcpProvider` / `ClaudeCodeMcpProviderOptions`

## 设计要点

- 运行时无关，不执行 Claude Code CLI
- 解析本地 `.claude/` 目录下的配置文件
- 依赖于 `bitfun-static-hook-support` 用于有限文件读取和 Hook 解析
- 使用 Markdown YAML front matter 解析 (`bitfun-services-core`)

## 一句话总结

从本地 Claude Code 配置文件中静态读取子代理、命令、Hook 和 MCP Server 声明。
