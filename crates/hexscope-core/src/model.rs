/// Index into `ParseTree::nodes`.
pub type NodeId = u32;

/// A half-open span of the source file: `[start, start + len)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub len: u64,
}

impl ByteRange {
    pub fn new(start: u64, len: u64) -> Self {
        Self { start, len }
    }

    /// The exclusive end, or None if the range cannot be represented.
    pub fn checked_end(&self) -> Option<u64> {
        self.start.checked_add(self.len)
    }

    /// A saturating endpoint for display and ordering. Use checked_end
    /// before validating or reading an untrusted range.
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.len)
    }
}

/// A decoded field value. Large byte payloads are not copied — only their
/// length is recorded, because the bytes stay addressable via the node range.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    U64(u64),
    Text(String),
    /// Length in bytes of an opaque payload.
    Bytes(u64),
    /// A numeric value with a known meaning, e.g. color type 6 = "RGBA".
    Enum {
        raw: u64,
        name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Groups other nodes, e.g. a chunk.
    Container,
    /// A decoded leaf value.
    Field,
    /// The bytes were read, but they break a rule of the format or look
    /// wrong: a CRC mismatch, an undefined enum value, a damaged signature.
    /// Nothing is lost.
    Warning,
    /// These bytes could not be read as what they claim to be — truncated,
    /// out of range, undecodable. Parsing continued elsewhere.
    ///
    /// A tool limitation is neither: it is not a problem with the file.
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub label: String,
    pub range: ByteRange,
    pub value: Option<Value>,
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
}

/// Arena of nodes. The first node added becomes the root.
#[derive(Debug)]
pub struct ParseTree {
    nodes: Vec<Node>,
    node_limit: usize,
    limit_node: Option<NodeId>,
}

impl Default for ParseTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ParseTree {
    pub fn new() -> Self {
        Self::with_node_limit(500_000)
    }

    /// A shared tree allocation budget. Reaching it records an error node;
    /// subsequent additions reuse that node rather than growing the arena.
    pub fn with_node_limit(node_limit: usize) -> Self {
        Self {
            nodes: Vec::new(),
            node_limit: node_limit.clamp(1, u32::MAX as usize - 1),
            limit_node: None,
        }
    }

    pub fn add(
        &mut self,
        parent: Option<NodeId>,
        label: impl Into<String>,
        range: ByteRange,
        kind: NodeKind,
        value: Option<Value>,
    ) -> NodeId {
        if let Some(id) = self.limit_node {
            return id;
        }
        if self.nodes.len() >= self.node_limit {
            return self.record_node_limit(range);
        }
        self.push_node(parent, label.into(), range, kind, value)
    }

    // Keep the arena insertion shared between label types and call sites in
    // size-optimized browser builds, rather than duplicating the allocation
    // and parent-link logic in every parser.
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn push_node(
        &mut self,
        parent: Option<NodeId>,
        label: String,
        range: ByteRange,
        kind: NodeKind,
        value: Option<Value>,
    ) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            id,
            parent,
            label,
            range,
            value,
            kind,
            children: Vec::new(),
        });
        if let Some(p) = parent {
            self.nodes[p as usize].children.push(id);
        }
        id
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn record_node_limit(&mut self, range: ByteRange) -> NodeId {
        let id = self.push_node(
            self.root(),
            "parse tree node limit reached; remaining structures were not inspected".into(),
            range,
            NodeKind::Error,
            None,
        );
        self.limit_node = Some(id);
        id
    }

    /// Records damage: bytes that could not be read as what they claim to be.
    pub fn error(&mut self, parent: NodeId, label: impl Into<String>, range: ByteRange) -> NodeId {
        self.add(Some(parent), label, range, NodeKind::Error, None)
    }

    /// Records bytes that were read but break a rule of the format.
    pub fn warning(
        &mut self,
        parent: NodeId,
        label: impl Into<String>,
        range: ByteRange,
    ) -> NodeId {
        self.add(Some(parent), label, range, NodeKind::Warning, None)
    }

    /// Renames a node and sets its range, for a container whose extent is
    /// known only once its fields are read. An id not from this tree is ignored.
    pub fn relabel(&mut self, id: NodeId, label: impl Into<String>, range: ByteRange) {
        if self.limit_node == Some(id) {
            return;
        }
        if let Some(n) = self.nodes.get_mut(id as usize) {
            n.label = label.into();
            n.range = range;
        }
    }

    /// Sets a node's range after the fact. An id not from this tree is ignored.
    pub fn set_range(&mut self, id: NodeId, range: ByteRange) {
        if self.limit_node == Some(id) {
            return;
        }
        if let Some(n) = self.nodes.get_mut(id as usize) {
            n.range = range;
        }
    }

    /// Replaces ranges when a decoded view came from a compressed source and
    /// its byte positions cannot map one-to-one to the original stream.
    pub(crate) fn map_ranges(&mut self, mut map: impl FnMut(ByteRange) -> ByteRange) {
        for node in &mut self.nodes {
            node.range = map(node.range);
        }
    }

    /// Sets a node's value after the fact, for a summary known only once its
    /// children are read. An id not from this tree is ignored.
    pub fn set_value(&mut self, id: NodeId, value: Option<Value>) {
        if self.limit_node == Some(id) {
            return;
        }
        if let Some(n) = self.nodes.get_mut(id as usize) {
            n.value = value;
        }
    }

    /// The node with this id.
    ///
    /// # Panics
    ///
    /// If `id` was not returned by [`ParseTree::add`] on this tree — the same
    /// contract as indexing a `Vec`. Use [`ParseTree::try_get`] for ids from
    /// outside, such as ones round-tripped through JavaScript.
    pub fn get(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn try_get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize)
    }

    pub fn root(&self) -> Option<NodeId> {
        if self.nodes.is_empty() { None } else { Some(0) }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Copies every node below `other`'s root under `parent`, shifting ranges
    /// by `offset`: a file embedded in this one, parsed on its own, placed
    /// where its bytes actually are.
    pub fn graft(&mut self, parent: NodeId, other: &ParseTree, offset: u64) {
        self.graft_inner(parent, other, offset, false);
    }

    /// As [`ParseTree::graft`], and returns the new node ID for every node in
    /// `other`. The embedded root has no corresponding node; its children map
    /// to nodes below `parent`.
    pub fn graft_with_ids(
        &mut self,
        parent: NodeId,
        other: &ParseTree,
        offset: u64,
    ) -> Vec<NodeId> {
        self.graft_inner(parent, other, offset, true)
    }

    fn graft_inner(
        &mut self,
        parent: NodeId,
        other: &ParseTree,
        offset: u64,
        return_mapping: bool,
    ) -> Vec<NodeId> {
        let Some(root) = other.root() else {
            return Vec::new();
        };
        if let Some(id) = self.limit_node {
            if return_mapping {
                let mut mapping = vec![id; other.nodes.len()];
                mapping[root as usize] = 0;
                return mapping;
            }
            return Vec::new();
        }
        let mut mapping = return_mapping.then(|| vec![0; other.nodes.len()]);
        let mut stack: Vec<(NodeId, NodeId)> = other
            .get(root)
            .children
            .iter()
            .rev()
            .map(|&c| (c, parent))
            .collect();
        while let Some((id, into)) = stack.pop() {
            let n = other.get(id);
            let shifted = n
                .range
                .start
                .checked_add(offset)
                .and_then(|start| start.checked_add(n.range.len).map(|_| start));
            let copy = if let Some(start) = shifted {
                self.add(
                    Some(into),
                    n.label.clone(),
                    ByteRange::new(start, n.range.len),
                    n.kind,
                    n.value.clone(),
                )
            } else {
                self.error(
                    into,
                    "embedded byte range cannot be represented",
                    ByteRange::new(offset, 0),
                )
            };
            if let Some(mapping) = mapping.as_mut() {
                mapping[id as usize] = copy;
            }
            if self.limit_node.is_some() && !return_mapping {
                break;
            }
            stack.extend(n.children.iter().rev().map(|&c| (c, copy)));
        }
        mapping.unwrap_or_default()
    }

    /// Nodes in insertion order. Used by the WASM bridge to flatten the tree.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
}

/// `items` in the order of `key`, ties kept in the order they came: sorted
/// by `(key, index)` pairs, the one sort the WebAssembly builds carry, rather
/// than a stable sort of each item type, a few kilobytes apiece.
pub(crate) fn sorted_by<T>(items: Vec<T>, key: impl Fn(&T) -> u64) -> Vec<T> {
    let mut order: Vec<(u64, usize)> = items.iter().enumerate().map(|(i, t)| (key(t), i)).collect();
    order.sort_unstable();
    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|(_, i)| slots[i].take())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_parent_child_tree() {
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "PNG",
            ByteRange::new(0, 100),
            NodeKind::Container,
            None,
        );
        let field = tree.add(
            Some(root),
            "width",
            ByteRange::new(16, 4),
            NodeKind::Field,
            Some(Value::U64(1920)),
        );

        assert_eq!(tree.root(), Some(root));
        assert_eq!(tree.get(root).children, vec![field]);
        assert_eq!(tree.get(field).parent, Some(root));
        assert_eq!(tree.get(field).value, Some(Value::U64(1920)));
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn graft_copies_a_subtree_with_shifted_ranges_in_order() {
        let mut inner = ParseTree::new();
        let r = inner.add(
            None,
            "JPEG",
            ByteRange::new(0, 10),
            NodeKind::Container,
            None,
        );
        let a = inner.add(Some(r), "SOI", ByteRange::new(0, 2), NodeKind::Field, None);
        let b = inner.add(
            Some(r),
            "APP0",
            ByteRange::new(2, 8),
            NodeKind::Container,
            None,
        );
        inner.add(
            Some(b),
            "length",
            ByteRange::new(4, 2),
            NodeKind::Field,
            None,
        );
        let _ = a;

        let mut outer = ParseTree::new();
        let root = outer.add(
            None,
            "thumbnail",
            ByteRange::new(100, 10),
            NodeKind::Container,
            None,
        );
        outer.graft(root, &inner, 100);

        let labels: Vec<&str> = outer.nodes().iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, ["thumbnail", "SOI", "APP0", "length"]);
        let length = &outer.nodes()[3];
        assert_eq!(length.range, ByteRange::new(104, 2));
        assert_eq!(outer.get(length.parent.unwrap()).label, "APP0");
    }

    #[test]
    fn overflowing_range_has_no_checked_end() {
        let range = ByteRange::new(u64::MAX - 1, 3);
        assert_eq!(range.checked_end(), None);
        assert_eq!(range.end(), u64::MAX);
        assert_eq!(ByteRange::new(u64::MAX, 0).checked_end(), Some(u64::MAX));
    }

    #[test]
    fn graft_reports_an_unrepresentable_shift() {
        let mut inner = ParseTree::new();
        let root = inner.add(
            None,
            "inner",
            ByteRange::new(0, 10),
            NodeKind::Container,
            None,
        );
        inner.add(
            Some(root),
            "field",
            ByteRange::new(8, 2),
            NodeKind::Field,
            None,
        );
        let mut outer = ParseTree::new();
        let root = outer.add(
            None,
            "outer",
            ByteRange::new(0, 10),
            NodeKind::Container,
            None,
        );
        let ids = outer.graft_with_ids(root, &inner, u64::MAX - 5);
        assert_eq!(outer.get(ids[1]).kind, NodeKind::Error);
    }

    #[test]
    fn exhausted_tree_does_not_convert_discarded_labels() {
        struct UnusedLabel;

        impl From<UnusedLabel> for String {
            fn from(_: UnusedLabel) -> Self {
                panic!("discarded labels must not be converted")
            }
        }

        let mut tree = ParseTree::with_node_limit(1);
        let range = ByteRange::new(0, 1);
        let root = tree.add(None, "root", range, NodeKind::Container, None);
        let limit = tree.add(Some(root), UnusedLabel, range, NodeKind::Field, None);
        assert_eq!(
            tree.add(Some(root), UnusedLabel, range, NodeKind::Field, None),
            limit
        );
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get(root).children, vec![limit]);
        assert_eq!(tree.get(limit).kind, NodeKind::Error);
    }

    #[test]
    fn limit_node_cannot_be_overwritten_by_post_add_updates() {
        let mut tree = ParseTree::with_node_limit(1);
        let root = tree.add(
            None,
            "root",
            ByteRange::new(0, 20),
            NodeKind::Container,
            None,
        );
        let limit = tree.add(
            Some(root),
            "child",
            ByteRange::new(4, 2),
            NodeKind::Field,
            None,
        );
        let original = tree.get(limit).clone();
        let reused = tree.add(
            Some(limit),
            "other",
            ByteRange::new(10, 1),
            NodeKind::Field,
            None,
        );
        assert_eq!(reused, limit);
        tree.relabel(reused, "rewritten", ByteRange::new(11, 1));
        tree.set_range(reused, ByteRange::new(12, 1));
        tree.set_value(reused, Some(Value::U64(42)));
        assert_eq!(tree.get(limit), &original);
        assert_eq!(tree.get(limit).kind, NodeKind::Error);
        assert_eq!(tree.get(root).children, [limit]);
        assert_eq!(tree.len(), 2);
        tree.set_value(root, Some(Value::U64(7)));
        assert_eq!(tree.get(root).value, Some(Value::U64(7)));
    }

    #[test]
    fn exhausted_graft_preserves_limit_node_and_mapping_contract() {
        let mut inner = ParseTree::new();
        let root = inner.add(
            None,
            "inner",
            ByteRange::new(0, 4),
            NodeKind::Container,
            None,
        );
        let child = inner.add(
            Some(root),
            "child",
            ByteRange::new(0, 2),
            NodeKind::Container,
            None,
        );
        inner.add(
            Some(child),
            "value",
            ByteRange::new(1, 1),
            NodeKind::Field,
            None,
        );
        let mut outer = ParseTree::with_node_limit(1);
        let root = outer.add(
            None,
            "outer",
            ByteRange::new(0, 4),
            NodeKind::Container,
            None,
        );
        outer.graft(root, &inner, 0);
        let limit = outer.get(root).children[0];
        let original = outer.get(limit).clone();
        assert_eq!(outer.graft_with_ids(root, &inner, 0), [0, limit, limit]);
        outer.graft(root, &inner, 0);
        assert_eq!(outer.get(limit), &original);
        assert_eq!(outer.len(), 2);
    }

    #[test]
    fn byte_range_end_is_exclusive() {
        assert_eq!(ByteRange::new(8, 25).end(), 33);
    }
}
