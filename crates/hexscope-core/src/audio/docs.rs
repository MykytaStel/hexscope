//! Plain explanations for audio metadata this parser can read without decoding.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const ID3: &str = "https://id3.org/id3v2.4.0-structure";
const FLAC: &str = "https://www.rfc-editor.org/rfc/rfc9639.html";
const OGG: &str = "https://www.rfc-editor.org/rfc/rfc3533.html";
const VORBIS: &str = "https://www.xiph.org/vorbis/doc/Vorbis_I_spec.html";
const OPUS: &str = "https://www.rfc-editor.org/rfc/rfc7845.html";
const RIFF: &str = "https://learn.microsoft.com/en-us/windows/win32/xaudio2/resource-interchange-file-format--riff-";
const BWF: &str = "https://tech.ebu.ch/docs/tech/tech3285.pdf";
const RF64: &str = "https://tech.ebu.ch/files/live/sites/tech/files/shared/tech/tech3306.pdf";
const BW64: &str =
    "https://www.itu.int/dms_pubrec/itu-r/rec/bs/R-REC-BS.2088-2-202511-I%21%21PDF-E.pdf";

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

const fn riff(text: &'static str) -> Doc {
    Doc::new(text).cite("Microsoft RIFF reference", RIFF)
}

const fn bwf(text: &'static str) -> Doc {
    Doc::new(text).cite("EBU Tech 3285, Broadcast Wave Format", BWF)
}

const fn rf64(text: &'static str) -> Doc {
    Doc::new(text).cite("EBU Tech 3306, RF64", RF64)
}

const fn bw64(text: &'static str) -> Doc {
    Doc::new(text).cite("ITU-R BS.2088-2, BW64", BW64)
}

const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const fn oddity(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Oddity)
}

const TABLE: Table = &[
    (
        "WAV audio",
        riff("A RIFF/WAVE recording. A clean copy can remove fully checked INFO and Broadcast Wave metadata chunks while leaving its format and audio data unchanged; the sound is not decoded or played."),
    ),
    (
        "RF64/WAVE audio",
        rf64("An RF64/WAVE recording with 64-bit sizes read from its mandatory ds64 chunk. Hexscope reads supported chunk sizes and metadata without decoding or playing audio. RF64 remains read-only; unsupported chunks make the read partial."),
    ),
    (
        "RF64 ds64 chunk",
        rf64("The mandatory RF64 ds64 chunk provides the 64-bit form size, data size, sample count and optional table of additional chunk sizes."),
    ),
    (
        "RF64 fmt chunk",
        rf64("The RF64 format chunk describes audio encoding, channel count, sample rate and sample layout."),
    ),
    (
        "RF64 data chunk",
        rf64("The RF64 audio data chunk. Its declared size is resolved from ds64 when the 32-bit size field is 0xFFFFFFFF; audio is not decoded or played."),
    ),
    (
        "RF64 * chunk",
        rf64("A chunk in the RF64/WAVE form. Its label, size and byte range are shown; unsupported chunk contents remain unread."),
    ),
    (
        "BW64/WAVE audio",
        bw64("A BW64/WAVE recording with 64-bit sizes read from its mandatory ds64 chunk. Hexscope reads supported chunk sizes and metadata without decoding or playing audio. BW64 remains read-only; XML and channel metadata chunks are not interpreted yet."),
    ),
    (
        "BW64 ds64 chunk",
        bw64("The mandatory BW64 ds64 chunk provides the 64-bit form and data sizes, reserved compatibility fields and an optional table of additional chunk sizes."),
    ),
    (
        "BW64 fmt chunk",
        bw64("The BW64 format chunk describes audio encoding, channel count, sample rate and sample layout."),
    ),
    (
        "BW64 data chunk",
        bw64("The BW64 audio data chunk. Its declared size is resolved from ds64 when the 32-bit size field is 0xFFFFFFFF; audio is not decoded or played."),
    ),
    (
        "BW64 * chunk",
        bw64("A chunk in the BW64/WAVE form. Its label, size and byte range are shown; unsupported chunk contents remain unread."),
    ),
    (
        "RIFX/WAVE audio",
        riff("A big-endian RIFX/WAVE recording. Hexscope reads the supported chunk structure and metadata fields without decoding or playing audio. RIFX remains read-only."),
    ),
    (
        "RIFX fmt chunk",
        riff("The big-endian RIFX format chunk describes the audio encoding, channel count, sample rate and sample layout."),
    ),
    (
        "RIFX data chunk",
        riff("The RIFX/WAVE audio data chunk. Its bytes are counted but are not decoded or played."),
    ),
    (
        "RIFX LIST/INFO chunk",
        riff("The big-endian RIFX INFO list is read as metadata, but stays read-only; other list contents remain unchecked."),
    ),
    (
        "RIFX bext chunk",
        bwf("Selected Broadcast Wave fields are read from this big-endian RIFX chunk. The format remains read-only."),
    ),
    (
        "RIFX * chunk",
        riff("A chunk in the big-endian RIFX/WAVE form; its label and byte range are shown in the structure map."),
    ),
    (
        "WAVE fmt chunk",
        riff("The WAVE format chunk describes the audio encoding, channel count, sample rate and sample layout."),
    ),
    (
        "WAVE data chunk",
        riff("The WAVE audio data chunk. Its bytes are counted but are not decoded or played."),
    ),
    (
        "WAVE LIST* chunk",
        riff("A RIFF list groups related chunks. INFO text is inspected and removed in a clean copy; other list contents remain unchecked and prevent cleaning."),
    ),
    (
        "WAVE bext chunk",
        bwf("Broadcast Wave metadata can include a description, originator, date, sample reference and coding history. A clean copy removes the whole chunk, including sample timing, loudness values and coding history."),
    ),
    (
        "WAVE * chunk",
        riff("A chunk in the RIFF/WAVE form; its label and byte range are shown in the structure map."),
    ),
    (
        "WAVE INFO *",
        riff("A text item in the WAVE INFO list, such as a title, artist, comment or software name."),
    ),
    (
        "WAVE INFO value",
        riff("The text stored in this WAVE INFO item."),
    ),
    (
        "broadcast description",
        bwf("A description of the broadcast recording, stored as plain text."),
    ),
    (
        "broadcast originator",
        bwf("The name of the person or organization recorded as the originator."),
    ),
    (
        "broadcast originator reference",
        bwf("A reference identifier for the recorded originator; Hexscope does not verify it."),
    ),
    (
        "broadcast origination date",
        bwf("The date recorded for the broadcast recording, in the format stored by the file."),
    ),
    (
        "broadcast origination time",
        bwf("The time recorded for the broadcast recording, in the format stored by the file."),
    ),
    (
        "WAVE audio data (not decoded)",
        riff("These bytes hold encoded audio. This view reports their location and size without decoding or playing them."),
    ),
    (
        "WAVE chunk data not inspected",
        riff("This chunk is part of the RIFF/WAVE form, but its contents are not interpreted here."),
    ),
    (
        "WAVE LIST contents not inspected",
        riff("This list type is not INFO, so its contents are not interpreted here."),
    ),
    (
        "WAVE extensible subformat GUID",
        riff("The extensible format's GUID identifies the specific audio encoding."),
    ),
    (
        "WAVE format extension not inspected",
        riff("This encoding-specific format extension is present, but its contents are not interpreted here."),
    ),
    (
        "WAVE format trailing bytes not inspected",
        riff("These bytes trail the declared format extension and are not interpreted here."),
    ),
    (
        "WAVE INFO value truncated at the 512-byte display limit",
        riff("This INFO value is longer than the display limit; only its first 512 bytes are shown."),
    ),
    (
        "nonzero bytes after WAVE INFO text",
        riff("Nonzero bytes follow the text's terminator, so the rest of this field is not interpreted."),
    ),
    (
        "nonzero bytes after Broadcast Wave fixed text",
        bwf("Nonzero bytes follow the fixed text's terminator, so the rest of this field is not interpreted."),
    ),
    (
        "Broadcast Wave coding history truncated at the 512-byte display limit",
        bwf("This coding history is longer than the display limit; only its first 512 bytes are shown."),
    ),
    (
        "broadcast *",
        bwf("A field in the Broadcast Wave extension; its stored value is shown without validating the recording's provenance."),
    ),
    (
        "broadcast coding history",
        bwf("The stored text describes processing steps recorded by the author of the Broadcast Wave file; Hexscope does not verify that history."),
    ),
    (
        "broadcast UMID",
        bwf("The UMID is a stored identifier for audiovisual material; Hexscope displays it as bytes and does not check the identity it refers to."),
    ),
    (
        "broadcast loudness stored value",
        bwf("A signed loudness measurement stored in the Broadcast Wave extension. Its numeric value is shown without interpreting the standard's units."),
    ),
    (
        "broadcast reserved bytes",
        bwf("Bytes reserved by the Broadcast Wave extension for future use; nonzero values are left unchecked."),
    ),
    (
        "broadcast coding history exceeds the inspection limit",
        bwf("This coding history is larger than the display limit, so it remains unchecked."),
    ),
    (
        "Broadcast Wave coding history is not UTF-8",
        bwf("The coding history does not use readable UTF-8 text, so its contents remain unchecked."),
    ),
    (
        "Broadcast Wave fixed text is not ASCII",
        bwf("A fixed-width Broadcast Wave text field contains bytes outside ASCII, so that value remains unchecked."),
    ),
    (
        "unsupported Broadcast Wave extension version",
        bwf("This Broadcast Wave extension version is newer than the versions whose fields this view understands."),
    ),
    (
        "nonzero Broadcast Wave reserved bytes",
        bwf("Reserved bytes are nonzero, so they may carry an extension this view does not understand."),
    ),
    (
        "duplicate WAVE format chunk",
        riff("The form has more than one audio format chunk, so it is unclear which description applies."),
    ),
    (
        "audio duration in whole seconds (estimated from chunk sizes)",
        riff("An estimate from the declared average byte rate and audio-data chunk sizes; compressed audio is not decoded to measure its duration."),
    ),
    (
        "bytes after the RIFF form",
        riff("These bytes are outside the length declared by the RIFF header and were not interpreted as WAVE chunks.").concern(Concern::Hidden),
    ),
    (
        "WAVE chunk count exceeds the inspection limit",
        riff("There are more WAVE chunks than this view inspects, so later chunks remain unchecked."),
    ),
    (
        "WAVE INFO item count exceeds the inspection limit",
        riff("There are more INFO items than this view inspects, so later metadata remains unchecked."),
    ),
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
        "leading ID3v2 tag count exceeds the 16-tag inspection limit",
        id3(
            "This file has more leading ID3v2 tags than this view can inspect; later tags and the audio boundary are left unchecked.",
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
        flac("The contents of the picture block; the clean copy removes the entire block."),
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
            "Ogg holds audio and other streams; this view reads supported audio tags and checks page checksums, and clean copies replace checked Vorbis or Opus comments while keeping audio packets byte for byte.",
        ),
    ),
    (
        "Ogg page *",
        ogg(
            "An Ogg page carries part of a logical stream, including packet boundaries and timing.",
        ),
    ),
    (
        "invalid Ogg page checksum",
        ogg(
            "The Ogg page checksum does not match its contents; metadata from this page is not trusted.",
        ),
    ),
    (
        "Ogg identification packet",
        ogg("The first packet identifies which codec's compressed audio follows."),
    ),
    (
        "Ogg identification packet is not complete and alone on its start page",
        ogg("A supported Ogg identification packet must be complete and alone on its first page."),
    ),
    (
        "Ogg comment packet does not begin on the second page",
        ogg("A supported Ogg comment packet must begin on the page after the start page."),
    ),
    (
        "Ogg audio begins on the final header page",
        ogg("The final Ogg header must finish its page before audio packets begin."),
    ),
    (
        "Ogg header page has an invalid granule position",
        ogg("An Ogg header page must have a zero granule position."),
    ),
    (
        "Vorbis comment packet",
        vorbis("The Vorbis comment packet carries text tags such as title, artist and encoder."),
    ),
    (
        "Vorbis setup packet (not decoded)",
        vorbis(
            "The Vorbis setup header lists the codebooks and settings for later packets. This view checks its signature, but does not decode it.",
        ),
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
        "Ogg page count exceeds the inspection limit",
        ogg("There are more Ogg pages than this view inspects."),
    ),
    (
        "multiplexed Ogg streams are not inspected",
        ogg("This page belongs to another logical stream, which this view does not inspect."),
    ),
    (
        "Ogg logical stream does not begin with a beginning-of-stream page",
        ogg("Each chained Ogg stream must begin with its own start page."),
    ),
    (
        "Ogg page repeats the beginning-of-stream flag",
        ogg("Only the first page of an Ogg logical stream can be its start page."),
    ),
    (
        "Ogg stream has no end-of-stream page",
        ogg("The Ogg logical stream has no final page, so its end was not confirmed."),
    ),
    (
        "Ogg stream has no complete comment packet",
        ogg("The supported audio stream ended before its complete tag packet was read."),
    ),
    (
        "Ogg stream ends with an incomplete packet",
        ogg("The final Ogg page stops in the middle of a packet."),
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
