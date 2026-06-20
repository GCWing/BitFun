---
name: synod-presets
description: |
  Predefined expert team configurations for the Synod tool.
  Maps common use cases to optimal councillor compositions.
  Use when: you need a pre-configured set of experts for
  a common evaluation scenario.
---

## 预设配置

### review（代码审查）
- 架构: model=primary, role="关注模块边界、API设计、依赖方向和扩展性"
- 安全: model=primary, role="关注OWASP Top 10、攻击面、权限校验和数据泄露"
- 性能: model=fast,   role="关注延迟、并发瓶颈、N+1查询和资源消耗"

### design（方案设计）
- 技术选型: model=primary, role="关注方案成熟度、生态系统、学习曲线和长期维护成本"
- 成本评估: model=fast,   role="关注开发成本、维护成本、迁移成本和团队技能匹配"
- 可行性:   model=primary, role="关注实施风险、外部依赖、时间线合理性"

### deep（深度审查，双强模型）
- 主审: model=primary, role="全面代码审查，关注正确性、边缘条件和稳定性"
- 对抗: model=primary, role="对抗性审查，专门找其他人可能遗漏的极端情况和隐藏假设"

### startup（创业评估）
- 市场:   model=fast,   role="关注市场需求强度、竞品格局、差异化定位"
- 技术:   model=primary, role="关注技术可行性、实现复杂度、技术债务积累风险"
- 产品:   model=fast,   role="关注用户体验、MVP范围、用户获取路径"
