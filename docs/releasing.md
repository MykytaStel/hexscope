# Releasing a version

One version number covers the website, the CLI and the crates. It lives in
one place: `version` under `[workspace.package]` in the root `Cargo.toml`.
The site reads it at build time and shows it in its footer and in structure
reports; `hexscope --version` prints it.

## While working

Every merged change that a person would notice adds a line under
`## [Unreleased]` in [`CHANGELOG.md`](../CHANGELOG.md), in Added, Changed,
Fixed or Removed. Say what changed for people, not which files.

## Making a version

1. Pick the number: until 1.0, a new feature or a change to the CLI's JSON
   is a minor version (0.3.0), fixes only are a patch (0.2.1).
2. In `Cargo.toml`, set the version; set the same in `hexscope-core`'s
   dependency on `hexscope-inflate` and the CLI's on `hexscope-core`.
3. In `CHANGELOG.md`, rename `[Unreleased]` to `[0.3.0] — YYYY-MM-DD`, start
   a new empty `[Unreleased]`, and update the compare links at the bottom.
4. Open a pull request; CI checks that the changelog has the new version.
5. Once it is merged and deployed, tag it and publish the release notes:

   ```sh
   git tag -a v0.3.0 -m "hexscope 0.3.0" && git push origin v0.3.0
   gh release create v0.3.0 --title "hexscope 0.3.0" --notes-file <(sed -n '/^## \[0.3.0\]/,/^## \[/p' CHANGELOG.md | sed '$d')
   ```

6. Publishing to crates.io is a separate decision each time: core first, then
   the CLI (`cargo publish -p hexscope-inflate`, `-p hexscope-core`,
   `-p hexscope-cli`).
