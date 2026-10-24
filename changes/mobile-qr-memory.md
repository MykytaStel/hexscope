### Fixed

- **Large-photo QR scans use less temporary memory where the browser supports it.** Images with known dimensions are decoded at the scanner's size instead of first allocating a full-resolution bitmap; the browser keeps the photo's proportions, and the bitmap is released on every exit path.

### Changed

- **Large batches do less repeated work while they update.** File counts, filters and visible rows are now prepared together, so large folders cost less CPU on phones as files finish reading.
