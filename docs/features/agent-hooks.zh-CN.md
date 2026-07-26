# Agent Hooks（生命周期钩子）

Hooks 让你在 BitFun Agent 生命周期的固定节点运行自己的命令：工具调用前后、
即将弹出权限确认时、提交提示词时、上下文压缩前后、子 Agent 启动与结束时，
以及会话与回合的开始与结束。一个 Hook 可以观察 Agent 的行为、注入模型可见的
上下文、改写工具调用参数，或者直接阻止某个动作。

`hooks.json` 文档结构、事件名、stdin 上的 JSON 载荷、退出码语义以及 stdout 上的
JSON 决策结构**与 Codex Hooks 一致**，因此 Codex 的 Hook 脚本可以直接在 BitFun
中运行，反之亦然。差异在于文件位置和部分扩展能力：

| | Codex | BitFun |
| --- | --- | --- |
| 用户 Hooks | `~/.codex/hooks.json` | `<用户配置目录>/config/hooks.json` |
| 项目 Hooks | `<仓库>/.codex/hooks.json` | `<工作区>/.bitfun/config/hooks.json` |
| 主配置内联 Hooks | `config.toml` 的 `[hooks]` 表 | 不支持 —— 请使用 `hooks.json` |
| 总开关 | `[features] hooks = false` | `app.json` 中的 `app.hooks.enabled` |
| 插件内置与托管 Hooks | 支持 | 不支持 |

部分载荷字段目前也尚未填充，见[当前差距](#当前差距)。

> 适用范围：本地工作区。远程 SSH 与容器工作区会跳过 Hooks，因为本地 Hook 进程与
> 远程工作区路径描述的并不是同一个文件系统。
>
> 非目标：`prompt` 与 `agent` 处理器类型。BitFun 会解析它们（以便共享的配置文件
> 保持有效），但只执行 `type: "command"` 处理器。

## 快速开始

1. 在用户配置目录下创建 `hooks.json`：

   | 平台 | 路径 |
   | --- | --- |
   | Linux | `~/.config/bitfun/config/hooks.json` |
   | macOS | `~/Library/Application Support/bitfun/config/hooks.json` |
   | Windows | `%APPDATA%\bitfun\config\hooks.json` |

2. 添加一个记录 Agent 执行过的所有 shell 命令的 Hook：

   ```json
   {
     "description": "My hooks",
     "hooks": {
       "PreToolUse": [
         {
           "matcher": "Bash",
           "hooks": [
             {
               "type": "command",
               "command": "jq -r '.tool_input.command' >> ~/bitfun-commands.log"
             }
           ]
         }
       ]
     }
   }
   ```

3. 新建一个 BitFun 会话，让 Agent 执行一条 shell 命令。它执行的每条命令都会追加到
   `~/bitfun-commands.log`。

配置文件变化后会自动重新读取，编辑 `hooks.json` 之后无需重启 BitFun。

## 配置

### 文件位置与分层

Hooks 按以下顺序从两个层级读取：

| 顺序 | 层级 | 路径 |
| --- | --- | --- |
| 1 | 用户 | `<用户配置目录>/config/hooks.json`（见上表） |
| 2 | 项目 | `<工作区>/.bitfun/config/hooks.json` |

两个层级是叠加关系：两层中所有匹配的处理器都会执行，用户层优先。层级之间不存在
覆盖或屏蔽关系。

**项目级 Hooks 默认关闭。** 项目 Hook 文件执行的是仓库中的命令，任何能提交代码的
人都可能借此在你的机器上执行代码。只对你信任的工作区开启，在
`<用户配置目录>/config/app.json` 的 `app` 段中设置：

```json
{
  "app": {
    "hooks": {
      "enabled": true,
      "project_hooks_enabled": true
    }
  }
}
```

| 配置项 | 默认值 | 含义 |
| --- | --- | --- |
| `app.hooks.enabled` | `true` | 总开关。`false` 会禁用所有 Hooks。 |
| `app.hooks.project_hooks_enabled` | `false` | 是否启用 `<工作区>/.bitfun/config/hooks.json`。 |

### 文档结构

```json
{
  "description": "可选，自由文本",
  "hooks": {
    "<事件名>": [
      {
        "matcher": "可选的匹配模式",
        "hooks": [
          {
            "type": "command",
            "command": "要执行的命令",
            "commandWindows": "可选的 Windows 覆盖命令",
            "timeout": 30,
            "statusMessage": "可选的说明文本"
          }
        ]
      }
    ]
  }
}
```

根对象只接受 `description` 和 `hooks`。出现其他根级字段会导致整个文件被拒绝 —— 这
与 Codex 行为一致，可以尽早暴露拼写错误，而不是静默忽略你的配置。

每个事件对应一组**匹配组**。每个匹配组包含可选的 `matcher` 和必需的处理器数组
`hooks`。

#### 处理器字段

| 字段 | 必需 | 含义 |
| --- | --- | --- |
| `type` | 是 | 必须为 `"command"` 才会执行。`"prompt"` 和 `"agent"` 会被接受但跳过。 |
| `command` | 是 | 通过 `sh -c`（Unix）或 `cmd /C`（Windows）执行，工作目录为工作区根目录。 |
| `commandWindows` | 否 | 在 Windows 上代替 `command` 使用。 |
| `timeout` | 否 | 单位为秒。默认 600（`SessionEnd` 默认 1，上限 3）。 |
| `statusMessage` | 否 | 简短说明该 Hook 的用途。 |

#### 匹配规则

matcher 根据事件对应的一个值筛选要执行的处理器（见事件表）。matcher 是对整个值做
锚定匹配的正则表达式，因此直接写名字就是精确匹配。

| matcher | 匹配范围 |
| --- | --- |
| 省略、`""` 或 `"*"` | 全部匹配 |
| `"Bash"` | 精确匹配 `Bash`（不匹配 `BashOutput`） |
| `"^Bash$"` | 精确匹配 `Bash` |
| `"Edit\|Write"` | `Edit` 或 `Write` |
| `"mcp__filesystem__.*"` | 该 MCP 服务下的所有工具 |
| `"startup"` | 对应的 `SessionStart` 来源（见[当前差距](#当前差距)） |

无效的 matcher（不是合法模式，或不是字符串）不会匹配任何内容，并会记录到日志。
它不会被当作"匹配全部"处理。

### 限制

| 限制项 | 取值 |
| --- | --- |
| 单个 `hooks.json` 最大体积 | 1 MiB |
| 所有层级可检查处理器总数上限 | 2048（无效处理器和非 `command` 处理器同样计入） |
| 单个 Hook 的模型可见文本上限 | 10,000 字节（超出会截断并标记） |

## 事件

| 事件 | 触发时机 | matcher 取值 | 能否阻止？ |
| --- | --- | --- | --- |
| `SessionStart` | 创建会话时 | `source`（当前恒为 `startup`） | 否 |
| `SessionEnd` | 删除会话时 | — | 否 |
| `UserPromptSubmit` | 提交提示词后、回合开始前 | — | 可以 —— 拒绝该提示词 |
| `PreToolUse` | 工具调用被排入执行前，早于权限评估与工具运行 | `tool_name` | 可以 —— 拒绝该工具调用 |
| `PermissionRequest` | 工具调用即将向你请求权限时 | `tool_name` | 可以 —— 直接放行或拒绝，不再弹窗 |
| `PostToolUse` | 工具调用返回结果后（成功或报错都会触发） | `tool_name` | 仅反馈（追加到工具结果中） |
| `PreCompact` | 上下文压缩之前 | `trigger`（`auto`、`manual`） | 否 |
| `PostCompact` | 上下文压缩之后 | `trigger` | 否 |
| `SubagentStart` | 子 Agent 回合开始时 | `agent_type` | 否 |
| `SubagentStop` | 子 Agent 回合成功结束时 | `agent_type` | 仅记录到日志 |
| `Stop` | 顶层回合即将以最终回答结束时 | — | 可以 —— 重新打开该回合（最多 3 次） |

未知事件名会被忽略并记录警告，文件其余部分仍然生效。

### 当前差距

以下是本版本与 Codex 契约的真实差异。它们会影响 Hook 能依赖的内容，因此明确列出
而非含糊带过：

| 字段或事件 | 当前行为 |
| --- | --- |
| `transcript_path`、`agent_transcript_path` | 恒为 `null` —— 会话转录尚未开放给 Hook |
| `permission_mode` | 只会是 `default` 或 `bypassPermissions`，其余 Codex 模式不会出现 |
| `SessionStart.source` | 只有 `startup`；`resume`、`clear`、`compact` 尚未派发 |
| `SessionEnd.reason` | 恒为 `other` |
| `SubagentStop.stop_hook_active` | 恒为 `false` —— `SubagentStop` 不会重新打开子 Agent 回合 |
| `SubagentStop` | 仅在子 Agent 成功结束时派发；失败、取消或超时不会派发 |
| `Stop` | 仅顶层回合触发；子 Agent 回合通过 `SubagentStop` 上报 |

## Hook 进程接口

### 输入：stdin 上的 JSON

每个载荷都包含以下字段：

```json
{
  "session_id": "string",
  "transcript_path": "string 或 null",
  "cwd": "string",
  "hook_event_name": "string",
  "model": "string",
  "permission_mode": "default | acceptEdits | plan | dontAsk | bypassPermissions"
}
```

除 `SessionStart` 和 `SessionEnd` 之外的事件还会带上 `turn_id`。各事件另有自己的
字段：

| 事件 | 附加字段 |
| --- | --- |
| `SessionStart` | `source` |
| `SessionEnd` | `reason` |
| `UserPromptSubmit` | `prompt` |
| `PreToolUse` | `tool_name`、`tool_use_id`、`tool_input` |
| `PermissionRequest` | `tool_name`、`tool_input` |
| `PostToolUse` | `tool_name`、`tool_use_id`、`tool_input`、`tool_response` |
| `PreCompact` / `PostCompact` | `trigger` |
| `SubagentStart` | `agent_id`、`agent_type` |
| `SubagentStop` | `agent_id`、`agent_type`、`agent_transcript_path`、`stop_hook_active`、`last_assistant_message` |
| `Stop` | `stop_hook_active`、`last_assistant_message` |

当 Agent 已经因为某个 `Stop` Hook 的阻止而继续运行时，`stop_hook_active` 为
`true`。检查这个字段可以避免无限阻止；BitFun 本身也会将同一回合的重新打开次数
限制为 3 次。

### 输出：退出码，以及可选的 stdout JSON

| 退出码 | 含义 |
| --- | --- |
| `0` | 成功。若 stdout 能解析为 JSON，则按决策文档解读（见下）。 |
| `2` | 阻止该事件。**stderr** 的内容作为原因传给 Agent。 |
| 其他 | 非阻塞错误：记录警告，Agent 继续运行。 |

启动失败或超时的 Hook 同样只产生非阻塞警告 —— 出问题的 Hook 会让 Agent 变慢，
但不会把它卡死。

对 `SessionStart`、`UserPromptSubmit`、`SubagentStart` 而言，退出码为 0 时非 JSON
的普通 stdout 会成为模型可见的上下文。其他事件会忽略普通 stdout，因此用 `echo`
调试不会泄漏进对话。

#### 决策文档

所有字段均为可选：

```json
{
  "continue": true,
  "stopReason": "continue 为 false 时展示",
  "systemMessage": "记录给你看，不发送给模型",
  "suppressOutput": false,
  "decision": "block",
  "reason": "被阻止的原因",
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow | deny",
    "permissionDecisionReason": "string",
    "updatedInput": { "command": "改写后的参数" },
    "additionalContext": "模型会读到的文本",
    "decision": { "behavior": "allow | deny", "message": "string" }
  }
}
```

各事件生效的字段：

| 事件 | 生效字段 |
| --- | --- |
| `PreToolUse` | `permissionDecision`（`allow` 跳过权限确认，`deny` 阻止调用）、`permissionDecisionReason`、`updatedInput`，以及 `continue: false`（按拒绝该工具调用处理） |
| `PermissionRequest` | `decision.behavior` 及 `decision.message` |
| `PostToolUse` | `decision: "block"` 加 `reason`，以及 `additionalContext` —— 两者都会追加到模型读到的工具结果中 |
| `UserPromptSubmit` | `decision: "block"` 加 `reason`（拒绝该提示词）、`additionalContext`，以及 `continue: false`（同样拒绝该提示词） |
| `Stop` / `SubagentStop` | `decision: "block"` 加 `reason` |
| 任意事件 | `systemMessage`（写入 BitFun 日志，不会发送给模型） |

`permissionDecision: "allow"` 只会免去交互式权限确认。被权限规则拒绝的工具调用
依然会被拒绝 —— Hook 只能收紧权限策略，永远无法放宽。

以下两个字段为兼容 Codex 而被接受，但当前除上表所列外没有实际作用：
`suppressOutput` 会被解析但忽略；`continue`/`stopReason` 仅对 `PreToolUse` 和
`UserPromptSubmit` 生效（其他事件请使用 `decision: "block"` 来阻止）。

当一个事件匹配到多个处理器时，它们按顺序执行（用户层优先），结果会合并：第一个
阻止或拒绝的决策会终止其余处理器；`deny` 覆盖此前的 `allow`；后出现的
`updatedInput` 替换先前的；上下文与系统消息会累加。

## 示例

### 阻止修改受保护路径

`PreToolUse` 返回拒绝决策。Agent 会得知原因，并在没有该编辑的情况下继续回合。

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [{ "type": "command", "command": "python3 ~/hooks/protect.py" }]
      }
    ]
  }
}
```

```python
#!/usr/bin/env python3
import json, sys

payload = json.load(sys.stdin)
path = payload.get("tool_input", {}).get("file_path", "")

if "/migrations/" in path:
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "迁移文件由生成器产出，请改动 schema。",
        }
    }))
sys.exit(0)
```

### 每次写入后格式化并回报

`PostToolUse` 返回模型可读的反馈。注意 `PostToolUse` 会忽略普通 stdout 和
stderr —— 反馈必须通过 JSON 决策文档中的 `additionalContext` 给出，因此这里用
一个小脚本而不是一行命令。

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/hooks/format.sh",
            "timeout": 60,
            "statusMessage": "Formatting Rust code"
          }
        ]
      }
    ]
  }
}
```

```bash
#!/usr/bin/env bash
# ~/hooks/format.sh
cat >/dev/null                       # 读掉载荷
output=$(cargo fmt 2>&1 | head -20)
[ -z "$output" ] && exit 0           # 没有需要回报的内容
jq -n --arg ctx "cargo fmt output:\n$output" \
  '{hookSpecificOutput: {hookEventName: "PostToolUse", additionalContext: $ctx}}'
```

### 测试通过后才允许结束

`Stop` 通过退出码 2 阻止结束，stderr 文本作为原因，Agent 会继续工作而不是收尾。

```bash
#!/usr/bin/env bash
# ~/hooks/require-tests.sh
payload=$(cat)
if [ "$(printf '%s' "$payload" | jq -r '.stop_hook_active')" = "true" ]; then
  exit 0        # 已经重新打开过一次，不要循环
fi
if ! cargo test --quiet >/tmp/hook-tests.log 2>&1; then
  echo "测试未通过。请查看 /tmp/hook-tests.log，修复后再结束。" >&2
  exit 2
fi
```

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "bash ~/hooks/require-tests.sh", "timeout": 300 }
        ]
      }
    ]
  }
}
```

### 会话开始时注入项目上下文

`SessionStart` 的普通 stdout 会成为模型可见的上下文。

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "git log --oneline -5 && git status --short" }
        ]
      }
    ]
  }
}
```

### 自动放行安全工具，不再弹窗

`PermissionRequest` 代替你做决定。

```json
{
  "hooks": {
    "PermissionRequest": [
      {
        "matcher": "Read",
        "hooks": [
          {
            "type": "command",
            "command": "printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PermissionRequest\",\"decision\":{\"behavior\":\"allow\",\"message\":\"Reads are always allowed\"}}}'"
          }
        ]
      }
    ]
  }
}
```

## 安全

Hook 是以你的用户权限运行的任意代码，且每次对应事件触发都会运行。请像对待 shell
配置文件那样对待 `hooks.json`：

- 启用任何非你本人编写的 Hook 之前先审阅它。
- 除非你信任所有能向仓库提交代码的人，否则保持 `app.hooks.project_hooks_enabled`
  关闭；拉取代码后重新检查项目 Hook 文件。
- 载荷中的值（提示词、工具参数、文件路径）是模型和用户提供的文本。请按 JSON 解析，
  不要拼接进 shell 命令 —— 上面的示例正是为此用 `jq` / `json.load` 读取字段。
- Hook 继承 BitFun 的环境变量。对于上下文类事件，不要把密钥打印到 stdout，因为这些
  文本会发送给模型。

## 排查

| 现象 | 原因 |
| --- | --- |
| 完全没有 Hook 运行 | `app.hooks.enabled` 为 `false`、文件不在文档所述路径，或工作区是远程工作区。 |
| 项目 Hooks 不运行 | `app.hooks.project_hooks_enabled` 为 `false`（默认值）。 |
| 整个文件被忽略 | JSON 无效，或存在 `description`/`hooks` 之外的根级字段。 |
| 某个事件被忽略 | 事件名拼写错误 —— 事件名区分大小写。 |
| 某个处理器从不运行 | matcher 不匹配，或 matcher 不是合法模式。 |
| `prompt`/`agent` 处理器从不运行 | 只有 `type: "command"` 处理器会执行。 |
| 阻止没有生效 | 阻止需要退出码 2（原因写入 stderr），或退出码 0 时在 stdout 输出 `decision`/`permissionDecision` 字段。 |
| 模型看不到普通 `echo` 输出 | 只有 `SessionStart`、`UserPromptSubmit`、`SubagentStart` 会把普通 stdout 转为上下文；其他事件请使用 `hookSpecificOutput.additionalContext`。 |

配置问题、非零退出、超时以及每个 Hook 决策都会写入 BitFun 后端日志。提升日志级别的
方法见 [`src/crates/LOGGING.md`](../../src/crates/LOGGING.md)。

## 相关

- CLI 中的 [`/hooks`](../../src/apps/cli/src/modes/chat/external_hooks.rs) 用于查看
  *其他* AI 应用（Claude Code、Codex、OpenCode）配置的 Hooks。该视图只读，不会执行
  任何内容；本文描述的是 BitFun 自身的 Hooks，它们会真正执行。
