### Changed

- Borrow validated stored ZIP entries for embedded Office and OpenDocument photo inspection instead of allocating an additional full entry buffer.
- Preserve the owned extraction API and all CRC, encryption, declared-size and range checks; compressed entries still decode into owned storage.
