### Changed

- **Email authentication results stay claims from the message.** Hexscope now shows SPF, DKIM and DMARC statuses recorded in `Authentication-Results` as unverified claims, including when the header is present but contains no recognized results.
- **Shared files expire after 24 hours.** The PWA keeps a shared group together and accepts up to 50 files and 400 MiB per group.

### Fixed

- **Film Scan samples large rasters without repeating a fixed pixel stride.** Sampling is bounded and deterministic, including on 32-bit WebAssembly.
