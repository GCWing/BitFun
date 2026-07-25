# tool-runtime

**路径**: src/crates/execution/tool-execution
**描述**: 工具运行时执行 crate。

## 模块

- `background_command_output` — 后台命令输出处理
- `computer_use` — 计算机使用工具
- `context` — 工具执行上下文
- `exec_command` — 命令执行
- `fs` — 文件系统操作
- `pipeline` — 工具管道
- `search` — 搜索（Grep/Glob）
- `shell` — Shell 处理（ANSI 转义等）
- `util` — 工具函数
- `web_readable` — Web 可读内容提取（feature: web-readable，依赖 htmd/legible/readability-js）
- `web_search` — Web 搜索

## 功能

工具运行时执行 crate。实现各个具体工具的执行逻辑，包括文件系统操作（Read/Write/Edit/Delete/LS）、搜索（Grep/Glob）、Shell 命令执行、Web 搜索/可读内容提取、后台命令输出处理、ANSI 转义处理等。依赖 tool-contracts 的契约定义，是工具的实际执行后端。web-readable feature 提供网页内容提取能力。
