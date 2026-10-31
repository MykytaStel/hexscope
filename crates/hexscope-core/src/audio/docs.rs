//! Plain explanations for the audio metadata this parser can read.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const ID3: &str = "https://id3.org/id3v2.4.0-structure";
const FLAC: &str = "https://www.rfc-editor.org/rfc/rfc9639.html";
const OGG: &str = "https://www.rfc-editor.org/rfc/rfc3533.html";
const VORBIS: &str = "https://www.xiph.org/vorbis/doc/Vorbis_I_spec.html";
const OPUS: &str = "https://www.rfc-editor.org/rfc/rfc7845.html";

const fn id3(text: &'static str) -> Doc {
    Doc::new(text).cite("ID3v2.4.0", ID3)
}

const fn flac(text: &'static str) -> Doc {
    Doc::new(text).cite("RFC 9639, FLAC", FLAC)
}

const fn ogg(text: &'static str) -> Doc {
    Doc::new(text).cite("RFC 3533, Ogg", OGG)
}

const fn vorbis(text: &'static str) -> Doc {
    Doc::new(text).cite("Vorbis I", VORBIS)
}

const fn opus(text: &'static str) -> Doc {
    Doc::new(text).cite("RFC 7845, Ogg Opus", OPUS)
}

const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const fn oddity(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Oddity)
}

const TABLE: Table = &[
    (
        "MP3 audio",
        id3(
            "An MP3 stream: ID3 tags beside compressed audio frames, which this view does not decode.",
        ),
    ),
    (
        "ID3v2.* tag",
        id3(
            "An ID3 tag: a versioned set of named frames for the title, people, dates and other details.",
        ),
    ),
    (
        "ID3 frame * not inspected",
        id3(
            "This ID3 frame can hold metadata this view does not read, so the tag is only partly inspected.",
        ),
    ),
    (
        "ID3 frame *",
        id3("An ID3 frame: a named piece of text or other metadata inside the tag."),
    ),
    (
        "ID3v1 tag",
        id3(
            "The older fixed-size ID3 tag at the end of an MP3, with title, artist, album and year.",
        ),
    ),
    (
        "audio frames (not decoded)",
        id3(
            "The compressed sound frames; this view shows their bytes without decoding or playing them.",
        ),
    ),
    (
        "FLAC audio",
        flac("A FLAC stream: metadata blocks followed by losslessly compressed sound frames."),
    ),
    (
        "FLAC STREAMINFO block",
        flac(
            "The required stream description: sample rate, channels, sample count and frame sizes.",
        ),
    ),
    (
        "FLAC padding block",
        flac(
            "Unused space between metadata and sound frames, available for later metadata changes.",
        ),
    ),
    (
        "FLAC seek table block",
        flac("Optional sample positions that let a player seek through the FLAC stream."),
    ),
    (
        "FLAC Vorbis comments block",
        flac(
            "Text tags stored as Vorbis comments, including title, artist and other user metadata.",
        ),
    ),
    (
        "FLAC picture block",
        flac("An embedded picture such as cover art; a clean copy removes the entire block."),
    ),
    (
        "FLAC picture data removed with its block",
        flac("The image bytes and any metadata inside them; this clean copy removes them whole."),
    ),
    (
        "FLAC * block",
        flac("A FLAC metadata block; this view does not inspect its contents."),
    ),
    (
        "FLAC metadata block content not inspected",
        flac(
            "This block can carry application data, cue metadata or extensions that this view does not inspect.",
        ),
    ),
    (
        "Ogg media stream",
        ogg(
            "Ogg is a container for audio, video or other streams; this view reads supported audio tags only.",
        ),
    ),
    (
        "Ogg page *",
        ogg(
            "An Ogg page carries part of a logical stream, including packet boundaries and timing.",
        ),
    ),
    (
        "Ogg identification packet",
        ogg("The first packet identifies which codec's compressed audio follows."),
    ),
    (
        "Vorbis comment packet",
        vorbis("The Vorbis comment packet carries text tags such as title, artist and encoder."),
    ),
    (
        "Opus tags packet",
        opus("The Opus tags packet carries text tags such as title, artist and encoder."),
    ),
    (
        "unsupported Ogg codec; metadata tags are not inspected",
        ogg("This Ogg stream uses a codec this view does not inspect, so its tags may be missing."),
    ),
    (
        "remaining Ogg packets (not decoded)",
        ogg("The later packets hold compressed media; this view leaves them undecoded."),
    ),
    (
        "title: *",
        id3("The recording's title, stored in an ID3 or Vorbis comment tag."),
    ),
    (
        "artist: *",
        id3("A performer named in the recording's metadata."),
    ),
    (
        "album: *",
        id3("The album named in the recording's metadata."),
    ),
    (
        "album-artist: *",
        id3("The main artist credited for the album."),
    ),
    (
        "year: *",
        id3("A year or date recorded in the audio metadata."),
    ),
    (
        "track: *",
        id3("The recording's track number within an album."),
    ),
    (
        "disc: *",
        id3("The recording's disc number within a multi-disc album."),
    ),
    (
        "genre: *",
        id3("A genre label stored in the recording's metadata."),
    ),
    ("comment: *", id3("A comment stored with the recording.")),
    (
        "composer: *",
        id3("The composer named in the recording's metadata."),
    ),
    (
        "encoder: *",
        id3("The encoder or software named in the recording's metadata."),
    ),
    (
        "organization: *",
        id3("An organization named in the recording's metadata."),
    ),
    (
        "location: *",
        id3("A location written in the recording's metadata."),
    ),
    (
        "audio-tag: *",
        id3("An audio metadata field this view preserves but does not assign a standard name to."),
    ),
    (
        "ID3 tag uses an unsupported compression or unsynchronisation transform",
        id3("The ID3 tag changes its stored bytes in a way this view does not reverse."),
    ),
    (
        "frame transform not inspected",
        id3("This frame changes its stored bytes in a way this view does not reverse."),
    ),
    (
        "* not inspected",
        ogg("These bytes may contain metadata this view does not interpret."),
    ),
    (
        "uninspected bytes after comment fields",
        vorbis("Extra bytes follow the comment fields and are not interpreted here."),
    ),
    (
        "metadata fact count exceeds the 256-item inspection limit",
        id3("More metadata values are present than the safe display limit allows."),
    ),
    (
        "* exceeds the * inspection limit",
        id3("This part is larger than the safe inspection limit, so some metadata was skipped."),
    ),
    (
        "* count exceeds the inspection limit",
        id3("There are more metadata parts than this view inspects."),
    ),
    (
        "Ogg header pages exceed the inspection limit",
        ogg("There are more Ogg pages than this view inspects."),
    ),
    (
        "multiplexed Ogg streams are not inspected",
        ogg("This page belongs to another logical stream, which this view does not inspect."),
    ),
    (
        "* value truncated at the 512-byte display limit",
        id3("The metadata value is longer than the 512-byte display limit."),
    ),
    (
        "comment truncated at the 512-byte display limit",
        id3("The comment is longer than the 512-byte display limit."),
    ),
    (
        "truncated *",
        damage("The stream ends before this part is complete, so its metadata cannot all be read."),
    ),
    (
        "missing *",
        damage("A required part is absent, so the stream is incomplete."),
    ),
    (
        "invalid *",
        oddity("This part does not follow the format's rules, so its metadata may be incomplete."),
    ),
    (
        "unsupported *",
        oddity("This version or feature is not inspected, so the result may be incomplete."),
    ),
    (
        "duplicate *",
        oddity("The stream has more than one copy of a part that should occur once."),
    ),
    (
        "* exceeds the * inspection limit",
        oddity("This part is larger than the safe inspection limit, so some metadata was skipped."),
    ),
    (
        "* count exceeds the inspection limit",
        oddity("There are more metadata parts than this view inspects."),
    ),
    (
        "* count * exceeds *",
        oddity("There are more metadata parts than the format or this view allows."),
    ),
    (
        "* type 127 is forbidden",
        oddity("This metadata block type is forbidden by the FLAC format."),
    ),
    (
        "first FLAC metadata block must be 34-byte STREAMINFO",
        oddity("FLAC requires a 34-byte STREAMINFO block first."),
    ),
    (
        "reserved *",
        oddity(
            "Reserved bits or values are set; the tag may use a feature this view does not understand.",
        ),
    ),
    (
        "* flags *",
        oddity("These flags are not valid for this format version."),
    ),
    (
        "* continues past end of input",
        damage("A packet continues beyond the available bytes, so the stream was cut short."),
    ),
    (
        "* runs past the end*",
        damage("This part claims bytes beyond the end of the file."),
    ),
    (
        "*",
        oddity(
            "This part is malformed or was not fully inspected, so the metadata may be incomplete.",
        ),
    ),
];

pub(crate) fn describe(_tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    lookup(TABLE, &node.label, problem)
}

/// Every explanation here, for the page's copy of the sentences ([`crate::docs::every`]).
pub(crate) const EVERY: &[Table] = &[TABLE];
