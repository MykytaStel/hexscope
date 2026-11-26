### Changed

- Yield during large individual entropy windows rather than only between windows.
- Preserve histogram state across bounded chunks and choose conservative CPU time slices from coarse concurrency hints.
- Retain exact entropy values, sampling policy and cancellation semantics.
