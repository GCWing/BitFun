# taiji-growth

**路径**: src/crates/taiji/taiji-growth
**描述**: Taiji growth engine — user behavior analytics, A/B testing, and email dispatch

## 依赖
- 内部: taiji-content, taiji-engine
- 外部: serde, serde_json, chrono, tera, tokio, reqwest, lettre, thiserror, async-trait

## 模块结构
- `email_dispatcher` — 邮件分发器（lettre SMTP）
- `publisher_website` — 网站内容发布
- `report_md_gen` — Markdown 报告生成（tera 模板）
- `task_dag_exec` — 任务 DAG 执行引擎
- `task_dag_types` — 任务 DAG 类型定义
- `types` — 共享类型（含 SmtpConfig）

## 核心类型
- `task_dag_types::TaskDag` — 任务 DAG
- `email_dispatcher::EmailDispatcher` — SMTP 邮件分发器
- `report_md_gen::ReportMdGen` — 报告生成器

## 属于领域
- growth / operations
