# taiji-blog-gen

**路径**: src/crates/taiji/taiji-blog-gen
**描述**: Taiji blog generator — Agent JSON → Hugo Markdown blog posts

## 依赖
- 内部: taiji-growth
- 外部: serde, serde_json, tera, chrono, clap, anyhow

## 模块结构
- `main.rs` — CLI binary（支持单文件/批量模式）
- `templates/` — Tera 模板（daily_post / weekly_summary / special_topic）

## 核心类型
- `Cli` — clap CLI 参数结构
- `AgentInput` — 7 个 Agent 分析结果输入（structure/delta/magnet/thrust/resonance/decision/risk）
- `AgentOutput` — 单个 Agent 输出（analysis/confidence/decision/constraints）

## 核心函数
- `process_single()` — 处理单个 JSON → Hugo Markdown
- `process_batch()` — 批量处理目录下所有 JSON
- `map_tags()` — 根据 Agent 分析自动映射 Hugo 标签

## 属于领域
- content / publishing
