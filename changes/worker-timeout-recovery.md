### Fixed

- **A timed-out copy check no longer leaves the reader blocked.** If copy verification exceeds its time limit, Hexscope stops the parser worker and reopens the current file from its local source so the reader can continue. Requests from the stopped worker cannot affect its replacement.
