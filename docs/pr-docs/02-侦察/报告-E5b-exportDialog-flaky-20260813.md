# 报告-E5b-exportDialog-flaky-20260813

> 任务：E-5b 根治 · CLI export_dialog flaky（urgent，开发版指挥官补派）
> 工位：executor（E5b）
> 日期：2026-08-13
> 定标：flaky 复现 1 次即修（CI run 31660460781 已复现 = 派工位修）
> 方案：方案 A（测试侧对齐 + 放宽 prompt 断言超时）；方案 B（生产侧加固）本轮不实施，登记后续优化项

## 一、三证据

### 证据 1：CI 失败日志全量抓取（run 31660460781，job 94324069966）

`gh api repos/1688mengdie/BitFun/actions/jobs/94324069966/logs` 实测：

```
---- export_dialog_writes_markdown_under_the_local_cli_directory stdout ----
thread 'export_dialog_writes_markdown_under_the_local_cli_directory' (6639)
panicked at src/apps/cli/tests/terminal_process_contracts.rs:702:9:
startup prompt was not rendered; output:
ESC[?1049h ESC[?1000h...ESC[?2004h ESC[15;34H [1m[38;5;6;49mInitializing system, please wait... [0m [?25l
test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.12s
```

- 同批 8 测试 7 过 1 挂；失败测试是最后一个（其余 7 个全 ok）。
- 失败测试**首个断言即失败**：`?2004h`（30s）已通过、`write(b"export transcript contract")` 已发出，但「startup prompt 渲染」断言（原 15s 超时）未等到。
- 输出停在 `Initializing system, please wait...`（render_loading 帧）——进程卡在核心服务初始化窗口，整个 30s 测试窗口内 PTY 无后续输出。
- 同 commit 跨 run 时序：31660460781 挂、31661954732 过 → 慢启动偶发，非功能缺陷。

### 证据 2：启动链路读码（根因定位）

`run_interactive`（main.rs:971）中 `render_loading("Initializing system...")` 之后、startup 页首帧之前是**同步等待链**：

```
render_loading → setup_workspace() → initialize_core_services().await
  → initialize_global_config / AIClientFactory::initialize_global / init_agentic_system
  → EmbeddedAppServerHost::start().await → try_restore_session().await
  → startup_page.run(&mut terminal)  ← 第一个 terminal.draw 首帧
```

- 首帧前无任何 `terminal.draw` 刷新；若核心服务初始化任一 `await` 在 CI 偶发变慢，PTY 输出就停在 loading 一帧。
- 同批其余 7 测试首断言均为 `expect_output("\x1b[?2004h", 30s)`——该 ESC 在 `init_terminal()`（main.rs:123）立即发出，**不依赖核心服务初始化**；而 export_dialog 测试是 8 个中唯一「首断言依赖 startup 首帧渲染」的，因此是唯一把慢启动暴露成首帧竞态的测试。
- 测试 `write()` 的输入由 PTY 内核缓冲，慢启动期间不丢失；启动完成后 prompt 必被渲染。

### 证据 3：修复前本地验证（改动前）

- 本地 Windows（非 CI）不重现 CI 慢启动，目标测试单轮 0.76~0.81s 稳定通过 → 纯 CI 偶发。
- 分 crate `cargo check -p bitfun-cli --tests --jobs 4` 0 error（唯一 warning 为 [daemon/provision.rs:182](src/apps/cli/src/daemon/provision.rs#L182) pre-existing unused import，未触碰）。

## 二、修复内容（方案 A，1 处测试改动）

`src/apps/cli/tests/terminal_process_contracts.rs` `export_dialog_writes_markdown_under_the_local_cli_directory`：

- prompt 渲染断言超时 **15s → 30s**（与其余 7 测试的 30s 首帧等待对齐）。
- 保留先 `expect_output("\x1b[?2004h", 30s)` → `write("export transcript contract")` → 等 prompt 的既有顺序（`?2004h` 已在修复前存在，无需新增）。
- 新增注释说明根因（慢启动窗口内 PTY 缓冲输入，30s 内必渲染，非错过）。

### 影响范围

- 仅测试代码 1 处超时 + 注释；无生产代码改动、无 Cargo.toml 变更。
- 不改动同批其余 7 测试；不新增/删除断言。

## 三、验收证据

| 验收项 | 结果 |
| --- | --- |
| 目标测试 10 轮连跑（--test-threads=1 串行） | 10/10 通过（0.73s~0.81s/轮） |
| 同批其余 7 测试回归 | terminal_process_contracts 全量：7 passed; 0 failed（1 个 `startup_bracketed_paste...` 为 `#[cfg(unix)]`，Windows 本地跳过，CI ubuntu 会跑） |
| 分 crate check 0 error | `cargo check -p bitfun-cli --tests --jobs 4` EXIT=0，0 error |

- 验证命令（本地 Windows）：
  - 10 轮：`cargo test --jobs 4 -p bitfun-cli --test terminal_process_contracts -- export_dialog_writes_markdown_under_the_local_cli_directory --exact --test-threads=1` ×10
  - 全量回归：`cargo test --jobs 4 -p bitfun-cli --test terminal_process_contracts`
  - check：`cargo check -p bitfun-cli --tests --jobs 4`

## 四、涉及文件

- `src/apps/cli/tests/terminal_process_contracts.rs`（prompt 断言 15s→30s + 根因注释）

## 五、后续优化项（本轮不实施，方案 B）

- 生产侧加固（方案 B，登记）：`run_interactive` 在 `initialize_core_services().await` 期间周期刷新 loading 帧，或后台化初始化 + 首帧先行渲染，使 PTY 在慢启动时仍有输出。风险大、改动面广（涉及 AIClientFactory/EmbeddedAppServerHost/account 启动链），本轮不做。
- 若 CI 仍偶发（理论上 30s 已对齐 7 测试基线），下一步可捕获 stderr/tracing 定位 `initialize_core_services` 内具体慢 await。
