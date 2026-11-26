# Background scan lifecycle

The interactive analyzer owns one AbortController per QR/film-clue job. Workspace transitions abort old jobs before switching parser state; cancelling or failing a file open allows the retained model to be inspected again. A cancelled worker reply is not evidence of a completed scan. Completion is recorded only after a non-cancelled reply, and a late result cannot duplicate facts. The main-thread candidate enumeration stops after the same first thirty eligible pictures.

Cancellation is cooperative: synchronous WASM/image decoding cannot be interrupted mid-call. Required core inspection and cleaning verification are not skipped. Batch QR continues to use its existing independent signal. Tests cover cancellation and retry, no unavailable film clue on abort, and bounded candidate enumeration.
