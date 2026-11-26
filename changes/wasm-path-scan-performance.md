### Changed

- Scan WASM data/debug paths once in forward order rather than rescanning for absent home-directory prefixes after each candidate.
- Reject oversized usernames before allocating and bound path-preview scanning to the displayed 60 bytes.
- Preserve existing path findings, ordering, placeholder exclusions and preview text.
