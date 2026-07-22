# MIT 许可证法律研究报告

> 研究日期：2026-07-22  
> 目标：评估 MIT 协议下 fork + 独立商业化 + 专利风险

---

## 核心结论

### 1. MIT 是否允许独立商业化？→ 是

MIT 明确允许任何人 fork、修改、商用，无需原作者同意。唯一要求：保留版权声明和许可声明。

> "Permission ... to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell"

来源：opensource.org、choosealicense.com

### 2. MIT 是否包含专利授权？→ 美国法下：是（明示许可）；中国法下：有风险

美国主流观点（Red Hat 法务、开源律师 Kyle E. Mitchell）认为 "to deal in the Software without restriction" 已构成覆盖专利权的**明示许可**。"without restriction" 和 "use"（35 U.S.C. § 271(a) 专利法术语）是关键证据。

但中国法律体系下，专利授权需要更明确的表述。Apache 2.0 有明确的 "Grant of Patent License" 条款，MIT 没有。

**实务建议**：MIT 的专利保护弱于 Apache 2.0，但强于无许可。如果担心专利风险，可考虑将核心创新部分单独以 Apache 2.0 发布。

### 3. 分叉后需要保留什么？→ 版权声明

必须保留原项目的版权声明和 MIT 许可声明在源码中。不需要在营销材料或 UI 中提及原作者。

违反后果：Rosen v. Blue Mountain (2018) — $5,000 法定赔偿 + 律师费。

### 4. 贡献者的权利 → 保留版权，按 inbound=outbound 授权

没有 CLA 的情况下，贡献者保留代码版权，但通过 GitHub 服务条款（inbound=outbound）自动按 MIT 授权给下游。贡献者可以独立使用自己的代码。

### 5. 中国法院先例

最高法（2021）知民终 51 号：开源合规与著作权侵权是两个独立法律问题。即使原告违反了 GPL，仍有权对未经授权的复制者主张著作权。

---

## 三种许可证对比

| | MIT | Apache 2.0 | GPLv3 |
|---|-----|-----------|-------|
| 商业化 | ✅ | ✅ | ✅（但需开源） |
| 闭源 | ✅ | ✅ | ❌ |
| 明示专利授权 | ❌（隐含） | ✅ | ✅ |
| 专利报复条款 | ❌ | ✅ | ✅ |
| 商标授权 | ❌ | ❌ | ❌ |
| 传染性 | 无 | 无 | 强 |

---

## 对 Taiji 项目的建议

1. **短期**：MIT 协议下独立商业化完全合法。保留 GCWing/BitFun 的版权声明在源码中即可
2. **中期**：核心策略代码（taiji-dvmi 等闭源 crate）可以用独立许可证发布，不受 MIT 约束
3. **防专利风险**：如果 BitFun 母公司（CWing）未来申请相关专利并试图限制你的商业化，Apache 2.0 的专利报复条款缺失是 MIT 的主要弱点。建议：① 保留所有书面告知记录（已完成）② 确保你的衍生代码有独立的原创性 ③ 关注 Open Invention Network
4. **商标注意**：MIT 不授权商标。不要使用 "BitFun" 品牌名称销售你的产品

---

## 参考来源

- opensource.org — MIT License 原文
- choosealicense.com — MIT 许可条款解读
- opensource.com — "Does the MIT License include a patent grant?" (Scott K Peterson, 2018)
- linuxstory.org — MIT 许可证逐行解读
- codequiry.com — "What Open Source Licenses Actually Enforce in Court" (2024)
- Rosen v. Blue Mountain Data Systems (2018) — MIT 版权声明强制执行判例
- 中国最高法（2021）知民终 51 号 — 开源合规与著作权独立原则
- GitHub Terms of Service — inbound=outbound 规则
