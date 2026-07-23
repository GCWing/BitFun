# taiji-knowledge-graph — Petgraph-Backed Knowledge Graph

Auto-builds a three-layer concept→strategy→case graph from 7 Agent JSON schemas and golden tick data. Exposes 3 Tauri commands for Cytoscape.js frontend rendering.

## Architecture Position

```
taiji-knowledge-graph (standalone — zero taiji internal deps)
  ├── petgraph::StableGraph<ConceptNode, RelationEdge>
  ├── build.rs (generates graph JSON at compile time)
  └── 3 Tauri commands → MiniApp/taiji-knowledge-graph
```

## Core Types

```rust
pub struct ConceptNode {
    pub id: String,
    pub name: String,
    pub category: NodeCategory,  // TheoryConcept | StrategyRule | DataIndicator
    pub description: String,
    pub sources: Vec<String>,
}

pub enum RelationType { DerivesFrom, Uses, CorrelatesWith }
```

## Tauri Commands

| Command | Description |
|---------|-------------|
| `taiji_kg_query(concept_id)` | 2-hop subgraph around a concept node |
| `taiji_kg_layout()` | Breadthfirst layout + Cytoscape.js elements JSON |
| `taiji_kg_search(query)` | Fuzzy search across node names/descriptions |

## Quick Start

```rust
use taiji_knowledge_graph::KnowledgeGraph;

let kg = KnowledgeGraph::build();
let subgraph = kg.query_subgraph("theory_magnet").unwrap();
let results = kg.search("三推");
```

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
