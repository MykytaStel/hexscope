### Changed

- Adapt optional sparse media cache and batch reader retention to coarse device-memory hints, conservatively treating unknown memory as a middle profile.
- Avoid redundant LRU map mutations for consecutive bytes on the most recently used page.
- Keep input admission, findings, verification and all format limits unchanged.
