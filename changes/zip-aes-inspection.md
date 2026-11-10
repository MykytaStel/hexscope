### Changed

- **See how AES-encrypted ZIP entries are configured.** The archive view reads
  WinZip AES version, key strength and the compression method from the `0x9901`
  extra field, and compares those recognized fields in the local header and
  central directory. Additional extension bytes remain uninterpreted, and
  encrypted file data is not decrypted or opened.
