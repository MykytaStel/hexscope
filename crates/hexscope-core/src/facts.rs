//! Metadata findings shared by every format parser.

use crate::model::NodeId;

/// One value a file reveals and the node that provides its source evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentFact {
    /// A stable code such as `author`, `location`, or `artist`.
    pub kind: &'static str,
    /// The value found in the file, kept separate from display wording.
    pub text: String,
    pub node: NodeId,
}
