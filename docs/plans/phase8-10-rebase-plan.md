# Phase 8-10 执行计划：Rebase 收尾 + 安全审查 + GitHub 上架

> 版本：v1.0 | 日期：2026-07-22 | 分支：src-v2 (base: main bf0b05765)
>
> 配套文档：
> - `.bitfun/team/type-contract-phase8-10.md` — 跨 crate 类型契约 + merge 规则
> - `.bitfun/team/phase8-10-dispatch-prompts.md` — 每个 R-ID 的完整派发指令
>
> 目标：零错误零 warning，安全审查通过，GitHub CI/CD 上线就绪。

---

## 一、架构决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|---------|
| B1 | 已有 src-v2 分支（基于 main bf0b05765）作为当前工作分支 | taiji 变更已全部应用，diff 仅 15 个文件；`cargo check` 仅剩 1 个 warning | 重新从 main 创建（重复劳动） |
| B2 | 24 个 taiji crates 全部留在 workspace（含 4 个闭源注释） | 与 taiji 分支保持一致；闭源 crate 通过注释保持占位 | 删除闭源 crate（丢失上游同步能力） |
| B3 | Phase 9 安全审查全并行（8 维度 × 只读分析） | 安全审查不依赖编译通过；并行可最大化效率 | 串行审查（耗时长） |
| B4 | Phase 10 CI 新增 5 个 job 而非合并到现有 job | taiji crates 构建与 BitFun 核心独立，分离避免构建超时 | 合并到现有 job（CI 时间过长） |
| B5 | 4 个闭源 crate (dvmi/magnet/thrust/risk) 不进入 CI test job | 闭源代码不可在 GitHub Actions 公开运行 | 用 feature flag 隔离（过度工程） |

---

## 二、当前状态诊断

### 2.1 编译状态

```
cargo check --workspace 结果：
  - Error: 0
  - Warning: 1（bitfun-acp: detect_cli 函数未被调用，dead_code）
  - 状态：可编译，仅需修复 1 个 warning
```

### 2.2 差异摘要（src-v2 vs main）

| 类别 | 文件数 | 说明 |
|------|--------|------|
| Cargo.toml | 1 | workspace members + deps + video feature |
| Rust 核心修改 | 8 | ACP 增强 (cli_detect/launch_policy/probe)、LegionControl、ffmpeg_api、MCP 协议适配 |
| 前端 i18n | 3 | legionPattern + acpExternal 翻译 |
| Python 绑定 | 1 | taiji-engine-py |
| taiji crates | 24 | 全部 24 个 crates（含 20 个活跃 + 4 个闭源注释） |
| 文档/脚本 | - | 全部 taiji 独有文件，已在 src-v2 中 |

### 2.3 taiji crate 清单

**活跃（20 个）：**
taiji-bar, taiji-cli, taiji-engine, taiji-engine-py, taiji-content, taiji-publisher,
taiji-growth, taiji-alert, taiji-knowledge-graph, taiji-blog-gen, taiji-example,
taiji-llm, taiji-backtest, taiji-executor, taiji-realtime, taiji-pattern,
taiji-abnormal, taiji-sentiment, taiji-orderflow, taiji-strategen

**闭源注释（4 个）：**
taiji-dvmi, taiji-magnet, taiji-thrust, taiji-risk

### 2.4 已知缺口

| # | 缺口 | 等级 | 根因 | 修复方案 | R-ID | 状态 |
|---|------|------|------|---------|------|------|
| G1 | detect_cli dead_code warning | 低 | 新增 cli_detect.rs 但未在 probe.rs 中调用 | 在 probe.rs 中实现两阶段探测的 Step 1 | R8.8.1 | 派发中 |
| G2 | CI 未覆盖 taiji crates | 中 | taiji 不在上游 CI 范围 | R10.1-R10.4 新增 CI job | R10.1 | 待执行 |
| G3 | taiji 部分 crate 缺少 README | 低 | 开发阶段，文档滞后 | R10.9 | 待执行 |
| G4 | 安全审查未执行 | 高 | Phase 9 尚未启动 | R9.1-R9.12 | 待执行 |
| G5 | GitHub Actions 未验证 taiji CI 配置 | 中 | 新增 CI 后需验证 | R10.5 | 待执行 |

---

## 三、R-ID 矩阵

### Phase 8：Rebase 收尾（2 R-ID）

| R-ID | 任务 | 文件 | 依赖 | 预计影响 |
|------|------|------|------|---------|
| R8.8.1 | 修复 detect_cli dead_code warning | `src/crates/interfaces/acp/src/client/cli_detect.rs`, `probe.rs` | 无 | bitfun-acp |
| R8.8.2 | 终局验证：cargo check + test 全量回归 | 全 workspace | R8.8.1 | 全部 crate |

### Phase 9：安全审查（12 R-ID）

#### Wave 1：审查（8 并行，只读）

| R-ID | 审查维度 | 范围 | 方法 |
|------|---------|------|------|
| R9.1 | Secrets 考古 | `src/crates/taiji/`, `scripts/`, `test_data/` | grep API key/secret/token/密码模式 |
| R9.2 | 依赖供应链 | `Cargo.toml`, `src/crates/taiji/*/Cargo.toml` | `cargo audit` + 手动审查非 crates.io 依赖 |
| R9.3 | 路径遍历 + SSRF | `taiji-content`, `taiji-publisher`, `taiji-realtime`, `taiji-blog-gen`, `taiji-knowledge-graph` | 审计文件路径拼接、URL 构造 |
| R9.4 | 输入验证 | `taiji-engine/src/source/`, `taiji-realtime/`, `taiji-backtest/` | 审计外部数据入口（tick/bar/配置） |
| R9.5 | 权限/认证 | `taiji-cli/`, `taiji-executor/`, `taiji-publisher/` | 审计命令执行、文件访问边界 |
| R9.6 | 日志安全 | 全 `src/crates/taiji/` | 审计日志中是否有敏感数据泄露 |
| R9.7 | unsafe 代码 | 全 `src/crates/taiji/` | `grep -rn "unsafe"` 审计 |
| R9.8 | 加密实践 | 全 `src/crates/taiji/` | 审计哈希/随机数/加密使用 |

#### Wave 2：修复 + 文档（4 串行，依赖审查结果）

| R-ID | 任务 | 依赖 |
|------|------|------|
| R9.9 | 修复 P0 严重安全问题 | R9.1-R9.8 全部完成 |
| R9.10 | 修复 P1 高优先级问题 | R9.9 完成 |
| R9.11 | 更新 SECURITY.md（添加 taiji 特定安全策略） | 无（可与 R9.9 并行） |
| R9.12 | 安全审查报告归档（docs/plans/phase9-security-report.md） | R9.9-R9.11 完成 |

### Phase 10：GitHub 上架（14 R-ID）

#### Wave 1：CI/CD 适配（5 并行）

| R-ID | 任务 | 文件 | 依赖 |
|------|------|------|------|
| R10.1 | CI 新增 `taiji-cargo-check` job | `.github/workflows/ci.yml` | Phase 8 完成 |
| R10.2 | CI 新增 `taiji-cargo-test` job（排除闭源） | `.github/workflows/ci.yml` | R10.1 |
| R10.3 | CI 新增 `taiji-cargo-audit` job | `.github/workflows/ci.yml` | 无 |
| R10.4 | CI 新增 `taiji-clippy` job | `.github/workflows/ci.yml` | R10.1 |
| R10.5 | 验证 CI 配置完整性 | `scripts/check-github-config.mjs` | R10.1-R10.4 |

#### Wave 2：文档与协议（5 并行）

| R-ID | 任务 | 文件 | 依赖 |
|------|------|------|------|
| R10.6 | 更新 README 加入 taiji 模块说明 | `README.md`, `README.zh-CN.md` | 无 |
| R10.7 | 更新 CONTRIBUTING 加入 taiji 贡献指南 | `CONTRIBUTING.md`, `CONTRIBUTING_CN.md` | 无 |
| R10.8 | 确认 LICENSE 覆盖所有 taiji crates | `LICENSE` | 无 |
| R10.9 | 生成 taiji crates 的 README.md | `src/crates/taiji/*/README.md` | 无 |
| R10.10 | 添加 CODEOWNERS（可选） | `.github/CODEOWNERS` | 无 |

#### Wave 3：代码质量（4 并行）

| R-ID | 任务 | 依赖 |
|------|------|------|
| R10.11 | `cargo clippy --workspace` 零 warning（排除闭源） | Phase 8 + 9 全部完成 |
| R10.12 | `cargo fmt --check --all` 格式验证 | Phase 8 + 9 全部完成 |
| R10.13 | 仓库卫生检查 `pnpm run check:repo-hygiene` | Phase 8 + 9 全部完成 |
| R10.14 | 最终全量 `cargo test --workspace` 通过 | R10.11-R10.13 完成 |

---

## 四、依赖图（Kahn 拓扑排序）

```
Level 0 (前置):     R8.8.1 (修复 detect_cli)

Level 1 (Phase 8    R8.8.2 (终局验证)
        收尾):

Level 2 (Phase 9   ┌─ R9.1  (secrets考古)
        Wave 1,   ├─ R9.2  (依赖供应链)
        全并行):   ├─ R9.3  (路径遍历+SSRF)
                   ├─ R9.4  (输入验证)
                   ├─ R9.5  (权限认证)
                   ├─ R9.6  (日志安全)
                   ├─ R9.7  (unsafe代码)
                   ├─ R9.8  (加密实践)
                   └─ R9.11 (SECURITY.md)  ← 可与审查并行

Level 3 (Phase 9   R9.9 (P0修复) ──→ R9.10 (P1修复)
        Wave 2,
        串行):

Level 4 (Phase 9   R9.12 (报告归档)
        收尾):

Level 5 (Phase 10 ┌─ R10.1 (cargo-check CI)
        Wave 1,  ├─ R10.2 (cargo-test CI)
        并行):    ├─ R10.3 (cargo-audit CI)
                   ├─ R10.4 (clippy CI)
                   └─ R10.5 (CI 验证)

Level 6 (Phase 10 ┌─ R10.6 (README)
        Wave 2,  ├─ R10.7 (CONTRIBUTING)
        并行):    ├─ R10.8 (LICENSE)
                   ├─ R10.9 (crate README)
                   └─ R10.10 (CODEOWNERS)

Level 7 (Phase 10 ┌─ R10.11 (clippy零warning)
        Wave 3,  ├─ R10.12 (fmt验证)
        并行):    ├─ R10.13 (repo-hygiene)
                   └─ R10.14 (全量test)

Level 8 (终审):    R-ID 逐项闭合 + 缺口登记表清零 + force push
```

---

## 五、并行策略

### 5.1 同级并行

| Level | 并行度 | 说明 |
|-------|--------|------|
| Level 2 | 9 R-ID 全并行 | R9.1-R9.8 + R9.11 全部只读，无文件冲突 |
| Level 5 | 5 R-ID 全并行 | 各自 CI job，无交叉依赖 |
| Level 6 | 5 R-ID 全并行 | 各自文档文件，无重叠 |
| Level 7 | 4 R-ID 全并行 | 各自独立命令 |

### 5.2 跨级覆盖

- Phase 9 Wave 1（安全审查）可与 Phase 8 并行启动（审查不依赖编译通过）
- Phase 10 Wave 1（CI/CD）可与 Phase 9 Wave 2（安全修复）并行
- Phase 10 Wave 2（文档）可与 Phase 10 Wave 1（CI/CD）并行

### 5.3 串行瓶颈

| 瓶颈 | 原因 | 影响 |
|------|------|------|
| R8.8.1 → R8.8.2 | 终局验证依赖 warning 修复 | Level 0 → Level 1 必须串行 |
| R9.1-R9.8 → R9.9 | P0 修复依赖审查结果 | Level 2 → Level 3 必须串行 |
| R9.9 → R9.10 | P1 修复依赖 P0 先完成（避免冲突） | Level 3 内部串行 |
| R10.11-R10.14 | 依赖 Phase 8+9 全部完成 | Level 7 必须在 Phase 8+9 之后 |

---

## 六、测试策略

### 6.1 每 R-ID 验证

| R-ID 类型 | 最低验证 |
|-----------|---------|
| 只读审查 (R9.1-R9.8) | 输出清单完整性（逐条标注文件:行号） |
| 代码修复 (R8.8.1, R9.9, R9.10) | `cargo check -p <crate>` + `cargo test -p <crate>` |
| CI/CD (R10.1-R10.5) | GitHub Actions workflow 语法验证 + check-github-config |
| 文档 (R10.6-R10.10) | Markdown 格式 + 中英文一致 |
| 终局验证 (R8.8.2, R10.11-R10.14) | 全量 `cargo check` + `cargo test` + `cargo fmt` + `cargo clippy` |

### 6.2 回归测试

- 每个代码修改后运行 `cargo test -p <affected_crate>`
- Phase 8 + 9 完成后运行 `cargo test --workspace`
- Phase 10 完成后运行 `cargo test --workspace` + CI 全绿

---

## 七、风险列表

| # | 风险 | 概率 | 影响 | 缓解措施 |
|---|------|------|------|---------|
| R1 | 安全审查发现真实泄露（P0） | 低 | 高 — 需轮换密钥 + 清理历史 | R9.1 只读先行，发现后立即止损 |
| R2 | cargo-audit 发现 CVE 需升级依赖 | 中 | 中 — 可能引发编译兼容问题 | R9.2 先跑 audit，版本锁定后再升级 |
| R3 | CI job 构建超时（taiji crates 过多） | 中 | 低 — 可拆分 job | R10.1 用 rust-cache + 增量编译 |
| R4 | 闭源 crate 被误加入 CI test | 低 | 中 — CI 失败 | Cargo.toml 注释明确，CI 用 --exclude |
| R5 | detect_cli 修复引入新 bug | 低 | 低 — 仅 ACP 探测逻辑 | R8.8.1 改动小，带单元测试 |
| R6 | main 在期间有新 commit 导致冲突 | 低 | 高 — 需重新 merge | 定期 `git fetch upstream main` 检查差异 |

---

## 八、缺口登记表

| # | 缺口 | 等级 | 根因 | 修复方案 | R-ID | 状态 |
|---|------|------|------|---------|------|------|
| G1 | detect_cli dead_code warning | 低 | cli_detect.rs 未在 probe.rs 中调用 | 在 probe.rs 中实现 Step 1 CLI 检测，调用 detect_cli | R8.8.1 | 派发中 |
| G2 | CI 未覆盖 taiji crates | 中 | taiji 不在上游 CI 范围 | 新增 5 个 CI job（R10.1-R10.5） | R10.1 | 待执行 |
| G3 | taiji 部分 crate 缺少 README | 低 | 开发阶段，文档滞后 | 为每个 taiji crate 生成 README.md | R10.9 | 待执行 |
| G4 | 安全审查未执行 | 高 | Phase 9 尚未启动 | R9.1-R9.12 完整执行 | R9.1 | 待执行 |
| G5 | GitHub Actions 未验证 CI 配置 | 中 | 新增 CI 后需通过 check-github-config | R10.5 验证 | R10.5 | 待执行 |

---

## 九、执行铁则

1. **改前必须 Read**：任何文件修改前，先 `Read` 目标文件（铁则 11）
2. **先读 type-contract**：所有 agent 执行前必须读取 `.bitfun/team/type-contract-phase8-10.md`
3. **taiji 优先**：taiji 独有功能优先保留；仅当与 main 冲突时做最小适配
4. **缺口闭环**：每个问题：根因 → 修复 → 验证 → 更新登记表
5. **只读审查**：R9.1-R9.8 只读分析，不修改代码。输出风险清单
6. **连续失败 2 次 → 指挥官介入**：不盲目重试同一方案
7. **加法不做减法**：保持 BitFun 原有功能，taiji 功能叠加
8. **按步验证**：每完成一个 R-ID，立即运行验证命令。不累积到最后
