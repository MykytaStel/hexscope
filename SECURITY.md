# Security

hexscope opens files people do not trust, in the browser of whoever opens
them. Two things matter most:

- **The parser must never panic, hang or run away with memory** on any
  input. It is fuzzed in CI and bounded by budgets in the code, but a file
  that makes it misbehave is a security bug.
- **Nothing leaves the tab.** The site's Content-Security-Policy allows no
  script, style or connection from anywhere else (`apps/web/public/_headers`).
  Anything that sends a byte of a file elsewhere, or runs code from inside a
  file, is a security bug.

## Reporting

Please report security bugs privately, through GitHub's
[private vulnerability reporting](https://github.com/MykytaStel/hexscope/security/advisories/new),
not in a public issue.

If the bug needs a file to show it, do not send a private one: build the
smallest file that shows the problem, or send hexscope's *structure report*
(under the file's details, "Report a problem with this file"), which holds
the file's layout and none of its content.

Fixes go out as soon as they are ready; the site is the only release.
