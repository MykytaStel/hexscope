# PDF page context performance

The named PDF profile attributed 32380 raw body bytes to parse_with. Page-facing operations previously performed a complete inspection and then their own page walk. Split the common structural scan from full findings. The page context is private and cannot be mistaken for a complete inspection result.

The structural scan retains revision parsing, damage reporting, xref checks, unlock and object indexing. Only five read-only page callers use the lighter path: page_texts, jpegs_under, page_pictures, pictures and shapes. Primary inspection, attachment lookup, cleaning and output verification still use the full path. The separate page walk retains its own limits and completeness decisions. No new safety shortcut is used for a clean copy.

Evidence: parity tests compare page text, picture positions and shapes using the full context and page context on real, revised, packed and encrypted fixtures. Existing PDF verification, redaction and web tests must also pass. Measure same-machine repeated page operations and production gzip before attributing a speed or size improvement; the shared structural tree is still allocated. No limits or feature flags are changed.
