# Flat spatial index

FileModel previously allocated one Int32Array per node for spatial descent, including empty arrays for leaves. The replacement uses Uint32Array offsets (n+1) and a single Int32Array edge arena (at most n-1 entries), plus one reusable sorting list. A leaf allocates no index object; a single child bypasses sorting. Ordinary child order, zero-length exclusion, stable ties, partial overlap exclusion and byte ownership are unchanged. Lookup descends using the same binary search semantics within flat ranges.

The two spatial array payloads occupy approximately 8*n bytes for a nonempty tree; the old per-node objects and their runtime overhead are not included in that comparison. The new arena may retain unused slots when siblings overlap. This is not a measured heap/RSS claim. Tests compare against the previous algorithm for generated nested trees and a 100000-leaf stress case. Rebuild and check analyzer bundle limits.
