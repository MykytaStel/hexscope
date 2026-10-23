//! Metadata findings shared by every format parser.

use crate::model::NodeId;

/// One value a file reveals and the node that provides its source evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentFact {
    /// A stable kind such as `author`, `location`, or `artist`.
    pub kind: &'static str,
    pub text: String,
    pub node: NodeId,
}
