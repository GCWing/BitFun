# Phase 8-10 总计划：taiji/src → main 适配 + 安全审查 + GitHub 上架

> 目标：将 taiji 分支（26 crates, 561 文件）和 src 分支（BitFun 集成）适配到上游最新 main (bf0b05765)，
> 同时完成安全审查和 GitHub 上架就绪。三省三域标准：99%准备 + 1%执行。
>
> 分支策略：创建 `taiji-v2` 从 main → 应用 taiji 全量变更 → 创建 `src-v2` 从 taiji-v2 → 应用 src 特定变更。

---

## 架构决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|---------|
| B1 | 新建 `taiji-v2` 从 main 而非 rebase | 避免 72 commit 逐提交冲突解决；干净历史 | rebase（冲突太多） |
| B2 | taiji 26 crates 全部迁移（含闭源注释） | 保持与 taiji 分支一致 | 仅迁移开源部分（需额外维护） |
| B3 | 闭源 crates (dvmi/magnet/thrust/risk) 保持注释 | 已有策略，不改变 | 用 feature flag（过度工程） |
| B4 | 安全审查在 Wave 1 文件就位后立即并行启动 | 安全审查不依赖编译通过 | 等编译通过后（延迟） |
| B5 | GitHub CI 需新增 taiji-crates-check job | taiji crates 不在当前 CI 覆盖范围 | 合并到现有 job（构建时间过长） |

---

## Phase 8：Rebase 适配（基于现有 rebase-to-main-task-list.md）

### 前置：环境准备

| R-ID | 任务 | 文件 | 预计行数 |
|------|------|------|---------|
| R8.0.1 | 创建 `taiji-v2` 分支从 main (bf0b05765) | git操作 | - |
| R8.0.2 | 补齐 workspace 缺失依赖 | `Cargo.toml` | 5 |
| R8.0.3 | 注册 taiji crates 到 workspace members | `Cargo.toml` | 20 |

### Wave 1：taiji 独有新增文件（无冲突，全并行）

| R-ID | 模块 | 源分支 | 文件数 |
|------|------|--------|--------|
| R8.1.1 | taiji 全部 26 crates | taiji | 200+ |
| R8.1.2 | taiji MiniApp | taiji | 10 |
| R8.1.3 | taiji desktop API | taiji | 8 |
| R8.1.4 | taiji desktop tests | taiji | 2 |
| R8.1.5 | taiji UI 组件 | taiji | 6 |
| R8.1.6 | taiji UI Legion 组件 | src | 4 |
| R8.1.7 | taiji UI BeeColony | src | 2 |
| R8.1.8 | ACP 增强模块 (cli_detect, launch_policy, probe) | src | 3 |
| R8.1.9 | 文档 | taiji+src | 50+ |
| R8.1.10 | 脚本 | taiji | 20+ |
| R8.1.11 | 网站 | taiji | 30+ |
| R8.1.12 | 测试数据 | taiji | 50+ |
| R8.1.13 | docker/配置 | taiji | 2 |

### Wave 2：BitFun 核心文件适配（手工合并，27 R-ID，7 组并行）

详见 `rebase-to-main-task-list.md` Wave 2 (R2.1-R2.27)。

### Wave 3：验证

| R-ID | 任务 |
|------|------|
| R8.3.1 | `cargo check --workspace` 全量编译 |
| R8.3.2 | `cargo test -p taiji-engine` 核心测试 |
| R8.3.3 | `cargo test --workspace` 全量测试（排除已知失败） |
| R8.3.4 | 交叉审查：逐 R-ID 闭合确认 |

---

## Phase 9：安全审查（CSO 级）

> 在 Wave 1 文件就位后即可并行启动，不依赖编译通过。
> 按 CSO skill 方法论：基础设施优先 → secrets考古 → 依赖供应链 → CI/CD管道 → LLM/AI安全 → OWASP Top 10 → STRIDE威胁建模 → 主动验证。

### Wave 4：安全审查（全并行）

| R-ID | 审查维度 | 范围 | 方法 |
|------|---------|------|------|
| R9.1 | Secrets 考古 | 全仓库 `src/crates/taiji/`, `scripts/`, `test_data/` | Grep API key/secret/token/密码 模式 |
| R9.2 | 依赖供应链 | `Cargo.toml` 全部依赖 | `cargo audit` + 手动审查非 crates.io 依赖 |
| R9.3 | 路径遍历 + SSRF | `taiji-content`, `taiji-publisher`, `taiji-realtime`, WebFetch | 审计文件路径拼接、URL 构造 |
| R9.4 | 输入验证 | `taiji-engine/src/source/`, `taiji-realtime/` | 审计外部数据入口（tick/bar/配置） |
| R9.5 | 权限/认证 | `taiji-cli/`, `taiji-executor/` | 审计命令执行、文件访问边界 |
| R9.6 | 日志安全 | 全 taiji crates | 审计日志中是否有敏感数据泄露 |
| R9.7 | 不安全代码块 | 全 taiji crates | `grep -r "unsafe" src/crates/taiji/` |
| R9.8 | 加密实践 | `taiji-engine/src/compliance.rs` | 审计密码学使用是否正确 |

### Wave 5：安全修复（串行依赖审查结果）

| R-ID | 任务 | 依赖 |
|------|------|------|
| R9.9 | 修复 P0 严重安全问题 | R9.1-R9.8 结果 |
| R9.10 | 修复 P1 高优先级问题 | R9.1-R9.8 结果 |
| R9.11 | 更新 SECURITY.md（添加 taiji 特定安全策略） | - |
| R9.12 | 安全审查报告归档 | R9.9-R9.10 完成 |

---

## Phase 10：GitHub 上架就绪

### Wave 6：CI/CD 适配（并行）

| R-ID | 任务 | 文件 |
|------|------|------|
| R10.1 | CI 新增 `taiji-cargo-check` job | `.github/workflows/ci.yml` |
| R10.2 | CI 新增 `taiji-cargo-test` job (排除闭源) | `.github/workflows/ci.yml` |
| R10.3 | CI 新增 `taiji-cargo-audit` job | `.github/workflows/ci.yml` |
| R10.4 | CI 新增 `taiji-clippy` job | `.github/workflows/ci.yml` |
| R10.5 | 验证 CI 配置完整性 (check-github-config) | `scripts/check-github-config.mjs` |

### Wave 7：文档与协议（并行）

| R-ID | 任务 | 文件 |
|------|------|------|
| R10.6 | 更新 README.md 加入 taiji 模块说明 | `README.md`, `README.zh-CN.md` |
| R10.7 | 更新 CONTRIBUTING.md 加入 taiji 贡献指南 | `CONTRIBUTING.md`, `CONTRIBUTING_CN.md` |
| R10.8 | 确认 LICENSE 覆盖所有 taiji crates | `LICENSE` |
| R10.9 | 生成/更新 taiji crates 的 README.md | 各 `src/crates/taiji/*/README.md` |
| R10.10 | 添加 CODEOWNERS 或 taiji 模块维护者 | `.github/CODEOWNERS` (可选) |

### Wave 8：代码质量（并行）

| R-ID | 任务 |
|------|------|
| R10.11 | `cargo clippy --workspace` 零 warning（排除闭源） |
| R10.12 | `cargo fmt --check --all` 格式验证 |
| R10.13 | 仓库卫生检查 `pnpm run check:repo-hygiene` |
| R10.14 | 最终全量 `cargo test --workspace` 通过 |

---

## 拓扑排序

```
Level 0 (前置):  R8.0.1 (创建分支)
Level 1:        R8.0.2, R8.0.3 (workspace 配置)
Level 2 (Wave 1): R8.1.1-R8.1.13 (taiji 独有文件，全并行)
Level 3 (Wave 4, 与 Wave 2 并行): R9.1-R9.8 (安全审查，全并行)
Level 4 (Wave 2): R2.1-R2.27 (BitFun 核心适配，7组并行)
Level 5 (Wave 3): R8.3.1-R8.3.4 (编译+测试验证)
Level 6 (Wave 5): R9.9-R9.12 (安全修复)
Level 7 (Wave 6): R10.1-R10.5 (CI/CD)
Level 8 (Wave 7): R10.6-R10.10 (文档)
Level 9 (Wave 8): R10.11-R10.14 (代码质量)
Level 10 (终审): R-ID 逐项闭合 + force push
```

## 并行策略

- Wave 1 全部 13 R-ID 可并行
- Wave 4 全部 8 R-ID 可并行，且与 Wave 2 并行为 Level 3
- Wave 2 分 7 组，组间无文件重叠，可全并行
- Wave 6/7/8 按 CI→文档→质量串行（但各自内部并行）

## 缺口登记表

| # | 缺口 | 等级 | 根因 | 修复方案 | R-ID | 状态 |
|---|------|------|------|---------|------|------|
| G1 | src 分支 Cargo.toml 仅有 5 个 taiji stub crates | 严重 | 历史：src 只集成 BitFun 层面，未包含完整引擎 | Wave 1 R8.1.1 补充完整 26 crates | R8.0.3 | 待修复 |
| G2 | taiji 分支 Cargo.toml 依赖版本可能与 main 不一致 | 严重 | taiji 基于旧 main (47a43e354) | 审计所有依赖版本差异 | R8.0.2 | 待修复 |
| G3 | scheduler.rs 和 coordinator.rs API 重构 | 中等 | 上游 main 已重构 background task API | R2.1-R2.2 适配新 API | R2.1 | 待修复 |
| G4 | CI 未覆盖 taiji crates | 中等 | taiji 不在上游 CI 范围 | R10.1-R10.4 | 待修复 |
| G5 | 无 taiji crate 文档 | 低 | 开发阶段，文档滞后 | R10.6-R10.9 | 待修复 |

## 关键原则

1. 每 R-ID 改前先 Read 目标文件（铁则 11）
2. taiji 独有新增功能优先保留；API 不兼容旧代码删除并标注原因
3. 每完成一个 R-ID，立即 `cargo check -p <affected_crate>` 验证（编译级）
4. 缺口登记表：每个问题标根因 → 修复 → 验证闭环
5. 安全 P0 问题阻塞 merge；P1 可记录为 follow-up issue
6. 连续失败 2 次 → 指挥官直接介入（经验 15）
