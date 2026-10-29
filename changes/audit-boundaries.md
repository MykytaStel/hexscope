### Fixed

- **Office files with conflicting ZIP entry headers no longer produce a misleading clean copy.** Hexscope refuses to rewrite an entry when its local and central records disagree on the name, flags or compression method.
- **Feedback requests are bounded while they arrive.** Oversized bodies are stopped before they can be fully buffered, and simultaneous submissions cannot bypass the daily message limit.
