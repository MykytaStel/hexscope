### Fixed

- Keep WebP chunk payloads, padding and extended-header fields within their declared RIFF boundaries; report truncated framing and unread chunks as errors instead of silently accepting them.
