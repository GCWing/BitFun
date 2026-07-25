# taiji-knowledge-graph

**路径**: src/crates/taiji/taiji-knowledge-graph
**描述**: Taiji knowledge graph — petgraph-backed concept/strategy/case relation graph

## 依赖
- 内部: 无
- 外部: serde, serde_json, chrono, petgraph, log, anyhow

## 模块结构
- `types` — ConceptNode / RelationEdge / NodeCategory / SubgraphResponse 等类型
- `embedding` — 图嵌入
- `lib.rs` — KnowledgeGraph 主结构（构建/查询/搜索/布局/路径）

## 核心类型
- `KnowledgeGraph` — petgraph 有向图，三层结构（Concept/Strategy/Case）
- `ConceptNode` — 概念/策略/案例节点
- `RelationEdge` — 关系边

## 核心函数
- `KnowledgeGraph::build()` — 从编译时 JSON 构建知识图谱
- `query_subgraph(concept_id)` — 2-hop 子图查询
- `search(query)` — 模糊搜索（名称+描述）
- `path_between(from, to)` — A* 最短路径
- `compute_layout()` — BFS 层次布局计算

## 属于领域
- knowledge / AI
