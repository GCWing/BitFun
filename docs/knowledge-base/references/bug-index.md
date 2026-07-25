# Bug & Issue Index

> 跟踪 taiji-quant 分支已知问题和待修复项
> 更新日期: 2026-07-25

---

## 当前已知问题

### P0 — 阻塞级

| # | 问题 | 文件 | 根因 | 状态 |
|---|------|------|------|------|
| B001 | sherpa-onnx-sys 构建失败 | upstream dep | 需要下载 ~43MB 原生库，本地 TLS 环境无法下载 | 🔴 需手动干预 |
| B002 | taiji-engine-py 依赖 PyO3 | taiji-engine-py/Cargo.toml | 需要 Python + pyo3 编译环境，CI 中需 `--exclude` | 🟡 需配置 |

### P1 — 严重

| # | 问题 | 文件 | 根因 | 状态 |
|---|------|------|------|------|
| B003 | candle-core `workspace = true` 未使用 | taiji-llm/Cargo.toml | 使用 `optional = true` + feature gate，workspace 声明了但 crate 用 inline | 🟢 无影响 |
| B004 | 品牌图标文件重复 | taiji-icon.png / Logo-ICON.png | 迁移时 cp 了 Logo-ICON 作为 taiji-icon，文件重复 | 🟢 无害 |

### P2 — 低优

| # | 问题 | 文件 | 根因 | 状态 |
|---|------|------|------|------|
| B005 | 上游 deprecated API（rmcp） | services-integrations | `.enable_roots()` 和 `.enable_sampling()` 已 deprecated，已移除 | 🟢 已修 |
| B006 | 上游 deprecated API（sse_stream） | services-integrations | `from_byte_stream` → `from_bytes_stream` 已修 | 🟢 已修 |

## 修复历史

| # | 问题 | 修复 | commit |
|---|------|------|--------|
| B005 | rmcp deprecated | rm enable_roots/enable_sampling | 1b8721eb1 |
| B006 | sse_stream deprecated | from_byte_stream → from_bytes_stream | 1b8721eb1 |
| — | ReviewPropagationNeeded 枚举缺失 | 添加枚举变体 | 1b8721eb1 |
| — | SessionTreeManager import 缺失 | 加 use 语句 | 1b8721eb1 |
| — | run_in_background dead field | 删除未用字段 | 1b8721eb1 |
| — | workspace 重复 key ×3 | 删重复行 | — |
| — | workspace 缺失依赖 ×6 | 添加 statrs/lettre/tera/crossbeam/dashmap/futures | — |
