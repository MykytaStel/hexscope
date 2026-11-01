### Fixed

- **Reject damaged Ogg metadata pages.** Page checksums are validated before their codec headers and comments are trusted. Ogg stays read-only, and audio pages after the metadata headers remain unchecked.
