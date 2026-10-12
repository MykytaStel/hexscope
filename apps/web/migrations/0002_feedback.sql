-- What people chose to send from the site: their words, a way to answer
-- them if they left one, and the structure report of the open file if they
-- ticked the box — its layout, none of its content. No address, no cookie.
-- The function makes this table on first use too (server/handlers.ts).
CREATE TABLE IF NOT EXISTS feedback (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  day TEXT NOT NULL, -- '2026-10-13'
  kind TEXT NOT NULL, -- bug, idea, thanks
  message TEXT NOT NULL,
  contact TEXT NOT NULL DEFAULT '',
  report TEXT NOT NULL DEFAULT '',
  page TEXT NOT NULL DEFAULT '',
  lang TEXT NOT NULL DEFAULT '',
  version TEXT NOT NULL DEFAULT '',
  device TEXT NOT NULL DEFAULT '',
  done INTEGER NOT NULL DEFAULT 0
);
