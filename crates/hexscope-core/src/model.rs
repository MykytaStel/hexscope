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
    /// File is readable but suspicious, e.g. a CRC mismatch.
    Warning,
    /// This region could not be read; parsing continued elsewhere.
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

    pub fn get(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
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
    fn byte_range_end_is_exclusive() {
        assert_eq!(ByteRange::new(8, 25).end(), 33);
    }
}
