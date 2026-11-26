### Changed

- Serialize QR/image-analysis jobs in the reader, including film export readback.
- Allow queued cancellation to release work immediately and yield between expensive scan phases.
- Check scan cancellation after an external compressed entry read, before inflating it.
