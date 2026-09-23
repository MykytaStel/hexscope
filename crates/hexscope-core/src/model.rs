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

    pub fn end(&self) -> u64 {
        self.start + self.len
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
#[derive(Debug, Default)]
pub struct ParseTree {
    nodes: Vec<Node>,
}

impl ParseTree {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add(
        &mut self,
        parent: Option<NodeId>,
        label: impl Into<String>,
        range: ByteRange,
        kind: NodeKind,
        value: Option<Value>,
    ) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            id,
            parent,
            label: label.into(),
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

    /// Sets a node's value after the fact, for a summary known only once its
    /// children are read. An id not from this tree is ignored.
    pub fn set_value(&mut self, id: NodeId, value: Option<Value>) {
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
        let Some(root) = other.root() else { return };
        let mut stack: Vec<(NodeId, NodeId)> = other
            .get(root)
            .children
            .iter()
            .rev()
            .map(|&c| (c, parent))
            .collect();
        while let Some((id, into)) = stack.pop() {
            let n = other.get(id);
            let copy = self.add(
                Some(into),
                n.label.clone(),
                ByteRange::new(n.range.start + offset, n.range.len),
                n.kind,
                n.value.clone(),
            );
            stack.extend(n.children.iter().rev().map(|&c| (c, copy)));
        }
    }

    /// Nodes in insertion order. Used by the WASM bridge to flatten the tree.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
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
    fn byte_range_end_is_exclusive() {
        assert_eq!(ByteRange::new(8, 25).end(), 33);
    }
}
