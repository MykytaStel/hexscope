### Changed

- **PDF and report tools are imported on demand.** Opening a non-PDF does not parse or run the PDF redaction UI, and report/export modules are imported only after the person asks for them. The offline service worker still caches the full app on the first visit so these tools remain available without a connection.
