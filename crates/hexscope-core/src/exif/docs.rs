//! What each part of an EXIF block is. EXIF is cited by section name: the
//! CIPA specification numbers its sections, but the named ones are what a
//! reader searches for.

use crate::docs::{Concern, Doc, Table, lookup};

const CIPA: &str = "https://www.cipa.jp/std/documents/e/DC-X008-Translation-2019-E.pdf";

const fn tiff(text: &'static str) -> Doc {
    Doc::new(text).cite("EXIF 2.32, TIFF attributes", CIPA)
}
const fn exif(text: &'static str) -> Doc {
    Doc::new(text).cite("EXIF 2.32, Exif IFD attributes", CIPA)
}
const fn gps(text: &'static str) -> Doc {
    Doc::new(text).cite("EXIF 2.32, GPS attributes", CIPA)
}
const fn structure(text: &'static str) -> Doc {
    Doc::new(text).cite("EXIF 2.32, file structure", CIPA)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}
const fn oddity(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Oddity)
}

/// Every tag the parser names, by that name. A test checks the parser's tag
/// table against this one, so a tag cannot be named without being explained.
pub(crate) const TAGS: Table = &[
    // IFD0 and IFD1: the image and the camera, in TIFF terms.
    ("ImageWidth", tiff("How many pixels wide the image is.")),
    ("ImageLength", tiff("How many pixels tall the image is.")),
    (
        "BitsPerSample",
        tiff("How many bits each colour component takes."),
    ),
    (
        "Compression",
        tiff("How the image, usually the thumbnail, is compressed; 6 means JPEG."),
    ),
    (
        "PhotometricInterpretation",
        tiff("How pixel values map to colours: RGB, YCbCr and so on."),
    ),
    (
        "ImageDescription",
        tiff("A title or description of the picture, as the camera or an editor wrote it."),
    ),
    ("Make", tiff("Who made the camera or phone.")),
    (
        "Model",
        tiff("Which camera or phone model took the picture."),
    ),
    (
        "StripOffsets",
        tiff("Where the image's data starts, for images stored in strips."),
    ),
    (
        "Orientation",
        tiff("Which way up the camera was held, so viewers can turn the picture the right way."),
    ),
    (
        "SamplesPerPixel",
        tiff("How many colour components make one pixel."),
    ),
    ("XResolution", tiff("Pixels per unit across, for printing.")),
    ("YResolution", tiff("Pixels per unit down, for printing.")),
    (
        "ResolutionUnit",
        tiff("The unit for the two resolutions: inches or centimetres."),
    ),
    (
        "Software",
        tiff("The program or firmware that last wrote the file, which can show it was edited."),
    ),
    ("DateTime", tiff("When the file was last changed.")),
    (
        "Artist",
        tiff("The photographer's name, if the camera was set up with it."),
    ),
    ("WhitePoint", tiff("The colour of white the image assumes.")),
    (
        "PrimaryChromaticities",
        tiff("The exact red, green and blue the image's colours are made of."),
    ),
    (
        "JPEGInterchangeFormat",
        tiff("Where the embedded thumbnail starts."),
    ),
    (
        "JPEGInterchangeFormatLength",
        tiff("How long the embedded thumbnail is."),
    ),
    (
        "YCbCrCoefficients",
        tiff("How to turn the stored brightness and colour-difference values into RGB."),
    ),
    (
        "YCbCrPositioning",
        tiff("Where colour samples sit relative to brightness samples."),
    ),
    (
        "ReferenceBlackWhite",
        tiff("Which stored values mean black and which mean white."),
    ),
    ("Copyright", tiff("The copyright notice.")),
    (
        "ExifIFDPointer",
        tiff("Where the camera-settings directory, the Exif IFD, starts."),
    ),
    (
        "GPSInfoIFDPointer",
        tiff("Where the location directory, the GPS IFD, starts: this picture records a place."),
    ),
    (
        "PrintImageMatching",
        tiff("Print settings from Epson's Print Image Matching system."),
    ),
    (
        "Padding",
        tiff(
            "Empty space an editor reserves so it can add metadata later without rewriting the file.",
        ),
    ),
    // Exif IFD: how the picture was taken.
    (
        "ExposureTime",
        exif("How long the shutter was open, in seconds."),
    ),
    (
        "FNumber",
        exif("The aperture, as an f-number: smaller lets in more light."),
    ),
    (
        "ExposureProgram",
        exif("The shooting mode: manual, automatic, aperture priority and so on."),
    ),
    (
        "ISOSpeedRatings",
        exif("The sensor's sensitivity to light: higher for darker scenes."),
    ),
    (
        "SensitivityType",
        exif("Which of the ISO standards the sensitivity value follows."),
    ),
    (
        "ExifVersion",
        exif("Which version of the EXIF standard the block follows."),
    ),
    ("DateTimeOriginal", exif("When the picture was taken.")),
    (
        "DateTimeDigitized",
        exif("When the picture was stored digitally, usually the same moment."),
    ),
    (
        "OffsetTime",
        exif("The time zone of the last-changed time."),
    ),
    (
        "OffsetTimeOriginal",
        exif("The time zone the picture was taken in, which narrows down where."),
    ),
    (
        "OffsetTimeDigitized",
        exif("The time zone of the digitized time."),
    ),
    (
        "ComponentsConfiguration",
        exif("The order of the colour components in each pixel."),
    ),
    (
        "CompressedBitsPerPixel",
        exif("How strongly the image was compressed, in bits per pixel."),
    ),
    (
        "ShutterSpeedValue",
        exif("The shutter speed, on the logarithmic APEX scale."),
    ),
    (
        "ApertureValue",
        exif("The aperture, on the logarithmic APEX scale."),
    ),
    (
        "BrightnessValue",
        exif("How bright the scene was, on the APEX scale."),
    ),
    (
        "ExposureBiasValue",
        exif("How much brighter or darker than metered the photographer asked for."),
    ),
    ("MaxApertureValue", exif("The lens's widest aperture.")),
    (
        "SubjectDistance",
        exif("How far away the subject was, in metres."),
    ),
    (
        "MeteringMode",
        exif("How the camera measured the light: centre-weighted, spot, pattern."),
    ),
    (
        "LightSource",
        exif("The kind of light: daylight, fluorescent, tungsten and so on."),
    ),
    ("Flash", exif("Whether the flash fired, and how.")),
    (
        "FocalLength",
        exif("The lens's focal length, in millimetres."),
    ),
    (
        "SubjectArea",
        exif("Where in the frame the main subject is."),
    ),
    (
        "MakerNote",
        exif(
            "The manufacturer's own data, in its own undocumented format; it often holds more serial numbers.",
        ),
    ),
    ("UserComment", exif("A free comment, in any character set.")),
    (
        "SubSecTime",
        exif("Fractions of a second for the last-changed time."),
    ),
    (
        "SubSecTimeOriginal",
        exif("Fractions of a second for when the picture was taken."),
    ),
    (
        "SubSecTimeDigitized",
        exif("Fractions of a second for when the picture was stored."),
    ),
    (
        "FlashpixVersion",
        exif("Which version of the FlashPix format the data follows."),
    ),
    (
        "ColorSpace",
        exif("The colour space: 1 is sRGB, the usual one."),
    ),
    (
        "PixelXDimension",
        exif("How many pixels wide the image really is."),
    ),
    (
        "PixelYDimension",
        exif("How many pixels tall the image really is."),
    ),
    (
        "RelatedSoundFile",
        exif("The name of a sound recording that goes with the picture."),
    ),
    (
        "InteroperabilityIFDPointer",
        exif("Where the interoperability directory starts."),
    ),
    (
        "FocalPlaneXResolution",
        exif("Sensor pixels per unit across."),
    ),
    (
        "FocalPlaneYResolution",
        exif("Sensor pixels per unit down."),
    ),
    (
        "FocalPlaneResolutionUnit",
        exif("The unit for the two sensor resolutions."),
    ),
    (
        "SensingMethod",
        exif("What kind of image sensor the camera has."),
    ),
    (
        "FileSource",
        exif("Where the image came from: a digital camera, a scanner and so on."),
    ),
    (
        "SceneType",
        exif("Whether the image was photographed directly."),
    ),
    (
        "CustomRendered",
        exif("Whether the camera applied special processing, such as a portrait effect."),
    ),
    (
        "ExposureMode",
        exif("Whether exposure was automatic, manual or bracketed."),
    ),
    (
        "WhiteBalance",
        exif("Whether white balance was automatic or manual."),
    ),
    (
        "DigitalZoomRatio",
        exif("How much digital zoom was used; 0 or 1 means none."),
    ),
    (
        "FocalLengthIn35mmFilm",
        exif("The focal length a 35 mm film camera would need for the same view."),
    ),
    (
        "SceneCaptureType",
        exif("The scene mode: landscape, portrait, night and so on."),
    ),
    (
        "CameraOwnerName",
        exif("The camera owner's name, as they set it in the camera: it names a person."),
    ),
    (
        "BodySerialNumber",
        exif("The camera body's serial number: it ties every picture to one particular camera."),
    ),
    (
        "LensSpecification",
        exif("The lens's focal-length and aperture range."),
    ),
    ("LensMake", exif("Who made the lens.")),
    ("LensModel", exif("Which lens was used.")),
    (
        "LensSerialNumber",
        exif("The lens's serial number: it ties the picture to one particular lens."),
    ),
    (
        "CompositeImage",
        exif("Whether the image was combined from several shots."),
    ),
    // GPS IFD: where the picture was taken.
    (
        "GPSVersionID",
        gps("Which version of the GPS tags follows."),
    ),
    (
        "GPSLatitudeRef",
        gps("Whether the latitude is north or south of the equator."),
    ),
    (
        "GPSLatitude",
        gps("How far north or south the picture was taken: half of its exact location."),
    ),
    (
        "GPSLongitudeRef",
        gps("Whether the longitude is east or west of Greenwich."),
    ),
    (
        "GPSLongitude",
        gps("How far east or west the picture was taken: the other half of its exact location."),
    ),
    (
        "GPSAltitudeRef",
        gps("Whether the altitude is above or below sea level."),
    ),
    (
        "GPSAltitude",
        gps("How high above sea level the picture was taken."),
    ),
    ("GPSTimeStamp", gps("The time of the GPS fix, in UTC.")),
    (
        "GPSSatellites",
        gps("Which satellites were used for the fix."),
    ),
    (
        "GPSStatus",
        gps("Whether the receiver was measuring when the picture was taken."),
    ),
    (
        "GPSMeasureMode",
        gps("Whether the fix was two- or three-dimensional."),
    ),
    ("GPSDOP", gps("How precise the fix was: lower is better.")),
    (
        "GPSSpeedRef",
        gps("The unit of the speed: km/h, mph or knots."),
    ),
    ("GPSSpeed", gps("How fast the camera was moving.")),
    (
        "GPSTrackRef",
        gps("Whether the direction of movement is true or magnetic."),
    ),
    ("GPSTrack", gps("Which way the camera was moving.")),
    (
        "GPSImgDirectionRef",
        gps("Whether the camera's direction is true or magnetic."),
    ),
    ("GPSImgDirection", gps("Which way the camera was pointing.")),
    (
        "GPSMapDatum",
        gps("The map system the coordinates use, such as WGS-84."),
    ),
    (
        "GPSDestLatitudeRef",
        gps("Whether the destination's latitude is north or south."),
    ),
    (
        "GPSDestLatitude",
        gps("The latitude of a destination point."),
    ),
    (
        "GPSDestLongitudeRef",
        gps("Whether the destination's longitude is east or west."),
    ),
    (
        "GPSDestLongitude",
        gps("The longitude of a destination point."),
    ),
    (
        "GPSDestBearingRef",
        gps("Whether the bearing to the destination is true or magnetic."),
    ),
    ("GPSDestBearing", gps("The bearing to the destination.")),
    (
        "GPSDestDistanceRef",
        gps("The unit of the distance to the destination."),
    ),
    ("GPSDestDistance", gps("The distance to the destination.")),
    (
        "GPSProcessingMethod",
        gps("How the location was found: GPS, cell towers, Wi-Fi."),
    ),
    ("GPSAreaInformation", gps("The name of the place.")),
    ("GPSDateStamp", gps("The date of the GPS fix, in UTC.")),
    (
        "GPSDifferential",
        gps("Whether the fix was corrected by a ground station."),
    ),
    (
        "GPSHPositioningError",
        gps("How far off the location may be, in metres."),
    ),
];

const STRUCTURE: Table = &[
    (
        "TIFF header",
        structure(
            "The start of the EXIF data, laid out like a TIFF file: byte order, a check number, and where the first directory is.",
        ),
    ),
    (
        "byteOrder",
        structure(
            "Whether numbers are stored little-end first (II, Intel) or big-end first (MM, Motorola).",
        ),
    ),
    (
        "magic",
        structure("The number 42, which confirms the byte order was read right."),
    ),
    (
        "ifd0Offset",
        structure("Where the first directory of tags starts."),
    ),
    (
        "IFD0",
        structure(
            "The main image's directory: a list of tags, each a number, a type and a value or where to find it.",
        ),
    ),
    (
        "IFD1 (thumbnail)",
        structure("The thumbnail's directory: a small preview of the photo, stored inside it."),
    ),
    (
        "Exif IFD",
        structure("The camera-settings directory: exposure, lens, dates."),
    ),
    (
        "GPS IFD",
        structure("The location directory: where the picture was taken."),
    ),
    (
        "Interop IFD",
        structure("The interoperability directory: which rules the file follows."),
    ),
    (
        "entryCount",
        structure("How many tags this directory holds."),
    ),
    (
        "nextIFDOffset",
        structure("Where the next directory starts, or 0 when this is the last."),
    ),
    (
        "thumbnail",
        structure(
            "A small JPEG preview of the photo. Editors do not always update it, so it can still show what was cropped out.",
        ),
    ),
    (
        "tag 0x*",
        structure("A tag this tool does not name, often one a camera maker added."),
    ),
    // Problems.
    (
        "not a TIFF header*",
        damage("The EXIF data does not start with II or MM, so none of it can be read."),
    ),
    (
        "TIFF header truncated",
        damage("The EXIF data stops before its header is complete."),
    ),
    (
        "TIFF magic is *",
        oddity("The check number is not 42; the rest is read anyway."),
    ),
    (
        "* offset * is past the end of the EXIF data",
        damage("A directory is said to start past the end of the EXIF data, so it cannot be read."),
    ),
    (
        "claims * entries, only * fit*",
        damage(
            "The directory claims more tags than the data holds, so only those that fit are read.",
        ),
    ),
    (
        "value at offset * runs past*",
        damage("A tag's value is said to be stored past the end of the EXIF data."),
    ),
    (
        "unknown value type *",
        oddity("A tag has a value type the standard does not define, so its value is not shown."),
    ),
    (
        "thumbnail runs past*",
        damage("The thumbnail is said to be longer than the EXIF data that holds it."),
    ),
    (
        "stopped after * IFDs",
        oddity("There are far more directories than any camera writes, so reading stopped."),
    ),
    (
        "* points back to an IFD already read",
        oddity("A directory points back to one already read; following it would loop forever."),
    ),
];

#[cfg(test)]
pub(crate) const ALL: Table = STRUCTURE;

pub(crate) fn describe(label: &str, problem: bool) -> Option<Doc> {
    // A value stored away from its entry means what its tag means.
    let tag = label.strip_suffix(" value").unwrap_or(label);
    lookup(TAGS, tag, problem).or_else(|| lookup(STRUCTURE, label, problem))
}
