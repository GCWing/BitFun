# Agent Hooks（生命周期钩子）

Hooks 让你在 BitFun Agent 生命周期的固定节点运行自己的命令：工具调用前后、
即将弹出权限确认时、提交提示词时、上下文压缩前后、子 Agent 启动与结束时，
以及会话与回合的开始与结束。一个 Hook 可以观察 Agent 的行为、注入模型可见的
上下文、改写工具调用参数，或者直接阻止某个动作。

## BitFun Hooks 就是 Codex Hooks

BitFun 实现的是 **Codex Hook 契约**，不是 BitFun 自己的方言：

- 同样的 `hooks.json` 文档 —— 事件、匹配组、处理器字段；
- 同样的事件名（`PreToolUse`、`PostToolUse`、`PermissionRequest`、
  `UserPromptSubmit`、`PreCompact`、`PostCompact`、`SessionStart`、
  `SessionEnd`、`SubagentStart`、`SubagentStop`、`Stop`）；
- 同样的 stdin JSON 载荷，字段名完全一致；
- 同样的退出码语义（`0` 成功、`2` 阻止且 stderr 作为原因、其他为非阻塞错误）；
- 同样的 stdout JSON 决策结构（`permissionDecision`、`updatedInput`、
  `additionalContext`、`decision`/`reason` 等）。

**Codex 的 Hook 脚本可以直接在 BitFun 中运行，反之亦然 —— 不需要做任何适配。**

因此本文不重复参考手册。事件语义、各事件的确切载荷字段、决策结构，请直接查阅
Codex 自己的文档，它把这些写得很完整：

**→ <https://learn.chatgpt.com/docs/hooks>**

本文其余部分只讲 BitFun 特有的内容：文件放在哪、怎么打开、以及目前哪里有差异。

## BitFun 从哪里读取 Hooks

Codex 读 `~/.codex/hooks.json`，BitFun 改为读自己的配置目录。文件内部结构完全相同。

| 层级 | 路径 |
| --- | --- |
| 用户 | `<用户配置目录>/config/hooks.json` |
| 项目 | `<工作区>/.bitfun/config/hooks.json` |

用户配置目录在 Linux 为 `~/.config/bitfun`，macOS 为
`~/Library/Application Support/bitfun`，Windows 为 `%APPDATA%\bitfun`。

两个层级是叠加关系：所有匹配的处理器都会执行，用户层优先，层级之间不存在覆盖或
屏蔽。修改后无需重启 BitFun。

## 开启 Hooks

**设置 → Agent Hooks**，或直接编辑 `<用户配置目录>/config/app.json` 的 `app` 段：

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
| `app.hooks.project_hooks_enabled` | `false` | 是否启用项目 Hook 文件。 |

**项目级 Hooks 默认关闭。** 项目 Hook 文件执行的是仓库中的命令，任何能提交代码的
人都可能借此在你的机器上执行代码。请只对你信任的仓库开启，并在拉取代码后重新
检查该文件。

Codex 的 `[features] hooks = false` 在 BitFun 没有对应项，请使用
`app.hooks.enabled`。

## 快速开始

创建 `<用户配置目录>/config/hooks.json`：

```json
{
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

新建一个会话，让 Agent 执行一条 shell 命令，它执行的每条命令都会追加到
`~/bitfun-commands.log`。

一个会阻止操作的 Hook —— 这里拒绝修改 `migrations/` 下的文件：

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
if "/migrations/" in payload.get("tool_input", {}).get("file_path", ""):
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "迁移文件由生成器产出，请改动 schema。",
        }
    }))
sys.exit(0)
```

## BitFun 与 Codex 的差异

未在此列出的部分，行为与 Codex 文档所述一致。

### 不支持

| Codex 能力 | BitFun |
| --- | --- |
| `config.toml` 的 `[hooks]` 表 | 不读取 —— 请把 Hook 写在 `hooks.json` |
| `[features] hooks = false` | 使用 `app.hooks.enabled` |
| 插件内置与托管 Hooks（`PLUGIN_ROOT`、`managed_dir`） | 不支持 |
| `prompt` 与 `agent` 处理器类型 | 会被解析（以便共享配置文件保持有效）但跳过 —— 只有 `type: "command"` 会执行 |
| 远程工作区 | 完全跳过 Hooks：本地 Hook 进程与远程工作区路径描述的不是同一个文件系统 |

### 尚未填充的字段

| 字段或事件 | 当前行为 |
| --- | --- |
| `transcript_path`、`agent_transcript_path` | 恒为 `null` |
| `permission_mode` | 只会是 `default` 或 `bypassPermissions` |
| `SessionStart.source` | 只有 `startup`；`resume`、`clear`、`compact` 尚未派发 |
| `SessionEnd.reason` | 恒为 `other` |
| `SubagentStop.stop_hook_active` | 恒为 `false` |
| `SubagentStop` | 仅在子 Agent 成功结束时派发；失败、取消或超时不会派发 |
| `Stop` | 仅顶层回合触发；子 Agent 回合通过 `SubagentStop` 上报 |

### 值得了解的行为

- **Hook 只能收紧权限策略，永远无法放宽。** `PreToolUse` 的
  `permissionDecision: "allow"` 只免去交互式确认；被权限规则拒绝的工具调用依然
  会被拒绝。
- `suppressOutput` 会被解析，但当前被忽略。
- `continue: false` 对 `PreToolUse` 和 `UserPromptSubmit` 生效；其他事件请使用
  `decision: "block"`。
- `PostToolUse` 在工具返回错误结果时同样会触发，不只是成功时。
- 限制：单个 `hooks.json` 最大 1 MiB；所有层级最多检查 2048 个处理器（无效处理器
  和非 `command` 处理器同样计入）；单个 Hook 的模型可见文本上限 10,000 字节，
  超出会截断。

## 安全

Hook 是以你的用户权限运行的任意代码，且每次对应事件触发都会运行。请像对待 shell
配置文件那样对待 `hooks.json`：

- 启用任何非你本人编写的 Hook 之前先审阅它。
- 除非你信任所有能向仓库提交代码的人，否则保持项目级 Hooks 关闭。
- 载荷中的值（提示词、工具参数、文件路径）是模型和用户提供的文本。请按 JSON 解析，
  不要拼接进 shell 命令 —— 上面的示例正是为此用 `jq` / `json.load` 读取字段。
- 不要在 `SessionStart`、`UserPromptSubmit`、`SubagentStart` 中把密钥打印到
  stdout，这些事件的普通 stdout 会成为模型可见的上下文。

## 排查

| 现象 | 原因 |
| --- | --- |
| 完全没有 Hook 运行 | `app.hooks.enabled` 为 `false`、文件不在文档所述路径，或工作区是远程工作区。 |
| 项目 Hooks 不运行 | `app.hooks.project_hooks_enabled` 为 `false`（默认值）。 |
| 整个文件被忽略 | JSON 无效，或存在 `description`/`hooks` 之外的根级字段。 |
| 某个事件被忽略 | 事件名拼写错误 —— 事件名区分大小写。 |
| 某个处理器从不运行 | matcher 不匹配，或 matcher 不是合法模式。matcher 是对整个值做锚定匹配的正则表达式，因此 `Bash` 匹配 `Bash` 但不匹配 `BashOutput`。 |
| `prompt`/`agent` 处理器从不运行 | 只有 `type: "command"` 处理器会执行。 |
| 阻止没有生效 | 阻止需要退出码 2（原因写入 stderr），或退出码 0 时在 stdout 输出 `decision`/`permissionDecision` 字段。 |
| 模型看不到普通 `echo` 输出 | 只有 `SessionStart`、`UserPromptSubmit`、`SubagentStart` 会把普通 stdout 转为上下文；其他事件请使用 `hookSpecificOutput.additionalContext`。 |

配置问题、非零退出、超时以及 Hook 决策都会写入 BitFun 后端日志。提升日志级别的
方法见 [`src/crates/LOGGING.md`](../../src/crates/LOGGING.md)。

## 相关

- CLI 的 `/hooks` 用于查看*其他* AI 应用（Claude Code、Codex、OpenCode）配置的
  Hooks。该视图只读，不会执行任何内容；本文描述的是 BitFun 自身的 Hooks，它们会
  真正执行。
