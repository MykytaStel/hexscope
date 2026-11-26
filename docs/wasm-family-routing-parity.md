# WASM routing parity

The core audio parser already recognizes RIFF, RIFX, RF64 and BW64 with WAVE at offset 8. Browser routing previously selected audio only for RIFF WAVE; the other containers fell through to a build without audio. Match all four, without broadening non-WAVE RIFF recognition. No new parser or codec is needed.

The core also recognizes QuickTime starting with moov, mdat, wide, free, skip or pnot when the initial size is 1 or between 8 and the actual file length. Route those after audio, ahead of PDF/document detection, so an incidental embedded signature does not choose a different feature family. ftyp handling is unchanged. These are targeted parity fixes, not proof that every browser heuristic matches every native parser. Tests include malformed boundaries and competing signatures. No feature or WASM size budget changes.
