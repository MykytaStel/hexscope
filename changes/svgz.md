### Added

- **Read and clean bounded `.svgz` drawings** in Core and Web. Gzip integrity is validated and the cleaned copy is read back; compressed input and expanded XML are each limited to 64 MiB, and concatenated gzip members are rejected.
