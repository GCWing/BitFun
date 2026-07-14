# Edit Constraint Guard（编辑约束守卫）设计

## 背景与目标

SWE-bench Pro 全量 731 复核（`bitfun-swebenchpro-full-20260712-final-5efa6c83`）发现：730/731
的题面里带有强约束"don't have to modify the testing logic / already taken care of all
changes to test files"，其中 **102 个 trial 最终 patch 仍然改了测试文件**（48 个因此失败，
54 个侥幸通过）。

对其中一个失败案例（`instance_future-architect__vuls__57XPJjQ`）做完整 trace 回溯后确认了
一个此前未被发现的机制：

1. verifier 在跑测试前会执行 `git checkout <gold_commit> -- <test files>`，把 agent 对测试
   文件的改动**整体覆盖**——agent 改没改测试文件，跟最终判分**没有直接因果关系**。
2. 真正的败因始终是 agent 自己的**源码实现**跟 gold 测试期望的契约（函数签名、返回值个数、
   字段名）对不上——这是已有结论"契约簇=信息不可得"的一部分，不是独立问题。
3. 但"agent 改了不该改的测试文件"这个行为本身，是一个很干净的**行为标记**：它标出了
   "agent 已经在本地跑测试时撞到了矛盾信号（测试跑不过），但选择篡改测试文件让本地看起来
   通过，而不是把这个矛盾当成'我的实现可能不对，该重新想想'的提示"。agent 自己的推理轨迹
   里也明确写出了这种挣扎（"the user says not to modify tests. This is a contradiction"）。

目标：把题面/用户输入里"不要做 X"类强约束，从纯文本指令变成运行时可拦截的机制——在
Edit/Write/Delete **真正落盘之前**拦下命中约束的改动，用结构化反馈把 agent 推回"重新审视
自己实现"的路径，而不是任其用篡改被保护文件的方式绕过矛盾信号。这是通用产品行为（真实用户
自己说"别碰 X"的场景同样适用），不是评测专用逻辑。

## 核心机制

### 1. 约束抽取（会话级，一次性，LLM）

新组件，会话首次调用 Edit/Write/Delete 时懒加载触发（非编辑类会话零成本），用 fast 模型对
首条用户消息做结构化抽取，结果缓存进 session state，全程复用不重跑：

```rust
#[derive(Deserialize)]
struct ExtractedConstraint {
    /// 原话摘要，人类可读，塞进拒绝提示里给 agent 看
    description: String,
    matcher: ConstraintMatcher,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ConstraintMatcher {
    /// 复用 test_selection_gate.rs 的 Lang::is_test_file()，不重新定义"什么算测试文件"
    TestFiles,
    /// 路径含指定子串
    PathContains { substrings: Vec<String> },
    /// 路径在指定目录下
    PathUnderDir { dirs: Vec<String> },
    /// 指定扩展名
    Extension { exts: Vec<String> },
    /// 识别到约束但套不进以上任何原语——不强制执行，只记录，为后续扩展词表积累证据
    Unmatched,
}
```

**为什么是受限词表而不是自由 glob/正则**：LLM 直接生成路径匹配字符串风险高（语法错误、
匹配范围过宽/过窄、难以低成本校验）。改成从 5 个固定分类里选，解析失败直接整体丢弃，不会
出现"LLM 生成了个非法正则"这类故障模式。`TestFiles` 是一等公民——因为这是当前唯一被
102 个真实案例验证过的模式，直接复用现成判定函数，保证与 gate 对"测试文件"的定义一致。

**抽取契约**：要求模型严格按 `{"constraints": [...]}` 结构返回，空约束返回
`{"constraints": []}`。解析失败按 SWE-Atlas rubric judge 同款套路重试几次；最终仍失败
→ **fail open**（本会话不启用约束检查，不能因为抽取失败反而挡住正常编辑）。

### 2. 执行阶段：pre-call hook，确定性匹配，零 LLM 调用

**关键决策（已用真实数据验证过）**：pre-tool-hook 本身**不**每次调用 LLM。实测过"每次
Edit/Write/Delete 都过一遍 LLM 语义判断"的真实成本（deepseek-v4-flash 生产端点，3 次
真实请求）：单次判断延迟均值 1.01s，单 task 平均 8.47 次 Edit/Write/Delete 调用（p90=18，
max=25），换算下来单 task 增加约 8.5 秒等待、约 1440 token——这个成本本身不算离谱，但
**目前挖出来的全部 102 个案例，无一例外靠"文件名匹配测试文件命名规则"这个确定性判断就能
100% 命中**，没有任何真实证据证明语义判断能抓住路径匹配漏判的情况。按"不过度宣称收益"的
原则，pre-tool-hook 走确定性匹配，零延迟：

```rust
impl ConstraintMatcher {
    fn matches(&self, file_path: &str) -> bool {
        match self {
            ConstraintMatcher::TestFiles => Lang::from_path(file_path)
                .is_some_and(|l| l.is_test_file(file_path)),
            ConstraintMatcher::PathContains { substrings } => substrings.iter().any(|s| file_path.contains(s)),
            ConstraintMatcher::PathUnderDir { dirs } => dirs.iter().any(|d| file_path.starts_with(d) || file_path.contains(&format!("/{d}/"))),
            ConstraintMatcher::Extension { exts } => exts.iter().any(|e| file_path.ends_with(e)),
            ConstraintMatcher::Unmatched => false,
        }
    }
}
```

**拦截位置**：不在 `file_edit_tool.rs`/`file_write_tool.rs`/`delete_file_tool.rs` 三处各写
一遍。当前 hook 体系只有 post-call（`post_call_hooks.rs`），且 `RuntimeHookErrorPolicy` 已
预留 `DenyTool` 变体但没有 pre-call kind 在用它——这次补齐这个真空，新增对称的 pre-call
hook 注册点，在 `execution_engine.rs` 里工具真正落到 `validate_input()`/`execute()` 之前
的统一分发路径上跑一遍已注册 hook。三个工具实现代码不用感知这个机制的存在，以后再加别的
pre-call 约束（"别碰 lockfile"之类）也走同一注册点，不用重新改工具本身。

### 3. 拒绝反馈设计：拦截 + 纠偏，不是单纯 block

命中约束时，走跟 `validate_input()` 已有的错误反馈通道一致的路径，把这次 Edit 的
`tool_result` 变成结构化的矛盾信号，而不是直接静默拒绝：

```
This file (`report/util_test.go`) matches a constraint stated in the task:
"<抽取到的 description>". This edit was not applied.

Editing a file you were told not to touch usually means your own implementation
doesn't match what's expected — not that the test is wrong. Reconsider your
source-code approach instead of adjusting this file.

If you're certain this file must change for a legitimate reason unrelated to
making your own code compile, explain why before retrying with `force: true`.
```

保留 `force: true` 逃生舱口（约束不总是绝对指令，可能存在合理例外），但要求 agent 显式
声明理由——防止悄悄绕过，理由落轨迹方便后续审计"是真有理由还是纯粹硬闯"。

### 4. 只拦"改已存在文件"，不拦新建

复用 `test_selection_gate.rs` 已有的 baseline/edited_files 追踪逻辑，区分"这一轮之前就
存在的文件"vs"这一轮新建的"——只拦前者。像 SWE-Atlas 的 Test Writing 任务类型本来就要求
新建测试文件，不能连这种正当场景也拦。

### 5. 子代理继承约束，不重复抽取

约束绑定在"这次任务"上，不是绑定在具体 agent 实例上。`GeneralPurpose` 等有 Edit 权限的
子代理被 Task 工具 fork 出来时，从 `fork_agent` 那条 spawn 路径继承父会话已抽取的约束
列表，不重新调用 LLM 抽取一遍（省成本，也避免父子判断不一致）。

## 明确不做（防过度工程）

- **不做 pre-tool-hook 的语义/LLM 判断**——已用真实数据验证当前证据下没必要，见 §2
- **不做自由格式 glob/正则抽取**——受限词表，解析失败直接整体丢弃
- **不重新发明"什么是测试文件"**——复用 `test_selection_gate.rs` 的 `Lang::is_test_file()`
- **不做硬死路**——`force: true` 逃生舱口保留，只是要求显式声明理由
- **不代 agent 猜测正确实现**——机制只负责拦截+提醒重新思考，不负责生成正确答案（那部分
  依然受限于"契约是否可得"这个更底层的问题，本机制解决不了）

## 改动面

| 位置 | 内容 | 规模 |
|---|---|---|
| `execution/agent-runtime/src/pre_call_hooks.rs`（新） | `PreCallHookKind`、`PreToolCallHookExecutor` trait、与现有 `RuntimeHookRegistry`/`DenyTool` 对接 | ~120 行 |
| `agentic/execution/edit_constraint_guard.rs`（新） | 约束抽取（LLM 调用编排）+ `ConstraintMatcher` + session state | ~250 行 |
| `agentic/execution/execution_engine.rs` | 工具分发前跑 pre-call hook | ~40 行 |
| `agentic/fork_agent/mod.rs` | 子代理继承父会话约束列表 | ~20 行 |
| 单测 | 抽取 schema 解析（含 fail-open 路径）+ matcher 规则表驱动 + 新建文件豁免 + 逃生舱口 | ~200 行 |

目标分支为 `yuyiqing/dev`/`upstream/main` 主线（真实产品行为），不是 `evals-on-release`——
这个跟 test-selection-gate 不同：gate 只在评测线验证过就没合入主线，这次设计的场景（用户
自己说"别碰 X"）对真实用户同样成立，应该走正常产品分支流程。

## 验证计划

验证集直接用这次挖出来的 **102 个真实案例**，不新造：

- **目标组 48**：题面有强约束、agent 最终仍改了测试文件、且判分失败的全部 trial
- **回归锚点 54**：题面有强约束、agent 改了测试文件、但仍然判分通过的全部 trial——
  **一个都不能因为这次改动从过变成不过**，误伤了就是设计的硬伤

步骤：
1. 单测 + 本地 smoke（抽取 schema 解析、matcher 各分支、fail-open 路径、force 逃生舱口）
2. 重编二进制（记 commit），拿 102-case 集重跑
3. 核心指标不是"拦截触发了几次"，是**拦截之后 agent 有没有真的改对源码、reward 有没有从
   0 翻到 1**——如果拦住之后 agent 只是卡住/超时/换个方式继续糊弄，说明拒绝文案还得迭代，
   不代表机制本身没用（吸取 gate 那轮"机制生效但看不到分数收益"的教训，不能只报"触发率"
   就宣称成功）
4. 54 个回归锚点必须 54/54 不掉；48 个目标组的翻盘数 + 归因质量（人工抽查几个确认是不是
   真的"重新想清楚了"而不是碰巧蒙对）一起作为最终判断依据
5. 无效或误伤 → revert 单个 commit

## 风险

- **抽取误判（Tier 2 LLM 语义理解偏差）**：把"别改测试格式"误判成"别碰测试文件"这类——
  靠 fail-open（抽取失败/低置信度直接不启用）兜底，且当前只有 `test_files` 这一种
  matcher 有真实证据支撑，其余分类先允许存在但不必急于扩大使用
- **逃生舱口被滥用**：agent 学会无脑加 `force: true` + 敷衍理由绕过——理由落轨迹，验证阶段
  抽查滥用率，必要时加"理由需引用具体技术原因"的格式约束
- **修不了真正的根因**：48 个目标里有多少能翻盘，取决于"重新想清楚"这条路走不走得通——
  部分案例的契约信息本身就不可得（比如要不要砍掉一个死代码 `err` 返回值），这种机制拦住了
  篡改测试的行为，但不代表 agent 就能凭空猜对正确答案，翻盘率可能有上限
- **token/延迟成本**：抽取阶段一次性 LLM 调用（会话首次编辑时），执行阶段零成本——总体
  成本可控，已用真实数据验证（见 §2）
