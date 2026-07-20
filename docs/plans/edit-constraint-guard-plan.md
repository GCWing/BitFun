# Edit Constraint Guard（编辑约束守卫）设计

## 目标

当用户明确要求“不修改某些文件、目录或文件类型”时，产品应在文件真正写入前执行该约束，并向
agent 返回可解释的拒绝原因。约束属于用户会话状态，必须在恢复、fork 和回滚后保持一致。

该能力是通用产品行为，不承担评测环境隔离职责。

## 边界

产品内负责：

- 从每条新的用户指令中抽取“哪些文件不可修改”的约束；
- 将约束持久化为会话状态，并支持显式追加和撤销；
- 在直接文件工具、显式 shell 文件写入和递归删除前执行确定性检查；
- 记录抽取、决策和成功文件操作，便于诊断误判与遗漏。

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

- 每次抽取的输入摘要、状态、耗时、模型输出摘要和失败原因；
- 生效、撤销及无法匹配的撤销 id；
- agent 创建文件的路径和创建它的 dialog turn。

回滚只重放仍然存活 turn 的抽取与文件来源记录；旧格式中缺少 turn id 的来源记录不授予豁免。

## 生命周期

1. execution engine 对每个不同的用户 turn 处理一次约束抽取。
2. 明确的测试文件禁改措辞先由确定性规则兜底；fast 模型补充其他 matcher，并返回显式撤销。
3. 抽取结果写入 session metadata。抽取失败单独记录，执行阶段 fail open。
4. 文件工具在 `validate_input` 中调用统一检查；shell 工具先将可静态识别的写入目标解析为路径。
5. 命中约束时返回 403 和结构化 `edit_constraint_guard` 元数据，不执行写入。
6. 成功的直接文件工具操作写入 session-scoped JSONL，用于产品诊断。

撤销只接受当前 active constraint id，且只有真实用户提交的 turn 可以授权撤销。含糊表达、
未知 id 或模型解析失败都不会放宽既有约束。

## 执行规则

- matcher 执行是确定性的，不在每次工具调用时请求模型。
- `force` 不是模型可用的逃生口；旧调用携带 `force` 时拒绝并记录。
- “不要修改测试”允许新建 agent 自己的测试辅助文件，也允许后续修改或删除该辅助文件。
- `delete_only` 约束不因 agent-created provenance 放宽。
- 递归删除先检查目标和所有非符号链接后代；存在生效约束时，无法完成检查则 fail closed。
- shell 解析只拦截能够明确解析出的目标。动态计算的路径不假装已被覆盖，依靠成功操作与最终
  patch 的外部关联暴露缺口。

## 关键不变量

- 无 active constraint 时，guard 不改变普通编辑、Web 或 Git 行为。
- 同一 dialog turn 和消息 hash 不重复抽取。
- 未知撤销 id 永远不删除约束。
- session restore 与 fork 后的 active constraints 与父会话一致。
- rollback 后不能保留来自已删除 turn 的约束或 agent-created 豁免。
- 拒绝发生在写入前，递归删除不会出现部分删除后才发现受保护文件。
- telemetry 失败不能导致普通工具操作失败。

## 验证策略

最小测试集覆盖：

- matcher 规则和 operation scope；
- 确定性抽取、模型解析失败、合法与非法撤销；
- 新建测试辅助文件、已有测试文件和 delete-only 的差异；
- shell 重定向、`tee`、`cp`、`mv`、`rm`、in-place `sed/perl`、Python、Node 与 Git
  pathspec；
- 递归删除、符号链接、远程工作区检查失败；
- session 持久化、fork、回滚和旧 schema 兼容；
- 普通 Web/Git 命令不被评测策略拦截。

Rust 变更至少运行该 crate 的 focused tests 和 `cargo check --workspace`。行为边界变化必须先
更新本文，再改实现；大提交的说明应记录动机、边界、失败策略和验证结果。
