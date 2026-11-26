### Changed

- Flatten the per-node spatial index into two typed arrays with no per-leaf index allocation.
- Reuse child offsets and temporary sibling storage; preserve exact byte ownership and overlap behavior.
