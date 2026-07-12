# Test Selection Gate（测试选择硬保障）设计

## 背景与目标

SWE-bench Pro run4 失败聚类：114 个「差一点」case 中 35 个的模式是 **agent 跑了测试、
但没跑到与自己改动相关的那个测试**，收尾时带着未验证的改动交卷。prompt 条款
（test discovery 规则已存在于 agentic_mode.md）被验证推不动模型行为，需要 scaffold
级的确定性保障。

目标：agent 即将结束回合时，若本会话修改过源码文件、但相关测试从未被运行，
**注入一次结构化 system reminder**（列出文件 → 建议命令），把结束推迟一轮。
不硬阻塞、不代跑测试、每回合最多触发一次。

## 核心机制

### 1. 会话内追踪（新组件 `TestSelectionGate`）

- 新文件 `src/crates/assembly/core/src/agentic/execution/test_selection_gate.rs`
- 在 execution loop 的 round 处理中收集（engine 已能看到全部 tool call）：
  - 成功的 `Edit`/`Write`/`MultiEdit` 的目标路径 → `edited_files: HashSet<PathBuf>`
  - 成功的 `Bash` 命令原文 → `bash_commands: Vec<String>`
- 状态挂在 engine 的 turn 级上下文，跨 round 存活，turn 结束丢弃。

### 2. 判定时机

`execution_engine.rs` 中模型返回无 tool call、即将标记 `has_final_response = true`
的位置（现有 repeated-tool-failure 拦截的同层）：

```
if gate.enabled() && !gate.reminded_this_turn() {
    if let Some(msg) = gate.build_reminder(&workspace_root) {
        inject internal_reminder(InternalReminderKind::TestSelectionGate, msg);
        continue;   // 多给一轮
    }
}
// 第二次到达此处必然放行 —— 防死循环
```

`InternalReminderKind` 新增 `TestSelectionGate` 变体。

### 3. 测试发现（纯静态启发式，不解析 AST）

**范围口径与 agentic_mode.md 既有验证规则严格一致**（"Scope the verifier to what
you changed" / "for each modified foo.py find test_foo.py, foo_test.go…run at the
package level"）——建议命令只覆盖**直接改动文件本身**映射到的测试，绝不扩大：

| 语言 | 改动文件 | 候选目标（建议命令） |
|---|---|---|
| Go | `pkg/x/y.go` | 仅当 `pkg/x/*_test.go` 存在 → `go test ./pkg/x/`（单包；**绝不** `./...`，不含依赖包——大仓全量编译会拖爆 agent 时长） |
| Python | `a/foo.py` | glob `**/test_foo.py`、`**/foo_test.py`；命中 → `pytest <具体文件>` |
| JS/TS | `src/foo.ts(x)` | `foo.test.*` / `foo.spec.*` / `__tests__/foo*` → 跑具体文件 |
| Rust | `crates/c/src/*.rs` | `cargo test -p <crate>`（单 crate） |

改动文件本身是测试文件（`_test.go`/`test_*.py`/`*.test.ts` 等）→ 它自己就是目标。
找不到映射的文件不产生要求（宁漏勿噪）。方向不对称：**建议从窄**（防超时），
**覆盖判定从宽**（agent 自己跑了更大范围也算达标，见 §4）。

### 4. 覆盖判定（宽松，宁放过不误拦）

目标视为「已运行」若任一 bash 命令同时满足：

1. 含测试运行器关键词：`go test` / `pytest` / `python -m pytest` / `jest` / `vitest`
   / `mocha` / `npm test` / `yarn test` / `cargo test` / `go vet`
2. 且（命令文本命中目标的 路径/文件名/包前缀）**或** 命令是全量跑
   （`./...`、`--workspace`、无路径参数的裸 `pytest`/`npm test` 等）

### 5. 提醒文案（一次性，最多列 5 条）

```
<system_reminder>
You edited the following files but never ran their associated tests this session:
- pkg/x/y.go  →  go test ./pkg/x/
- src/foo.ts  →  npx jest src/foo.test.ts
Run them now and fix any failures before finishing. If a listed test is genuinely
not applicable, you may finish without running it.
</system_reminder>
```

保留 agent 的最终判断权（明确允许其说明不适用后结束），避免死循环和误拦。

### 6. 启用范围（通用产品行为，无开关）

- **默认开启**，对所有用户生效——这是通用的质量保障，不是评测专用逻辑。
- 仅在可写编码模式（agentic）生效；Plan 等只读模式天然不触发（无成功 Edit）。
- 交互场景的噪声由既有设计压制：只对「能映射到已存在测试文件」的改动提醒、
  每回合最多一次、文案允许 agent 判断不适用后直接结束。
- 效果度量在评测侧完成：从 trial 的 agent trajectory 中检索提醒文案标记，
  统计触发数与提醒后补跑数，不在产品内埋遥测。

## 明确不做（防过度工程）

- 不做 AST/依赖图分析——文件名启发式覆盖四种主语言即可
- 不代跑测试、不把结果贴回（若提醒被普遍无视，这是第二迭代的升级方向）
- 不硬阻塞结束、不多次提醒
- 不改 prompt 文本

## 改动面

| 位置 | 内容 | 规模 |
|---|---|---|
| `execution/test_selection_gate.rs`（新） | tracker + 发现 + 覆盖判定 + 文案 | ~200 行 |
| `execution/execution_engine.rs` | 收集点 + gate 判定/注入 | ~40 行 |
| messages（InternalReminderKind） | 新变体 | ~5 行 |
| 单测 | 发现规则表驱动 + gate 状态机（含二次放行） | ~150 行 |

## 验证计划

验证集**换用 gate 专属集（57 case）**，不复用 prompt 修复的 75 集
（其中 50 个契约 case 本特性无法解决，跑了只烧机时稀释信号）：

- **目标组 37**：run4 中 agent「跑了测试但没跑到与改动相关的目标测试」35 个 +
  「完全没跑」2 个（名单存 memory：gate-validation-set）
- **回归锚点 20**：复用原有 10 全过 + 10 run4 过

步骤：
1. 单测 + 本地 smoke（改文件不跑测试 → 应见提醒；跑了 → 不应触发）
2. 重编 musl 二进制（记 commit），重跑 57-case 集
3. 三个数：目标组 37 的通过数变化（主指标）、提醒触发/被采纳率（评测侧从
   trajectory 检索）、回归锚点不得下降（allpass 组必须 10/10）
4. 无效 → revert 单个 commit + 重编回退

## 风险

- **提醒被无视**：与泛泛 prompt 不同，这是带具体命令的定向单次提醒；若验证显示
  无视率高，升级为 gate 代跑测试并贴回结果（第二迭代）
- **误报打扰**：靠宽松覆盖判定 + 每 turn 一次上限压制
- **token/时长成本**：多一轮测试运行，单 trial 增量有限；评测 agent 超时 3000s 余量充足
