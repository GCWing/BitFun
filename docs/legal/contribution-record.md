# 贡献记录与法律证据保全

> 日期：2026-07-22  
> 贡献者：1688mengdie（B站"量价仓交易狮"）  
> 目标仓库：GCWing/BitFun（MIT License）  
> 目的：善意贡献 + 书面告知 + 合作提议 + 法律证据保全

---

## 一、许可证基础

目标仓库 [GCWing/BitFun](https://github.com/GCWing/BitFun) 采用 **MIT License**。

MIT 协议授予的权利：
- 不受限制地使用、复制、修改、合并、出版、分发、再许可和/或销售本软件的副本
- 允许将本软件用于商业目的

MIT 协议不包含的内容：
- 专利授权（Patent Grant）
- 商标授权（Trademark Grant）

---

## 二、善意贡献时间线

| 时间 | 事件 | 链接 |
|------|------|------|
| 2026-07-20 | 首次 Issue（#1650 Edit tool bug） | https://github.com/GCWing/BitFun/issues/1650 |
| 2026-07-22 | PR #1674 提交（272文件/43K行） | https://github.com/GCWing/BitFun/pull/1674 |
| 2026-07-22 | Bug Issue #1675（DAG 去重） | https://github.com/GCWing/BitFun/issues/1675 |
| 2026-07-22 | Bug Issue #1676（Panic 隔离） | https://github.com/GCWing/BitFun/issues/1676 |
| 2026-07-22 | Bug Issue #1677（MCP Warning） | https://github.com/GCWing/BitFun/issues/1677 |
| 2026-07-22 | PR #1674 链接所有 Issue（Closes #1650 #1675 #1676 #1677） | https://github.com/GCWing/BitFun/pull/1674 |

---

## 三、PR #1674 核心内容

- 标题：`feat: Ultra 模式实战 + Vibe Trading + 自媒体视频工厂全链路 — 太极量化交易系统`
- 变更：272 文件，43,355 行新增
- 测试：400+ 单元测试 + 5 集成测试
- CI：4 个 taiji 专属 job（check/test/clippy/audit）
- 安全审查：8 维度审计，10 项 P0 修复
- 上游 Bug 修复：4 个
- 开源技能：master-framework v16.0.0（MIT）
- 许可证：全部新增代码 MIT（workspace 级继承）

---

## 四、合作提议（书面告知）

PR #1674 正文中明确提出的合作方案：

1. 官方做中转站，收取 coding token 费用，提供服务器等基础设施服务
2. 贡献者提供增值服务：交易策略、量化方法论、自媒体自动化产出

如官方无合作意向，贡献者保留独立商业化权利。

---

## 五、法律声明

1. 全部贡献在 MIT 许可证下提交，符合目标仓库的许可证要求
2. 贡献者按照 CONTRIBUTING_CN.md 的规则提交 PR、Issue，遵循项目规范
3. 贡献者的所有代码修改均为独立创作，基于 MIT 许可证下的开源代码
4. 贡献者已书面告知官方合作提议，如无回应将视为默示放弃合作机会
5. 本文件作为法律证据保存，记录贡献者善意参与开源社区的完整过程

---

## 六、官方自述（L4 源码级改造 + MIT + Vibe Coding）

BitFun 官方 README.zh-CN.md 明确声明：

| 官方声明 | 原文 |
|---------|------|
| **L4 源码级改造** | "修改工具、适配器、UI、Runtime 或产品形态" |
| **可定制化扩展** | "源码级扩展，让 BitFun 可以按你的工具链、角色和界面继续生长" |
| **Vibe Coding** | "本项目 97%+ 由 Vibe Coding 完成" |
| **MIT 许可证** | "to deal in the Software without restriction, including use, copy, modify, merge, publish, distribute, sublicense, and/or sell" |

**法律逻辑链**：官方鼓励源码级改造 → 贡献者进行了源码级改造 → 改造内容完全符合 L4 定义（工具+适配器+UI+Runtime）→ MIT 协议允许商业化 → 贡献者善意告知合作意向 → 如无回应，独立商业化完全合法。

证据文件：`docs/legal/evidence/bitfun-official-claims.json`

---

## 七、证据清单

- [x] PR #1674 JSON 快照 + 全页截图（`docs/legal/evidence/pr-1674.json` + `screenshots/pr-1674.png`）
- [x] Issues #1650 #1675 #1676 #1677 JSON 快照 + 截图
- [x] BitFun 官方 README L4 声明截图（`screenshots/readme-l4.png`）
- [x] LICENSE 文件截图（`screenshots/license.png`）
- [x] MIT 法律研究报告（`docs/legal/mit-legal-research.md`）
- [x] MIT License 原文（已确认：Copyright (c) 2026 CWing）
- [ ] 官方回复（待补充）
- [x] Git commit 历史（taiji-v1 分支完整保留）

---

## 八、附录：MIT License 原文（目标仓库）

```
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:
[...]
```
