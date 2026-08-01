# BitFun × LoopX issue-fix 集成设计

状态：设计草案（未实现）
BitFun 基线：`eb6e9de5a17dcdfd972e8406cc25d1cf0a5996b0`
LoopX 基线：`b1c09f32`（editable 安装，`C:\codeagent\loopx`）
验证日期：2026-07-31

---

## 1. 目标与范围

给 BitFun 新增「自动修复某个仓库的 issue」能力。LoopX 只提供 `issue-fix`
这一条能力作为**决策骨架**，不引入 LoopX 的 quota / scheduler / todo 体系。

范围内：
- 枚举目标仓库的开放 issue，逐个走「判断 → 定位 → 修 → 验证 → PR」
- 每个决策点由 LoopX 投影，每个证据由 BitFun 提供
- 节奏、预算、人工门禁由 BitFun 现有 `thread_goal` 承担

范围外（本期不做）：
- LoopX 的 `quota` / `scheduler` / `todo` / 多 agent 协作
- LoopX 的 reviewer 通知、Lark 集成、metrics
- 自动 merge

---

## 2. 职责边界

四层，每层只拥有自己那部分：

| 层 | 拥有 | 不拥有 |
|---|---|---|
| BitFun `thread_goal` | 何时跑下一轮、预算、人工门禁、终止 | 单个 issue 怎么修 |
| BitFun coding agent | 读代码、定位、改码、跑验证 | 该不该发 PR |
| LoopX `issue-fix` | 路线选择、PR 生命周期投影、就绪门禁 | 任何写操作、任何代码能力 |
| BitFun `review_platform` | 抓 issue、发 PR、鉴权 | 决策 |

核心不变式：**LoopX 只做判断，不动手；BitFun 只提供证据，不自己决定路线。**

LoopX 全程 `external_writes_performed: False`。它对缺失的信息不猜测，而是标为
`unresolved_required_aspects` 并给出 `expert_next_action: read_repository_sources`
——这正是交还给 BitFun 的信号。

---

## 3. 已验证的链路

以下均在 BitFun 仓库 + 真实 issue #1849 上实测通过（只读，无副作用）。

### 3.1 主链路

```
workflow-plan
  ↓ unresolved: [change_scope, reproduction, validation]
  ↓ expert_next_action: read_repository_sources
BitFun 读代码，产出 repository_context_json
  ↓
feasibility
  ↓ repository_context_status: grounded
  ↓ route: fix_pr | comment_only | triage_only
caller-repo-branch（--execute 才动仓库）
  ↓ branch_action / branch_ready / validation_passed
  ↓ review_packet.ready
pr-lifecycle
  ↓ decision: runnable_successor | monitor_continuation | user_gate | no_followup
```

### 3.2 feasibility 的判断力（三组对照）

同一 issue，只改输入：

| 证据 + 复现 + 范围 | route | transition |
|---|---|---|
| grounded / confirmed / bounded | `fix_pr` | `runnable_successor` |
| 无 / missing / uncertain | `triage_only` | `no_followup` |
| grounded / confirmed / **oversized** | `triage_only` | `no_followup` |

第三组是关键闸门：证据齐全且可复现，但范围过大时仍拒绝发 PR。

### 3.2.1 真正的 PR 门禁是 `--validation-label`，不是 context grounding

实测（`loopx_issue_fix_contracts.rs` 两个互为对照的测试）：

| context | `--validation-label` | route |
|---|---|---|
| **grounded** | 缺失 | `triage_only` |
| **partial**（validation 未覆盖） | 已提供 | **`fix_pr`** |

结论：LoopX 区分两件事——
- **context 里的 validation source** = 「你读了哪些测试文件」，影响 `coverage.validation` 和
  `context_status`，但**不**单独决定 route
- **`--validation-label`** = 「你打算怎么验证这个修复」，**这才是发 PR 的硬门禁**

所以 BitFun 侧必须能说出验证手段（例如「web-ui focused vitest」），说不出就只能走 triage，
即使代码读得再透。反之，context 只是 partial 时仍可发 PR，只会带上
`repository_context_partial` 这条 reason code。

### 3.2.2 什么算 aspect 已 grounded

LoopX 的判定（`repository_context.py:165-169`），三个条件全满足才算：

- `freshness == "current"`（且 context 必须带 `repository_revision`）
- `trust ∈ {authoritative, verified}`
- `source_kind != "external_expert"`

`context_status` 则看 `change_scope` / `reproduction` / `validation` 三项：全 grounded →
`grounded`，部分 → `partial`，全无 → `ungrounded`。

`RepositoryContextBuilder::context_status()` 在本地复刻了这套判定，可在不启动子进程的
情况下预测结果；契约测试逐 aspect 比对两者，防止规则漂移。

### 3.3 pr-lifecycle 的四种投影

| PR 状态 | decision | state_bucket |
|---|---|---|
| OPEN + 检查通过 | `monitor_continuation` | `review_required` |
| OPEN + CHANGES_REQUESTED + 检查失败 | `runnable_successor` | `checks_failed` |
| MERGED | `no_followup` | `terminal` |
| CLOSED | `no_followup` | `terminal` |

维护者反馈（`maintainer-correction-json`）的四种 `correction_kind`：

| correction_kind | decision | role |
|---|---|---|
| `actionable_patch` | `runnable_successor` | agent |
| `semantic_ambiguity` | **`user_gate`** | user |
| `missing_authority` | **`user_gate`** | user |
| `unchanged` | `monitor_continuation` | agent |

两种 `user_gate` 是必须停下来问人的边界：**意图有歧义**、**缺写权限**。

### 3.4 修复循环本身

`repo-branch-fixture` 在临时 git repo 中跑完 branch → repro → patch → validation → PR evidence：
`ok: True`、`validated_fix_artifact_ready: true`、5 个 git 步骤逐条带 exit code。

---

## 4. 环境约束（Windows）

### 4.1 必须设 `PYTHONUTF8=1`

LoopX 包内 **123 处** `subprocess` 调用带 `text=True` 但不带 `encoding`，无一例外，
且无统一包装函数。在非 UTF-8 locale 的 Windows（本机 `cp936` / `gbk`）上，
`gh` 的 UTF-8 输出会以 GBK 解码而抛 `UnicodeDecodeError`。

`PYTHONUTF8=1` 把 `locale.getpreferredencoding()` 全局改为 UTF-8，一次覆盖全部 123 处，
零源码改动，已实测 `--fetch-metadata` 恢复正常。

**这是宿主职责**：BitFun spawn LoopX 进程时必须在 env 中带上该变量。
不要试图修改 LoopX 源码——散弹改 123 处会与上游 `git pull` 冲突。

### 4.2 已知缺陷：临时目录清理与 Windows validation 启动

`repo-branch-fixture` 的 `finally` 清理会因 git object 只读属性抛 `WinError 5`
（`acceptance_loop.py:227` 的 `_remove_temporary_git_workspace` 重试 5 次无效——
只读属性不是文件锁，等待不会改变结果）。

循环主体不受影响（实测 `ok: True`）。若 BitFun 要用这个子命令，需要：
- 要么在调用侧接受非零退出但解析已产出的 artifact
- 要么向上游提 `shutil.rmtree(onexc=...)` + `os.chmod(p, stat.S_IWRITE)` 的修复

另一个 Windows 缺陷已被 BitFun 侧修复：`caller-repo-branch --execute` 的
validation command 由 LoopX 用 `subprocess.run(shlex.split(cmd))` 启动，不带 shell。
Windows 上 `CreateProcess` 无法直接解析 `.cmd` / `.bat` shim（如 `pnpm`），报
`[WinError 2]`。BitFun 编排器在 Windows 上自动用 `cmd /c` 包装 validation command
（`orchestrator.rs` 的 `windows_safe_validation_command`），这是宿主职责，不改
LoopX 源码；2026-08-01 已在真实仓库实测通过。

**注**：memory 中记录的「pnpm WinError 2 阻塞」与 LoopX 无关——全仓仅两处提及
pnpm，均在 benchmark 的正则字符串内，不执行。该记录需更正。

---

## 5. BitFun 侧改动

### 5.1 已有、可直接复用

| 能力 | 位置 |
|---|---|
| 取单个 issue（GitHub + GitLab） | `review_platform.rs:1077` `issue()` |
| issue 证据获取 | `review_platform.rs:3876` `acquire_issue_evidence` |
| issue 指纹 | `review_platform.rs:6336` `issue_fingerprint` |
| 创建 PR | `review_platform.rs:1224`；core 层封装 `service/review_platform/mod.rs:281` |
| 鉴权（含 `GhCli`） | `review_platform.rs` `load_stored_tokens` |
| 持久化目标循环 + 自动续跑 | `thread_goal.rs:580` `continuation_after_turn` |
| 预算与状态机 | `thread_goal.rs:647` `apply_budget_status` |
| 外部二进制探测先例 | `workspace_search/service.rs:660` `which::which` |
| git 操作 | `git2`（已是依赖） |

### 5.2 已实现（后端）

**A. issue 枚举** — `review_platform.rs`，提交 `8143f8ebc`

```rust
pub async fn list_issues(
    &self,
    request: ReviewPlatformListIssuesRequest<'_>,
) -> Result<ReviewPlatformIssuePage, ReviewPlatformError>;
```

用请求结构体而非位置参数，因为同级 `issue()` 已达 clippy 参数上限。返回轻量
`ReviewPlatformIssueSummary`（不含 body 与评论），避免列举时拉取巨量数据。

provider 差异：GitHub 走 `gh` CLI、issues 端点会混入 PR（按 `pull_request` 字段过滤）、
无 Link 头故以满页推断翻页；GitLab 走 HTTP、用项目内 `iid`、无 `all` 字面量（须省略
参数）、翻页看 `x-next-page`。

**B. repository context 生成** — `loopx_issue_fix/repository_context.rs`

`RepositoryContextBuilder` 在 `push` 时逐条校验，而非 build 时一次性报错——调用方能
知道是哪条 source 有问题。已强制的 LoopX 约束：

- `source_id` 形状 `^[A-Za-z0-9][A-Za-z0-9_.:-]{0,119}$`、不可重复
- `reference` ≤260 字符、**禁绝对路径与 `..`**、URL 须 https 且无 query、
  Windows 分隔符归一化为 POSIX
- `summary` ≤220 字符、空白折叠后计数（与 LoopX 一致）
- sources ≤16 条
- `memory_retrieval` / `external_expert` 的 trust 必须 `advisory`
- `freshness: current` 必须有 `repository_revision`

另提供 `context_status()` / `ungrounded_required_aspects()`，本地复刻 LoopX 的 grounding
判定（见 3.2.2），可在不启动子进程的情况下预测结果并决定还需读什么。

**C. LoopX 进程调用层** — `loopx_issue_fix/mod.rs`，提交 `d43576b09`

```rust
impl LoopxIssueFix {
    /// None → 特性不可用，隐藏入口
    pub fn probe() -> Option<Self>;   // LOOPX_BIN 覆盖，否则 which::which("loopx")

    pub async fn issue_fix<I, S>(&self, args: I)
        -> Result<serde_json::Value, LoopxIssueFixError>;
    // 自动附加 issue-fix 前缀、--format json、env PYTHONUTF8=1
}
```

关键实现细节：LoopX 的业务拒绝**同时**输出 `{"ok": false, "error": ...}` 到 stdout
**并**以非零码退出。因此必须先解析 stdout，否则结构化原因会被裸退出码覆盖——这是
实测发现的，不是设计推断。

统一走 `--format json`，不解析 markdown。

### 5.3 挂载到 thread_goal

一个 issue 一轮。`continuation_after_turn` 已实现自动续跑与上限
（`MAX_THREAD_GOAL_AUTO_CONTINUATIONS = 100`，见 `runtime-ports/src/lib.rs:1671`）。

LoopX 的 decision 映射到 `ThreadGoalStatus`：

| LoopX decision | BitFun 行为 |
|---|---|
| `runnable_successor` | 继续本轮工作 |
| `monitor_continuation` | 本 issue 完成，转下一个 |
| `user_gate` | `ThreadGoalStatus::Blocked`，等人 |
| `no_followup` | 本 issue 终态，转下一个 |

`user_gate` → `Blocked` 是安全默认：`thread_goal_status_is_resumable()`
已允许人工恢复（`thread_goal.rs:321`）。

---

## 6. 产品形态

### 6.1 用户怎么用

**入口**：聊天头部一个图标按钮 → 右侧面板打开新页签（仿
`FlowChatHeader.tsx:883-887` 的 PR 按钮 → `createReviewPlatformTab`）。

**布局**：左列 issue 列表带勾选框，右列当前 issue 的详情与进度。

```
┌─ Issues ──────┬─ #1805 ──────────────┐
│ ☑ #1677  ✓   │ 切换轮次无法跳转        │
│ ☑ #1849  ✓   │                        │
│ ☑ #1805  ⟳   │ route: fix_pr          │
│ ☑ #1920  ⚠   │ 分支: codex/1805-fix   │
│ ☐ #1687      │ 验证: 进行中...        │
│ ☐ #1234      │                        │
│              │ 改动 3 个文件：        │
│ [全选] [开始] │  SessionsSection.tsx  │
└──────────────┴────────────────────────┘
```

**流程**：打开页签 → 自动枚举开放 issue → 用户勾选要修哪些 → 点「开始」→
串行推进，每个 issue 实时更新状态 → 遇 `user_gate` 停下等确认。

**四种行状态**，直接对应 LoopX 的 decision：

| 行状态 | 图标 | 来源 |
|---|---|---|
| 排队中 | `○` | 已勾选未开始 |
| 正在修 | `⟳` | `runnable_successor` |
| 已完成 | `✓` | `monitor_continuation` / `no_followup` |
| 等你确认 | `⚠` | **`user_gate`** |

### 6.2 复用哪些现成的东西

| 需要的 | 复用 |
|---|---|
| 逐项状态列表（勾选 + 进行中 + 完成 + 锁定） | `RemediationSelectionPanel.tsx:166-200`，两个 `Set` 驱动：`completedRemediationIds` / `fixingRemediationIds` |
| 分组、全选三态、`needs_decision` 展开选项 | 同上，`GROUP_PRIORITY_META` |
| 右侧面板页签的开启方式 | `tabUtils.ts:285-309` `createReviewPlatformTab` 的事件派发模式 |
| 面板容器与 PR 详情三页签 | `ReviewPlatformPanel.tsx`（2646 行） |
| 目标的暂停 / 恢复 / 终止 | `thread_goal` 现有状态机 |

一个 issue 映射成一个 remediation item，`RemediationSelectionPanel` 的
交互模型几乎可直接迁移，包括 `user_gate` 对应它已有的 `requiresDecision` 流程。

### 6.3 不做的

- **不加 token 预算输入框**。靠现有 `MAX_THREAD_GOAL_AUTO_CONTINUATIONS = 100`
  上限（`runtime-ports/src/lib.rs:1671`）与用户随时可暂停。BitFun 目前没有任何
  预算输入 UI——`AgentAPI.ts:573-578` 的激活接口无该参数（后端
  `create_thread_goal` 支持，但仅 agent 工具可设），本期不新增这第一个。
- **不加斜杠命令**。入口只有面板按钮一处。
- **不做 MiniApp 版本**。

### 6.4 需要新增的前端

- 新 `PanelContentType` 成员（`panels/base/types.ts:20-36` 的联合类型）
- `FlexiblePanel.tsx:821` 的 switch 分支
- 面板组件本体 + `createIssueFixTab`（仿 `tabUtils.ts:285`）
- `FlowChatHeader` 一个图标按钮
- i18n 三语言键（`scripts/i18n-contract.test.mjs` 强制校验对齐）

---

## 7. 权限与门禁

已确认的授权决定：**受限的常驻发 PR 权限**（GCWing/BitFun 为 1381 star 公开仓库）。

### 7.1 三层开关

三层互相独立，职责不同：

| 层 | 机制 | 本期决定 |
|---|---|---|
| 编译期 | Cargo feature，非 `default` | **加一个 feature**，仿 `services-integrations/Cargo.toml:96` 的 `announcement` / `browser-control`（该 crate 为 `default = []`） |
| 能力探测 | `LoopxIssueFix::probe() -> Option<Self>` | 必需。仿 `workspace_search/service.rs:660` 的 `which::which`。`None` → 隐藏头部按钮 |
| 运行时 | 用户设置 bool | **不设默认关的总开关**：feature 编进去且 probe 成功即可用 |

编译期决定「代码是否进二进制」，探测决定「运行环境是否具备」，两者都通过就可用。

### 7.2 「默认开」带来的后果

因为没有「默认关」的运行时总开关兜底，**发 PR 这一步的门禁必须落在动作本身**，
不能依赖用户没打开开关。具体要求：

- `create_pull_request`（`review_platform.rs:1224`）的调用点必须在 `feasibility`
  返回 `route: fix_pr` **且** review packet `ready: true` 时才触发；两个条件缺一不可
- 首次对某个仓库执行 `caller-repo-branch --execute` 需人工确认（会真实建分支）
- `pr-lifecycle` 返回 `user_gate` 时必须转 `ThreadGoalStatus::Blocked`，
  并在列表行上显示为 `⚠ 等你确认`，不得自动跨过
- Cargo feature 在首个版本可以先不加入 `product-full`，让代码落地但不进发布构建，
  等真实仓库验证通过再纳入

必须保持人工的：
- LoopX 返回 `user_gate` 的两种情形（意图歧义、缺权限）
- merge（本期完全不做）
- `caller-repo-branch --execute` 首次在真实仓库建分支

可自动的：
- `workflow-plan` / `feasibility` / `pr-lifecycle`（只读投影，零写入）
- 分支内的改码与验证
- 在 `feasibility` 返回 `fix_pr` 且 `review_packet.ready` 为真时发 PR

---

## 8. 尚未验证

写实现前应补的：

1. `caller-repo-branch --execute` 未在真实仓库跑过（会真的建分支）
2. `create_pull_request` 未与 LoopX 的 review packet 串联验证
3. 多 issue 串行时 `thread_goal` 的 token 记账行为
4. GitLab 路径完全未测（`map_gitlab_issue` 存在但未走过本链路）
5. `promote-discovered-issue`（agent 自己发现的缺陷）未纳入本期

---

## 9. 实现进度

后端优先，UI 最后——前四步都无外部副作用，可独立验证。

- [x] **1.** Cargo feature `loopx-issue-fix`（非 `default`，暂不在 `product-full`）
- [x] **2.** `LoopxIssueFix::probe()` + `issue_fix()`（含 `PYTHONUTF8=1`）
- [x] **3.** `list_issues()`
- [x] **4.** repository context 生成器
- [x] **5.** 单 issue 编排器（`orchestrator.rs`，类型化 outcome）
- [x] **6.** 桌面 API 暴露（core facade → Tauri 命令 → 前端绑定）
- [x] **7.** 面板 UI（`panels/issue-fix/`，头部按钮，三语言）
- [x] **8.** `thread_goal` 桥接（`thread_goal_bridge.rs`，多 issue 串行）
- [x] **9.** 真实仓库验证通过，feature 已纳入 `product-full`

### 测试覆盖

| 类型 | 数量 | 说明 |
|---|---|---|
| Rust 单元 | 53 | 含 issue 枚举映射、context 校验、编排器解析、goal 桥接、Windows validation 包装 |
| Rust 契约 | 12 | 驱动真实 loopx CLI，无 loopx 时优雅跳过 |
| Rust `#[ignore]` | 1 | 驱动真实 `gh` CLI 验证 issue 枚举 |
| 前端单元 | 29 | 行状态映射，重点是 `user_gate` 不被当作完成 |
| i18n 契约 | 37 | 三语言对齐 + 治理预算 |

### 第 9 步的验证记录（2026-08-01）

在真实公开仓库 GCWing/BitFun 上对 issue #1849 走完整链路：

1. `workflow-plan --fetch-metadata`（只读）→ `candidate_runnable: true`，零写入
2. `feasibility`（grounded context + confirmed repro + 命名 validation 面）
   → `route: fix_pr`，四个 reason code 全部满足，`decision: runnable_successor`
3. `caller-repo-branch --execute`（经授权的临时 worktree `loopx/1849-verify`）
   → 分支创建 + 真实 vitest validation 通过，`review_packet.ready: true`
4. 过程中发现并修复 Windows 缺陷：validation command 必须以 `cmd /c` 包装
   （见 4.2）

按文档 7.2 的门禁，发 PR 动作需显式授权后才执行；feature 的编译期门禁已
从「暂不入 product-full」切换为「纳入 product-full」。边界检查
（`scripts/core-boundaries/rules/feature-rules.mjs`）同步登记了
`loopx-issue-fix` 的 owner 覆盖与 product-full 组。
