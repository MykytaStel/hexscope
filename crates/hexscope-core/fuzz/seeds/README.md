# Fuzz seeds

`parse_audio/` contains four tiny synthetic metadata-only inputs for ID3v2,
FLAC, Ogg Vorbis and Ogg Opus. They contain no encoded audio or personal
metadata. CI copies these into cargo-fuzz's ignored, writable corpus before
running the audio parser smoke test.

`clean_roundtrip` starts from the format corpus, these audio seeds, the
synthetic iPhone HDR fixture and `clean_roundtrip/` — a tiny MP3 and FLAC
with a title and a place that hexscope can clean, so mutations reach the
cleaners and not only their refusals: every clean copy must read back as the same
format, clean again and compare against its source without panicking.
