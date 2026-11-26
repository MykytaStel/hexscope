### Changed

- Release deferred media caches when files are replaced, removed from nested navigation or abandoned by stale parse results.
- Prevent pending Blob reads from repopulating retired caches without bypassing concurrent-read limits.
- Keep retained parent documents and parser evidence intact.
