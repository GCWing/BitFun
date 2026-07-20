# Edit Constraint Guard（编辑约束守卫）设计

## 目标

当用户明确要求“不修改某些文件、目录或文件类型”时，产品应在文件真正写入前执行该约束，并向
agent 返回可解释的拒绝原因。约束属于用户会话状态，必须在恢复、fork 和回滚后保持一致。

该能力是通用产品行为，不承担评测环境隔离职责。

## 边界

产品内负责：

- 从每条新的用户指令中抽取“哪些文件不可修改”的约束；
- 将约束持久化为会话状态，并支持显式追加和撤销；
- 在直接文件工具、shell/WriteStdin、Git 工作树操作和递归删除前执行确定性检查；
- 按显式 opt-in 记录决策和成功文件操作，便于诊断误判与遗漏。

产品内不负责：

- 屏蔽 WebFetch、WebSearch、GitHub、Sourcegraph 或任意代码托管域名；
- 禁用 Git 网络、远端引用、commit hash 或历史读取；
- 根据 benchmark、job 名称、环境变量或分支切换工具能力；
- 最终 patch 过滤、基准泄漏判定或评分。

网络出口、仓库历史净化和评测审计属于评测管道。产品工具保持普通用户预期的 Web 与 Git
能力，guard 代码不得出现 benchmark 名称或站点黑名单。

## 模块与职责

| 模块 | 职责 |
|---|---|
| `edit_constraint_guard.rs` | 对外兼容入口；抽取编排、确定性决策、递归删除检查、telemetry |
| `edit_constraint_guard/model.rs` | 可序列化约束、抽取记录、会话状态合并与回滚 |
| `edit_constraint_guard/shell_targets.rs` | 从常见 shell 命令中识别显式文件变更目标 |
| `session_manager.rs` | 状态持久化、恢复、fork 继承与按存活 turn 回滚 |
| 文件与 shell 工具的 `validate_input` | 在执行前调用 guard；不复制约束策略 |

`edit_constraint_guard.rs` 保留既有 public re-export，避免状态类型的存储路径因内部拆分而
改变。模型、解析器和执行编排之间只传递结构化约束或路径列表。

## 状态模型

每条约束包含稳定 id、人类可读描述、来源、操作范围和一个受限 matcher：

- `test_files`：常见测试文件与测试目录约定；
- `path_contains`：路径包含明确字面值；
- `path_under_dir`：路径位于明确目录下；
- `extension`：文件扩展名匹配；
- `unmatched`：只记录，不执行。

操作范围为 `all` 或 `delete_only`。自由正则和自由 glob 不进入持久化契约，避免模型生成
不可预测的匹配规则。

状态同时保存：

- fork 时继承的 active constraints 与 agent-created paths 基线；
- 每次抽取的输入摘要、状态、耗时、模型输出摘要和失败原因；
- 生效、撤销及无法匹配的撤销 id；
- agent 创建文件的路径和创建它的 dialog turn。

父会话状态在 fork 时固化为子会话基线。子会话回滚从该基线开始，只重放仍然存活的子会话
turn 抽取与文件来源记录；旧格式中缺少 turn id 的来源记录不授予豁免。

## 生命周期

1. execution engine 对每个不同的用户 turn 处理一次约束抽取。
2. 明确的测试文件禁改措辞先由确定性句法规则兜底；规则要求禁止词、变更动作与测试文件目标
   形成直接关系，不以关键词共现推断约束。fast 模型补充其他 matcher，并返回显式撤销。
3. 有约束、发生模型尝试或抽取失败时才将结果写入 session metadata；普通无信号 turn 不产生
   guard 状态，也不触发 metadata 重写。抽取失败单独记录，执行阶段 fail open。
4. 文件工具在 `validate_input` 中调用统一检查；shell、WriteStdin 和 Git 工具先将可静态识别的
   变更目标解析为路径。存在 active constraint 时，无法解析目标的高风险变更命令 fail closed。
5. 命中约束时返回 403 和结构化 `edit_constraint_guard` 元数据，不执行写入。
6. 设置 `BITFUN_EDIT_CONSTRAINT_TELEMETRY=1` 后，guard 决策和成功的直接文件工具操作写入
   session-scoped JSONL，用于产品诊断；默认不创建 JSONL。该开关不影响 AI provider 请求审计。

撤销只接受当前 active constraint id，且只有真实用户提交的 turn 可以授权撤销。含糊表达、
未知 id 或模型解析失败都不会放宽既有约束。

## 执行规则

- matcher 执行是确定性的，不在每次工具调用时请求模型。
- 普通无约束会话在工具热路径上不解析 shell、不解析路径、不查询远程文件是否存在，也不写
  session metadata 或 JSONL；只有 TestFiles 全操作约束需要维护 agent-created 文件来源。
- `force` 不是模型可用的逃生口；旧调用携带 `force` 时拒绝并记录。
- “不要修改测试”允许新建 agent 自己的测试辅助文件，也允许后续修改或删除该辅助文件。
- `delete_only` 约束不因 agent-created provenance 放宽。
- 递归删除先检查目标和所有非符号链接后代；存在生效约束时，无法完成检查则 fail closed。
- shell 解析优先检查明确目标；存在 active constraint 时，变量/glob 目标、交互式 shell、未知
  archive/patch 内容及其他无法确定影响范围的高风险变更命令在执行前拒绝。无约束会话不改变
  普通 shell、WriteStdin 或 Git 行为。

## 关键不变量

- 无 active constraint 时，guard 不改变普通编辑、Web 或 Git 行为，也不执行路径/远程存在性检查。
- 同一 dialog turn 和消息 hash 不重复抽取。
- 未知撤销 id 永远不删除约束。
- session restore 与 fork 后的 active constraints 与父会话一致。
- rollback 后不能保留来自已删除 turn 的约束或 agent-created 豁免。
- 拒绝发生在写入前，递归删除不会出现部分删除后才发现受保护文件。
- telemetry 默认关闭；启用后的写入失败不能导致普通工具操作失败。

## 验证策略

最小测试集覆盖：

- matcher 规则和 operation scope；
- 确定性抽取、模型解析失败、合法与非法撤销；
- 新建测试辅助文件、已有测试文件和 delete-only 的差异；
- shell 重定向、`tee`、`cp`、`mv`、`rm`、in-place `sed/perl`、Python、Node、WriteStdin、
  Git pathspec 与无法解析目标的高风险命令；
- 递归删除、符号链接、远程工作区检查失败；
- session 持久化、fork、回滚和旧 schema 兼容；
- 普通 Web/Git 命令不被评测策略拦截。

Rust 变更至少运行该 crate 的 focused tests 和 `cargo check --workspace`。行为边界变化必须先
更新本文，再改实现；大提交的说明应记录动机、边界、失败策略和验证结果。
