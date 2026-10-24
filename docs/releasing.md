# Releasing a version

One version number covers the website, the CLI and the crates. It lives in
one place: `version` under `[workspace.package]` in the root `Cargo.toml`.
The site reads it at build time and shows it in its footer and in structure
reports; `hexscope --version` prints it.

## While working

Every merged change that a person would notice adds a file of its own to
[`changes/`](../changes/README.md), with its lines under Added, Changed,
Fixed or Removed. Say what changed for people, not which files. Pull
requests then never conflict in [`CHANGELOG.md`](../CHANGELOG.md);
`node scripts/changelog.mjs` shows how the next version will read.

## Making a version

1. Pick the number: until 1.0, a new feature or a change to the CLI's JSON
   is a minor version (0.3.0), fixes only are a patch (0.2.1).
2. In `Cargo.toml`, set the version; set the same in `hexscope-core`'s
   dependency on `hexscope-inflate`, `hexscope-raster`'s on `hexscope-core`,
   and the CLI's on `hexscope-core` and `hexscope-raster`.
3. `node scripts/changelog.mjs 0.3.0` writes everything in `changes/` into
   `CHANGELOG.md` as `[0.3.0] — today`, newest first, adds its compare link
   at the bottom and removes the files. Read it through once.
4. Open a pull request; CI checks that the changelog has the new version.
5. Once it is merged and deployed, tag it and publish the release notes:

   ```sh
   git tag -a v0.3.0 -m "hexscope 0.3.0" && git push origin v0.3.0
   gh release create v0.3.0 --title "hexscope 0.3.0" --notes-file <(sed -n '/^## \[0.3.0\]/,/^## \[/p' CHANGELOG.md | sed '$d')
   ```

   Publishing the release builds the command for Linux (x86-64 and ARM,
   static), macOS (Apple silicon and Intel) and Windows, and attaches each
   with its checksum and a build attestation
   ([`release.yml`](../.github/workflows/release.yml)); the GitHub Action
   at that tag downloads them. Check the release page lists ten files.

6. Once the release lists its ten files, update the Homebrew tap
   ([MykytaStel/homebrew-hexscope](https://github.com/MykytaStel/homebrew-hexscope))
   from the checksums they carry, and check it installs:

   ```sh
   node scripts/homebrew-formula.mjs 0.3.0 > ../homebrew-hexscope/Formula/hexscope.rb
   (cd ../homebrew-hexscope && git commit -am "hexscope 0.3.0" && git push)
   brew install mykytastel/hexscope/hexscope && brew test hexscope
   ```

7. Publishing to crates.io is a separate decision each time. From a clean
   checkout of the tagged commit, run CI for the exact commit, then package
   every publishable crate with:

   ```sh
   cargo publish --workspace --locked --dry-run --no-verify
   ```

   `--no-verify` is needed before publishing a new workspace version: Cargo's
   default tarball verification resolves dependencies from crates.io, where
   `hexscope-core` 0.8.0 is not available yet, so the CLI is checked against
   the older published API and fails. CI compiles and tests the workspace
   sources; this dry run checks that all four crates package successfully.
   After deciding to publish, run `cargo publish --workspace --locked`. Cargo
   publishes `hexscope-inflate`, `hexscope-core`, `hexscope-raster` and
   `hexscope-cli` in dependency order.
