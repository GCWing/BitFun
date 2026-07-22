use serde::{Deserialize, Serialize};

/// 知识图谱节点分类：理论概念 / 策略规则 / 案例
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCategory {
    Concept,
    Strategy,
    Case,
}

/// 关系类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// 派生子概念（理论→子概念、Agent→输出字段）
    DerivesFrom,
    /// 使用/依赖（决策Agent使用结构Agent输出）
    Uses,
    /// 相关性关联（跨概念关联）
    CorrelatesWith,
    /// 包含关系（父概念包含子概念）
    Contains,
}

/// 知识图谱节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptNode {
    pub id: String,
    pub name: String,
    pub category: NodeCategory,
    pub description: String,
    /// 来源引用（四总纲章节、Agent名称等）
    pub sources: Vec<String>,
}

/// 知识图谱边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationEdge {
    pub from: String,
    pub to: String,
    pub relation: RelationType,
    /// 关系权重 0.0-1.0
    pub weight: f64,
    pub label: String,
}

/// 子图查询结果：节点 + 边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphResponse {
    pub nodes: Vec<ConceptNode>,
    pub edges: Vec<RelationEdge>,
}

/// 布局坐标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPosition {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub layer: u32,
}

/// 布局结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutResponse {
    pub positions: Vec<LayoutPosition>,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub node: ConceptNode,
    /// 匹配分数 0.0-1.0
    pub score: f64,
    /// 关联节点 ID 列表
    pub related_ids: Vec<String>,
}
