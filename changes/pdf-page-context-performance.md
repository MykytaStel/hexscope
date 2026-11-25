### Changed

- Reuse the same indexed PDF structure and decryption path for page text, page pictures, QR picture/shape discovery and JPEG redaction previews without repeating the full-document findings walk.
- Keep full metadata, historical text and blackout checks on the primary parser, attachment lookup and cleaner paths.
