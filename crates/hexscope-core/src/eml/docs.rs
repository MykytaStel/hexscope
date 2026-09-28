//! What each part of an email is. Header lines are labelled with their own
//! names, so a child of a "headers" node is a header: a known one gets its
//! own explanation, any other the general one.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const RFC5322: &str = "https://www.rfc-editor.org/rfc/rfc5322";
const RFC2045: &str = "https://www.rfc-editor.org/rfc/rfc2045";
const RFC2046: &str = "https://www.rfc-editor.org/rfc/rfc2046";
const RFC2183: &str = "https://www.rfc-editor.org/rfc/rfc2183";
const RFC8601: &str = "https://www.rfc-editor.org/rfc/rfc8601";

const fn msg(text: &'static str, cite: &'static str) -> Doc {
    Doc::new(text).cite(cite, RFC5322)
}

const PARTS: Table = &[
    (
        "email",
        msg(
            "An email saved as a file: header lines about who sent it, when and through which servers, then the message, which may be split into parts.",
            "RFC 5322 §2.1",
        ),
    ),
    (
        "headers",
        msg(
            "The header: one line per field, a name, a colon and its value; a line that starts with a space continues the one before.",
            "RFC 5322 §2.2",
        ),
    ),
    (
        "parts",
        Doc::new(
            "A message in parts, each with headers of its own, between lines that start with the boundary the type names.",
        )
        .cite("RFC 2046 §5.1", RFC2046),
    ),
    (
        "part *",
        Doc::new("One part of the message: the text, the same text as a web page, or a file.")
            .cite("RFC 2046 §5.1.1", RFC2046),
    ),
    (
        "body",
        Doc::new("The message itself, as the type before it says: plain text, or a web page.")
            .cite("RFC 2045 §5", RFC2045),
    ),
    (
        "forwarded message",
        Doc::new(
            "A message sent along inside this one, whole: headers and body of its own. Its lines say who sent it first, not who sent this.",
        )
        .cite("RFC 2046 §5.2.1", RFC2046),
    ),
    (
        "attachment",
        Doc::new(
            "A file sent with the message, written out as text — base64, most often. It opens here as a file of its own.",
        )
        .cite("RFC 2183 §2", RFC2183),
    ),
];

const HEADERS: Table = &[
    (
        "Received",
        msg(
            "A server the message passed through, stamped at the top as it went. The lowest line is the first server, with the address the sender connected from.",
            "RFC 5322 §3.6.7",
        ),
    ),
    (
        "From",
        msg("Who the message says it is from. Anyone can write anything here; the checks below say whether the domain agreed.", "RFC 5322 §3.6.2"),
    ),
    ("To", msg("Who it was sent to.", "RFC 5322 §3.6.3")),
    ("Cc", msg("Who was sent a copy, for everyone to see.", "RFC 5322 §3.6.3")),
    ("Subject", msg("The subject line.", "RFC 5322 §3.6.5")),
    (
        "Date",
        msg("When the sender's computer says it was sent, and its time zone: the offset from UTC at the end.", "RFC 5322 §3.6.1"),
    ),
    (
        "Message-ID",
        msg("The message's own name. The part after @ is often the sending server — and sometimes the sender's computer.", "RFC 5322 §3.6.4"),
    ),
    ("In-Reply-To", msg("The ID of the message this one answers.", "RFC 5322 §3.6.4")),
    ("References", msg("The IDs of the messages before this one in the conversation.", "RFC 5322 §3.6.4")),
    (
        "Reply-To",
        msg("Where replies go, when not to the sender.", "RFC 5322 §3.6.2"),
    ),
    (
        "Return-Path",
        msg("Where a message that cannot be delivered is sent back to: the envelope's sender.", "RFC 5322 §3.6.7"),
    ),
    (
        "Authentication-Results",
        Doc::new(
            "What the receiving server found when it checked the sender's domain: SPF (the server was allowed to send), DKIM (a signature), DMARC (the policy).",
        )
        .cite("RFC 8601 §2", RFC8601),
    ),
    (
        "DKIM-Signature",
        Doc::new("A signature by the sending domain over the message, which a receiver checks with a key the domain publishes.")
            .cite("RFC 6376 §3.5", "https://www.rfc-editor.org/rfc/rfc6376"),
    ),
    (
        "X-Mailer",
        Doc::new("The program that wrote the message, and often its version: the sender's mail app."),
    ),
    ("User-Agent", Doc::new("The program that wrote the message, and often its version and system.")),
    (
        "X-Originating-IP",
        Doc::new("The address the sender's computer connected from, as webmail services record it."),
    ),
    (
        "MIME-Version",
        Doc::new("Says the message uses MIME: types, encodings and parts.").cite("RFC 2045 §4", RFC2045),
    ),
    (
        "Content-Type",
        Doc::new("What this part is — text, a web page, a picture, parts — and its character set or boundary.")
            .cite("RFC 2045 §5", RFC2045),
    ),
    (
        "Content-Transfer-Encoding",
        Doc::new("How the part is written as text: base64, quoted-printable, or as it is.").cite("RFC 2045 §6", RFC2045),
    ),
    (
        "Content-Disposition",
        Doc::new("Whether the part is shown in the message or attached, and the attached file's name.")
            .cite("RFC 2183 §2", RFC2183),
    ),
];

const HEADER: Doc = msg(
    "A header line: its name, a colon, and its value. Names starting with X- are a program's own.",
    "RFC 5322 §2.2",
);

const PROBLEMS: Table = &[
    (
        "replies go to another domain",
        Doc::new(
            "Replies go to a different domain than the one the message says it is from — a common sign of a message pretending to be someone else.",
        )
        .concern(Concern::Hidden),
    ),
    (
        "the sender's domain did not vouch for it",
        Doc::new(
            "The receiving server asked the domain in From, and it did not confirm the message: it may not be from who it says.",
        )
        .cite("RFC 8601 §2.7", RFC8601)
        .concern(Concern::Hidden),
    ),
    (
        "no closing boundary",
        Doc::new("The last part has no closing boundary line: the message may be cut short.").concern(Concern::Damage),
    ),
];

pub(crate) fn describe(tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    let label = node.label.as_str();
    if problem {
        return lookup(PROBLEMS, label, true);
    }
    let parent = node
        .parent
        .and_then(|p| tree.try_get(p))
        .map(|p| p.label.as_str());
    if parent == Some("headers") {
        return Some(
            HEADERS
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(label))
                .map_or(HEADER, |(_, d)| *d),
        );
    }
    lookup(PARTS, label, false)
}
