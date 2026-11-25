# Worker memory lifecycle

WASM linear memory has a high-water mark. Dropping Rust objects or deleting module-map entries does not unload the instance. A fatal runtime/allocator error now marks the reader poisoned, sends a fatal response, and closes the worker. RPC terminates the actual realm, settles waiting calls and clears abandoned timers. The existing workspace recovery flow is notified once; stale messages cannot kill the replacement.

Batch summaries no longer need their worker-owned parser tokens after policy and QR checks complete. Recycle the idle realm at the end of the queue or after eight files / 32 MiB of input, whichever comes first. These thresholds are a heuristic, not measured WASM memory. Never interrupt outstanding RPCs or an abandoned parse. Returning to the batch list closes the inspection workspace; reopening an item reparses it. Active inspection, nested state and copy verification are not proactively recycled.

PDF page tables, repair results and failed nested opens use cleanup in finally. No WASM budget changes. Reinitialization may cost time; test batch throughput and browser memory. Unexpected recovery still follows the existing workspace behavior (nested history may be lost). Full adaptive heap telemetry and safe active-document rehydration remain separate work.
