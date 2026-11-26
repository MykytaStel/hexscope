### Changed

- Limit the sum of additional decoded inputs retained by nested browser navigation to 256 MiB, excluding the root; preserve the existing individual limit and nesting depth.
- Drop reloadable parent byte pages after a successful nested open while retaining parser evidence and selection history.
