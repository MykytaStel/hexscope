### Fixed

- **A timed-out copy check no longer leaves the reader blocked.** If copy verification exceeds its time limit, Hexscope stops the parser worker and tells you to reopen the file; later requests no longer wait behind the timed-out work.
