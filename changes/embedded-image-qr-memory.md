### Changed

- **Large pictures inside ZIP archives and email attachments can be downsampled before QR scanning.** PNG, JPEG, GIF, WebP and BMP dimensions are read from their headers; an unsupported or malformed header keeps the existing browser decode fallback.
