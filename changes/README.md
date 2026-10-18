# What changed, not released yet

Every merged change that a person would notice adds one file here, named
after it (`svg-guide.md`, `cli-skipped-files.md`), with Keep a Changelog
sections and a list under each:

```markdown
### Added

- **What is new, in bold**, then what it means for people, not which
  files changed.

### Fixed

- **What was wrong**, and what happens now.
```

Sections: Added, Changed, Deprecated, Removed, Fixed, Security. Two pull
requests that each add their own file never conflict.

`node scripts/changelog.mjs` shows how the next version will read;
`node scripts/changelog.mjs --check` is what CI runs; making a version
(`node scripts/changelog.mjs 0.7.0`) writes them into
[`CHANGELOG.md`](../CHANGELOG.md), newest first, and removes them
([docs/releasing.md](../docs/releasing.md)).
