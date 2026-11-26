### Changed

- Track cancelled parse and image scan requests until the reader acknowledges their completion; do not confuse cancelled promises with idle workers.
- Avoid restarting a recycled worker for a request that was already cancelled.
