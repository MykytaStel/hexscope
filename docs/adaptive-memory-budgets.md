# Adaptive retention budgets

The browser's optional deviceMemory value is a coarse, privacy-limited device hint, not current available RAM. Three retention profiles are used:

| Reported GiB | Media pages (64 KiB each) | Max concurrent page reads | Batch reader recycle (files or input bytes) |
| --- | ---: | ---: | --- |
| 0 < hint <= 2 | 16 (1 MiB) | 2 | 2 / 8 MiB |
| unknown, invalid, or 2 < hint <= 4 | 32 (2 MiB) | 3 | 4 / 16 MiB |
| hint > 4 | 64 (4 MiB) | 4 | 8 / 32 MiB |

No user-agent inference; an absent hint is normal on Safari. Cache size applies per sparse model, not globally. Outstanding read buffers are additional. Batch thresholds are heuristics based on input size, not measured WASM memory. Recycling still refuses active requests and never discards an open document. Smaller budgets trade cache hits and warm parser reuse for lower retained optional state. Input size, parser coverage, decompression limits, QR and verification are not reduced. Existing download prefetch policy is unchanged.

Measure repeat opens, sparse scrolling and batch latency on physical devices before tuning these defaults. A smaller cache capacity is not a measured peak RSS improvement.
