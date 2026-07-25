# bitfun-opencode-adapter

**描述**: OpenCode 兼容的源码和候选适配器。

**包名**: `bitfun-opencode-adapter` | lib: `bitfun_opencode_adapter`

## 核心模块

| 模块 | 说明 |
|------|------|
| `source_adapter` | OpenCode 插件包加载入口 |
| `agent_source` | 子代理 Provider |
| `command_source` | 命令 Provider |
| `hook_source` | Hook Provider |
| `mcp_source` | MCP Provider |
| `tool_source` | 工具 Provider |
| `hook_contributions` | Hook 贡献描述映射 |

## 关键类型/功能

- `load_opencode_package_adapter()` — 加载 OpenCode 插件包适配器
- `OpenCodeSubagentProvider` / `OpenCodeSubagentProviderOptions` — 子代理
- `OpenCodeCommandProvider` / `OpenCodeCommandProviderOptions` — 命令
- `OpenCodeHookProvider` / `OpenCodeHookProviderOptions` — Hook
- `OpenCodeMcpProvider` / `OpenCodeMcpProviderOptions` — MCP
- `OpenCodeToolProvider` / `OpenCodeToolProviderOptions` — 工具

## 设计要点

- 不执行 JavaScript，不安装 npm 包
- 不依赖用户本地的 `opencode` CLI
- 读取已下载的包内容，映射为 Plugin Runtime Host 适配器

## 一句话总结

解析 OpenCode 兼容插件包结构，将声明式配置映射为 Plugin Runtime 的 Provider 接口。
