//! What each part of an MP4 or QuickTime movie is. The box structure is
//! cited to ISO base media (ISO/IEC 14496-12, free) and Apple's QuickTime
//! File Format; the `loci` box to 3GPP TS 26.244.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const BMFF: &str = "https://standards.iso.org/ittf/PubliclyAvailableStandards/";
const THREE_GPP: &str = "https://www.3gpp.org/ftp/Specs/archive/26_series/26.244/";

const fn bmff(text: &'static str) -> Doc {
    Doc::new(text).cite("ISO base media, ISO/IEC 14496-12", BMFF)
}
const fn qt(text: &'static str, url: &'static str) -> Doc {
    Doc::new(text).cite("QuickTime File Format", url)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const MOVIE: &str = "https://developer.apple.com/documentation/quicktime-file-format/movie_atom";
const MVHD: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/movie_header_atom";
const TRAK: &str = "https://developer.apple.com/documentation/quicktime-file-format/track_atom";
const TKHD: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/track_header_atom";
const MDIA: &str = "https://developer.apple.com/documentation/quicktime-file-format/media_atom";
const MDHD: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/media_header_atom";
const HDLR: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/handler_reference_atom";
const STBL: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/sample_table_atom";
const EDTS: &str = "https://developer.apple.com/documentation/quicktime-file-format/edit_atom";
const UDTA: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/user_data_atoms";
const META: &str = "https://developer.apple.com/documentation/quicktime-file-format/metadata_atom";
const KEYS: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/metadata_item_keys_atom";
const ILST: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/metadata_item_list_atom";
const DATA: &str = "https://developer.apple.com/documentation/quicktime-file-format/data_atom";
const FTYP: &str =
    "https://developer.apple.com/documentation/quicktime-file-format/file_type_compatibility_atom";

const TABLE: Table = &[
    (
        "movie",
        bmff("A movie: a tree of boxes, with the picture and sound in one and everything about them in another."),
    ),
    // The top level.
    ("ftyp", qt("The file type: which kinds of movie the file follows, and so which players can open it.", FTYP)),
    ("mdat", bmff("The media data: the compressed picture and sound, in the order they play.")),
    ("moov", qt("The movie box: tracks, timing, and where each piece of picture and sound sits in the media data.", MOVIE)),
    ("free", bmff("Unused space, which players skip. Writers leave it to grow boxes later without moving the rest.")),
    ("skip", bmff("Unused space, which players skip.")),
    ("wide", qt("Room for a larger size header, should the box after it grow past 4 GB.", MOVIE)),
    // Tracks.
    ("mvhd", qt("The movie header: when the movie was made and changed, and how long it is.", MVHD)),
    ("trak", qt("One track: the picture, the sound, or something else that plays in time.", TRAK)),
    ("tkhd", qt("The track header: its times, its duration, and its size on screen.", TKHD)),
    ("tapt", qt("The track's aperture: how its picture is cropped and scaled for display.", TRAK)),
    ("edts", qt("Edits: which part of the track plays, and when.", EDTS)),
    ("elst", qt("The edit list itself.", EDTS)),
    ("mdia", qt("The track's media: its timing and how to read its samples.", MDIA)),
    ("mdhd", qt("The media header: its times, time scale and language.", MDHD)),
    ("hdlr", qt("The handler: what kind of data this is — video, sound or metadata.", HDLR)),
    ("minf", qt("Media information: how to decode and show the track.", MDIA)),
    ("vmhd", qt("Video media header: how to draw the picture over what is behind it.", MDIA)),
    ("smhd", qt("Sound media header: the sound's balance.", MDIA)),
    ("dinf", qt("Data information: where the media's data lives, which is this same file.", MDIA)),
    ("dref", qt("The list of places data can come from.", MDIA)),
    ("stbl", qt("The sample table: every frame's size, time and place in the media data.", STBL)),
    ("stsd", qt("Sample descriptions: the codec and its settings.", STBL)),
    ("stts", qt("How long each sample plays.", STBL)),
    ("ctts", qt("How far each frame's display time is from its decoding time.", STBL)),
    ("cslg", qt("The range of those offsets, for players that plan ahead.", STBL)),
    ("stss", qt("Which frames are key frames, where playback can start.", STBL)),
    ("sdtp", qt("Which frames depend on others.", STBL)),
    ("stsc", qt("How samples are grouped into chunks.", STBL)),
    ("stsz", qt("Each sample's size.", STBL)),
    ("stco", qt("Where each chunk starts in the file, as a 32-bit offset.", STBL)),
    ("co64", qt("Where each chunk starts in the file, as a 64-bit offset.", STBL)),
    // Metadata.
    ("udta", qt("User data: text about the movie, such as where and with what it was recorded.", UDTA)),
    ("meta", qt("Metadata: named items about the movie, such as an iPhone's location and model.", META)),
    ("keys", qt("The names of the metadata items that follow, in order.", KEYS)),
    ("key *", qt("One item's name, such as com.apple.quicktime.location.ISO6709.", KEYS)),
    ("ilst", qt("The metadata items themselves, each named by a key.", ILST)),
    ("data", qt("An item's value, and what kind of value it is.", DATA)),
    (
        "com.apple.quicktime.location.ISO6709",
        qt("Where the video was recorded, as latitude, longitude and altitude.", KEYS),
    ),
    ("com.apple.quicktime.location.*", qt("How precise that location is, or what it is called.", KEYS)),
    ("com.apple.quicktime.make", qt("Who made the camera or phone.", KEYS)),
    ("com.apple.quicktime.model", qt("The camera or phone model.", KEYS)),
    ("com.apple.quicktime.software", qt("The software version that recorded it.", KEYS)),
    ("com.apple.quicktime.creationdate", qt("When the video was recorded, with the time zone.", KEYS)),
    ("com.apple.quicktime.*", qt("A metadata item Apple's devices write.", KEYS)),
    ("*.*", qt("A named metadata item.", KEYS)),
    ("©xyz", qt("Where the video was recorded, as latitude and longitude. Android phones write it.", UDTA)),
    ("©mak", qt("Who made the camera or phone.", UDTA)),
    ("©mod", qt("The camera or phone model.", UDTA)),
    ("©swr", qt("The software that recorded it.", UDTA)),
    ("©too", qt("The software that encoded it.", UDTA)),
    ("©day", qt("When it was recorded.", UDTA)),
    ("©*", qt("A text about the movie.", UDTA)),
    (
        "loci",
        Doc::new("Where it was recorded, as 3GPP phones write it: a place name, latitude, longitude and altitude.")
            .cite("3GPP TS 26.244", THREE_GPP),
    ),
    ("uuid", bmff("A box of a kind named by a UUID; Adobe keeps XMP metadata in one.")),
    // Fields.
    ("size", bmff("How many bytes this box takes, header included.")),
    ("type", bmff("Four letters that say what this box is.")),
    ("largeSize", bmff("The box's size, when it is too large for four bytes.")),
    ("version", bmff("Which version of this box's layout follows.")),
    ("flags", bmff("Switches that change what this box holds.")),
    ("majorBrand", qt("The main kind of movie: qt for QuickTime, isom or mp42 for MP4.", FTYP)),
    ("minorVersion", qt("A version number for the main kind.", FTYP)),
    ("compatibleBrands", qt("Every kind of movie the file also follows.", FTYP)),
    ("creationTime", qt("When it was created, in seconds since 1904.", MVHD)),
    ("modificationTime", qt("When it was last changed, in seconds since 1904.", MVHD)),
    ("timescale", qt("How many time units make a second.", MVHD)),
    ("duration", qt("How long it lasts, in time units.", MVHD)),
    ("trackId", qt("The track's number.", TKHD)),
    ("reserved", bmff("Space reserved by the standard; always zero.")),
    ("layout", qt("The track's layer, alternate group and volume.", TKHD)),
    ("matrix", qt("How the picture is transformed on screen, including rotation.", TKHD)),
    ("width", qt("The picture's width on screen.", TKHD)),
    ("height", qt("The picture's height on screen.", TKHD)),
    ("language", qt("The language of the text or sound.", MDHD)),
    ("quality", qt("The playback quality; always zero now.", MDHD)),
    ("preDefined", qt("Always zero in a movie file.", HDLR)),
    ("handlerType", qt("The kind of data: vide, soun, or mdta for metadata.", HDLR)),
    ("name", qt("A name for the handler, or for the place.", HDLR)),
    ("entryCount", bmff("How many entries follow.")),
    ("typeIndicator", qt("What kind of value follows: 1 means UTF-8 text.", DATA)),
    ("locale", qt("The country and language the value is for; 0 means any.", DATA)),
    ("value", qt("The value itself.", DATA)),
    ("textLength", qt("How many bytes of text follow.", UDTA)),
    ("text", qt("The text.", UDTA)),
    ("role", Doc::new("What the place is: 0 where it was shot, 1 where it is really shown, 2 other.").cite("3GPP TS 26.244", THREE_GPP)),
    ("longitude", Doc::new("East or west, in fixed point with 16 bits after the point.").cite("3GPP TS 26.244", THREE_GPP)),
    ("latitude", Doc::new("North or south, in fixed point with 16 bits after the point.").cite("3GPP TS 26.244", THREE_GPP)),
    ("altitude", Doc::new("Height in metres, in fixed point with 16 bits after the point.").cite("3GPP TS 26.244", THREE_GPP)),
    ("astronomicalBody", Doc::new("Which world the place is on; Earth, almost always.").cite("3GPP TS 26.244", THREE_GPP)),
    ("additionalNotes", Doc::new("Any notes about the place.").cite("3GPP TS 26.244", THREE_GPP)),
    ("userType", bmff("The UUID naming this box's kind.")),
    ("body", bmff("The rest of the box, which this tool does not break down.")),
    // Problems.
    ("a box header is cut short", damage("The file stops in the middle of a box's header: it was cut short.")),
    ("a box's 64-bit size is cut short", damage("The file stops in the middle of a box's size.")),
    ("* box size * is smaller than its header", damage("A box claims to be smaller than its own header, so these bytes are damaged.")),
    ("* box runs past the end of its container", damage("A box claims more bytes than there are: the file was cut short or is damaged.")),
    ("* box is shorter than its fields", damage("A box is too short to hold what its type says it holds.")),
];

pub(crate) fn describe(tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    lookup(TABLE, &node.label, problem).or_else(|| {
        // Any other box: readers skip what they do not know.
        (!problem
            && node
                .children
                .iter()
                .any(|&c| tree.try_get(c).is_some_and(|n| n.label == "type")))
        .then_some(bmff(
            "A box this tool does not decode; players skip boxes they do not know.",
        ))
    })
}

#[cfg(test)]
pub(crate) const ALL: Table = TABLE;
