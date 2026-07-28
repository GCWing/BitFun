use crate::error::Result;
use crate::node::{ComputeNode, NodeConfig};
use std::collections::{HashMap, HashSet};

pub type NodeConstructor = Box<dyn Fn(&NodeConfig) -> Result<Box<dyn ComputeNode>> + Send + Sync>;

// ── Descriptor ─────────────────────────────────────────────────────────────────
// Mirrors BitFun's HarnessProviderDescriptor / ToolProviderGroupPlan: a static
// compile-time registration entry that pairs a type name with its constructor.

/// Static node registration descriptor.
///
/// Inspired by BitFun's `HarnessProviderDescriptor` and `ToolProviderGroupPlan`
/// patterns: a const-compatible struct that binds a `type_name` to its boxed
/// constructor so that node definitions can live as declarative static arrays.
///
/// Use [`NodeFactoryBuilder`] to install these into a [`NodeFactory`] with
/// duplicate detection at build time.
pub struct NodeDescriptor {
    pub type_name: &'static str,
    pub constructor: NodeConstructor,
}

impl std::fmt::Debug for NodeDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeDescriptor")
            .field("type_name", &self.type_name)
            .finish()
    }
}

// ── Builder ────────────────────────────────────────────────────────────────────
// Mirrors BitFun's HarnessRegistryBuilder: install → build with validation.

/// Builds a [`NodeFactory`] from declarative [`NodeDescriptor`]s.
///
/// Inspired by BitFun's `HarnessRegistryBuilder` pattern:
/// call [`install`](Self::install) for each node, then [`build`](Self::build)
/// validates uniqueness and produces a ready-to-use factory.
/// Duplicate `type_name` values are rejected at build time.
///
/// # Example
///
/// ```ignore
/// let factory = NodeFactoryBuilder::new()
///     .install(NodeDescriptor {
///         type_name: "ma_cross",
///         constructor: Box::new(|config| {
///             let mut node = MaCross::new("ma_cross");
///             let store = StateStore::new();
///             node.on_init(config, &store)?;
///             Ok(Box::new(node))
///         }),
///     })
///     .build()?;
/// ```
pub struct NodeFactoryBuilder {
    descriptors: Vec<NodeDescriptor>,
}

impl Default for NodeFactoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeFactoryBuilder {
    pub fn new() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }

    /// Append a node descriptor. Order is preserved for predictable iteration.
    pub fn install(mut self, descriptor: NodeDescriptor) -> Self {
        self.descriptors.push(descriptor);
        self
    }

    /// Build the [`NodeFactory`], rejecting duplicate `type_name` entries.
    pub fn build(self) -> Result<NodeFactory> {
        let mut seen = HashSet::new();
        let mut registry = HashMap::with_capacity(self.descriptors.len());

        for desc in self.descriptors {
            if !seen.insert(desc.type_name) {
                return Err(crate::error::TaijiError::Config(format!(
                    "duplicate node type '{}'",
                    desc.type_name
                )));
            }
            registry.insert(desc.type_name.to_string(), desc.constructor);
        }

        Ok(NodeFactory { registry })
    }
}

// ── Factory ────────────────────────────────────────────────────────────────────
// Mirrors BitFun's HarnessRegistry: a trait-object construction registry.

/// Factory that creates [`ComputeNode`] instances by type name.
///
/// Inspired by BitFun's `HarnessRegistry`: a named-constructor registry that
/// produces trait-object instances. Registration can be done imperatively via
/// [`register`](Self::register) or declaratively via [`NodeFactoryBuilder`].
///
/// The [`register_node!`] macro provides concise single-line registration
/// reminiscent of BitFun's `install_provider` chained builder calls.
pub struct NodeFactory {
    registry: HashMap<String, NodeConstructor>,
}

impl Default for NodeFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeFactory {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
        }
    }

    /// Register a node constructor for `type_name`.
    ///
    /// Does not check for duplicates — use [`NodeFactoryBuilder`] when
    /// duplicate detection is needed.
    pub fn register(&mut self, type_name: &str, ctor: NodeConstructor) {
        self.registry.insert(type_name.to_string(), ctor);
    }

    /// Create a [`ComputeNode`] by `type_name`, passing `config` to its constructor.
    ///
    /// Returns an error when the type is not registered.
    pub fn create(&self, type_name: &str, config: &NodeConfig) -> Result<Box<dyn ComputeNode>> {
        match self.registry.get(type_name) {
            Some(ctor) => ctor(config),
            None => Err(crate::error::TaijiError::Config(format!(
                "unknown node type: '{}'",
                type_name
            ))),
        }
    }

    /// All registered type names, in arbitrary order.
    pub fn list_types(&self) -> Vec<&str> {
        self.registry.keys().map(|s| s.as_str()).collect()
    }

    /// Whether `type_name` is registered.
    pub fn contains(&self, type_name: &str) -> bool {
        self.registry.contains_key(type_name)
    }
}

// ── Macro ──────────────────────────────────────────────────────────────────────

/// 一行注册 ComputeNode 到 NodeFactory。
///
/// 用法：
/// ```ignore
/// register_node!(factory, "ma_cross", taiji_example::MaCross, "ma_cross");
/// register_node!(factory, "bar_node", taiji_bar::BarNode, "bar_node");
/// ```
#[macro_export]
macro_rules! register_node {
    ($factory:expr, $type_name:expr, $node_ty:ty, $id:expr) => {
        $factory.register(
            $type_name,
            Box::new(|config: &$crate::node::NodeConfig| -> $crate::error::Result<Box<dyn $crate::node::ComputeNode>> {
                let mut node = <$node_ty>::new($id.into());
                let store = $crate::store::StateStore::new();
                node.on_init(config, &store)?;
                Ok(Box::new(node))
            }),
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::node::{ComputeNode, NodeConfig};
    use crate::store::StateStore;
    use crate::types::bar::{Freq, RawBar};
    use crate::types::state::StateKey;

    struct MockNode {
        id: String,
    }
    impl ComputeNode for MockNode {
        fn id(&self) -> String {
            self.id.clone()
        }
        fn name(&self) -> &'static str {
            "mock"
        }
        fn input_keys(&self) -> Vec<StateKey> {
            vec![]
        }
        fn output_keys(&self) -> Vec<StateKey> {
            vec![]
        }
        fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
            Ok(())
        }
        fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
            Ok(())
        }
    }

    fn mock_ctor(id: &str) -> NodeConstructor {
        let id = id.to_string();
        Box::new(move |_: &NodeConfig| Ok(Box::new(MockNode { id: id.clone() })))
    }

    #[test]
    fn test_register_and_create() {
        let mut factory = NodeFactory::new();
        factory.register("mock", mock_ctor("mock1"));
        let node = factory.create("mock", &NodeConfig::new()).unwrap();
        assert_eq!(node.id(), "mock1");
    }

    #[test]
    fn test_unknown_type() {
        let factory = NodeFactory::new();
        assert!(factory.create("nonexistent", &NodeConfig::new()).is_err());
    }

    #[test]
    fn test_list_types() {
        let mut factory = NodeFactory::new();
        factory.register("a", mock_ctor("a"));
        factory.register("b", mock_ctor("b"));
        assert_eq!(factory.list_types().len(), 2);
    }

    #[test]
    fn test_contains() {
        let mut factory = NodeFactory::new();
        factory.register("a", mock_ctor("a"));
        assert!(factory.contains("a"));
        assert!(!factory.contains("b"));
    }

    // ── Builder tests ────────────────────────────────────────────────────────

    #[test]
    fn builder_installs_nodes_and_produces_factory() {
        let factory = NodeFactoryBuilder::new()
            .install(NodeDescriptor {
                type_name: "a",
                constructor: mock_ctor("a"),
            })
            .install(NodeDescriptor {
                type_name: "b",
                constructor: mock_ctor("b"),
            })
            .build()
            .unwrap();

        let node = factory.create("a", &NodeConfig::new()).unwrap();
        assert_eq!(node.id(), "a");
        assert_eq!(factory.list_types().len(), 2);
    }

    #[test]
    fn builder_rejects_duplicates() {
        let result = NodeFactoryBuilder::new()
            .install(NodeDescriptor {
                type_name: "dup",
                constructor: mock_ctor("first"),
            })
            .install(NodeDescriptor {
                type_name: "dup",
                constructor: mock_ctor("second"),
            })
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn builder_creates_empty_factory() {
        let factory = NodeFactoryBuilder::new().build().unwrap();
        assert_eq!(factory.list_types().len(), 0);
    }

    #[test]
    fn builder_empty_factory_rejects_unknown_type() {
        let factory = NodeFactoryBuilder::new().build().unwrap();
        assert!(factory.create("nonexistent", &NodeConfig::new()).is_err());
    }
}
