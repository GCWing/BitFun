---
name: arkts-runtime-fix-cn
description: ArkTS Runtime Fix 的中文参考版本，仅用于保留中文说明，不作为主入口 skill。
---

# ArkTS Runtime Fix（中文参考）

本技能通过 **skill 私有脚本**（Node）完成证据采集与结构化解析，不再使用全局崩溃分析工具。设备枚举与日志采集优先走 `devecocli device list` / `devecocli log`。执行前若环境无 `node`，须停止并向用户说明。

## 症状与触发

在以下情况必须加载本技能：

- 日志中出现 `TypeError`、`ReferenceError`、`RangeError`、`SyntaxError`、`BusinessError` 等运行时异常
- 应用启动闪退、白屏、点击后崩溃
- 用户提供 `jscrash` 日志、堆栈、`@file` 临时日志文件
- 构建成功但运行期立即失败

## 证据优先约束

在已拿到以下锚点之前，避免对工程做大规模 `Read` / `Glob` / `Explore`：

- `error_type`
- `error_message`
- `suspected_file`
- `top_stack`
- 或者用户明确指出的崩溃页面 / 模块

在证据不足时，可以执行本节的私有脚本、向用户追问复现与包名、或读取用户已显式给出的单个日志文件路径。

一旦锚点齐备，再定向读取疑似 `ets/ts/js` 与相关导航入口，做最小修改。

## 私有脚本执行约定

所有脚本通过 Shell 执行，形式与 `deveco-create-project` 一致：

```bash
node "{SKILL_DIR}/scripts/<script>.mjs" ...
```

`{SKILL_DIR}` 由执行环境替换为当前技能根目录的绝对路径。脚本 `stdout` 采用稳定 `key: value` 行文本；退出码非 0 表示当前步骤无法继续，应向用户报告 `next_action` 中的原因并停止。

## 场景 A：用户已提供原始日志文本

```bash
node "{SKILL_DIR}/scripts/jscrash-report.mjs" --log-text "{crashLog}" --bundle-name "{bundleName}" --include-text
```

## 场景 B：用户提供 `@file` 或本地日志路径

```bash
node "{SKILL_DIR}/scripts/parse-jscrash-log.mjs" --log-file "{logFilePath}" --bundle-name "{bundleName}" --include-text
```

## 场景 C：仅有症状，无日志

设备采证前必须先读取 `AppScope/app.json5` 中的 `app.bundleName`，作为后续 `--bundle-name` 的精确值；禁止用 `vendor`、`com.example` 或模块目录名猜测。

先探测 faultlogger 近期 `jscrash-*.log`：

```bash
node "{SKILL_DIR}/scripts/probe-faultlogger.mjs" --bundle-name "{bundleName}" --device-id "{deviceId}" --max-age-minutes "30" --limit "10"
```

- 若输出 `status: found`：读取 `latest_faultlog` 或 `candidates`，再拉取并解析：

```bash
node "{SKILL_DIR}/scripts/fetch-faultlog.mjs" --faultlog-name "{latestFaultlog}" --device-id "{deviceId}" --output-dir "{tempDir}"
node "{SKILL_DIR}/scripts/parse-jscrash-log.mjs" --log-file "{localFaultlogPath}" --bundle-name "{bundleName}" --source file --include-text
```

- 若输出 `status: not_found` 或 `status: probe_failed`：可按需进入基于 `devecocli log` 的 hilog 兜底采集：

```bash
node "{SKILL_DIR}/scripts/collect-hilog.mjs" --device-id "{deviceId}" --lines "4000" --output-dir "{tempDir}"
node "{SKILL_DIR}/scripts/parse-jscrash-log.mjs" --log-file "{hilogPathFromCollect}" --bundle-name "{bundleName}" --source hilog --include-text
```

## 解析输出字段契约

`jscrash-report.mjs` 和 `parse-jscrash-log.mjs` 首段 `key: value` 行包含：

- `status`: `detected` | `no_crash_signature` | `parse_failed`
- `source`: `file` | `text` | `hilog` 等
- `error_type`
- `error_message`
- `suspected_file`
- `top_stack`: 以 `|` 分隔的帧列表
- `keywords`: 逗号分隔
- `next_action`

`--include-text` 会在其后追加人类可读完整报告。

若 `status: no_crash_signature`，须向用户说明证据不足，请求复现或更完整日志后再读代码。

## 常见崩溃签名

| 签名 | 常见原因 | 优先修复方向 |
|---|---|---|
| `TypeError` 属性访问 | 渲染 / 生命周期空状态 | 空值保护、提前初始化、调整生命周期 |
| `ReferenceError` | 作用域 / 导入 / 闭包 | 修正符号、导入路径或回调捕获 |
| `RangeError` | 下标 / 递归 / 长度 | 边界检查、断环、限制索引 |
| `BusinessError` / `ParameterError` | API 前置条件 | 校验参数、权限与调用时机 |

## 解释规则

- 应用栈优先于框架噪声；首个具体 `.ets/.ts/.js` 路径是起点而非唯一根因。
- 若用户给出复现步骤，步骤优先于单纯堆栈推断。
- 若堆栈指向非入口页，默认视为交互触发路径，除非证据表明仅冷启动。
- 不做大范围重构，先消除崩溃路径。

## 交互建议

1. 先说明当前已有的证据。
2. 如果证据不足，再说明是否按需检查 faultlogger 或 hilog。
3. 一旦拿到锚点，就切换到定向读代码和最小修复。

## 约束

- 不得仅凭推理声称已修复崩溃。
- 不得用重试、随意延时或大面积防御性改写替代根因修复。
- 涉及不熟悉的 `@ohos.*` / `@kit.*` API 时，应先查证约束再改代码。
- 本技能不决定最终的编译 / 运行 / 验证顺序；主 Agent 负责后续验证。
