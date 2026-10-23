# Fuzz seeds

`parse_audio/` contains four tiny synthetic metadata-only inputs for ID3v2,
FLAC, Ogg Vorbis and Ogg Opus. They contain no encoded audio or personal
metadata. CI copies these into cargo-fuzz's ignored, writable corpus before
running the audio parser smoke test.
