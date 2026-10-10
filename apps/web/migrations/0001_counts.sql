-- Totals only: how many people a day or a month, and how many of a day's
-- first visits came from each country, site, page, kind of device and
-- language. Nothing here can name a visitor.
CREATE TABLE counts (
  period TEXT NOT NULL, -- '2026-10-10' for a day, '2026-10' for a month
  metric TEXT NOT NULL, -- people, new, country, from, page, device, lang
  value TEXT NOT NULL DEFAULT '', -- '' for people and new; 'UA', 'reddit.com', '/stats', ...
  n INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (period, metric, value)
) WITHOUT ROWID;
