# bitfun-services-integrations

**描述**: BitFun 集成服务 owner crate，通过 feature gate 组织各类外部集成。

**包名**: `bitfun-services-integrations` | lib: `bitfun_services_integrations`

## 核心模块 (feature-gated)

| 模块 | feature | 说明 |
|------|---------|------|
| `announcement` | announcement | 公告服务 |
| `browser_control` | browser-control | 浏览器控制 |
| `canvas` | canvas-runtime | Canvas 运行时 |
| `debug_log` | debug-log | 调试日志上传 |
| `deep_research` | deep-research | 深度研究功能 |
| `file_watch` | file-watch | 文件监视 |
| `function_agents` | function-agents | Function Agent |
| `git` | git | Git 操作（git2 绑定） |
| `mcp` | mcp | MCP (Model Context Protocol) 客户端 |
| `miniapp` | miniapp-runtime | 小程序运行时 |
| `plugin_source` | plugin-source | 插件源码发现 |
| `remote_connect` | remote-connect | 远程连接（加密隧道） |
| `remote_ssh` | remote-ssh | SSH 远程连接抽象 |
| `review_platform` | review-platform | 审查平台 |
| `script_tool` | script-tool-runtime | 脚本工具运行时 |
| `speech` | speech | 语音识别（sherpa-onnx） |
| `workspace_search` | workspace-search | 工作区搜索 |
| `web_tools` | web-tools | Web 工具（HTTP 请求） |

## 关键依赖

- `bitfun-agent-runtime` — 深度研究使用
- `rmcp` — MCP 协议实现
- `russh` / `russh-sftp` — SSH/SFTP
- `sherpa-onnx` — 离线语音识别
- `aes-gcm` / `x25519-dalek` — 远程连接加密
- `oxc` — Canvas 运行时 JS 解析

## 一句话总结

所有重量级外部集成放在此 crate，通过 feature 选择启用，包含 MCP、Git、SSH、语音、远程连接等。
