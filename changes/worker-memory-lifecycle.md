### Changed

- Terminate and replace the actual WASM worker on fatal parser errors, instead of clearing its module cache.
- Release idle batch readers at queue boundaries and after bounded work.
- Ensure temporary parser objects are freed on exceptional paths.
