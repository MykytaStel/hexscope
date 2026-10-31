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

1. Pick the number: the next releases before 1.0 are substantial 0.9.x
   releases (0.9.1, 0.9.2, and so on), with features and fixes collected
   together. Additions must preserve the documented CLI and receipt contracts;
   review a breaking change and its version separately. Use normal SemVer
   numbers: `0.9.01` is invalid because numeric components cannot have leading
   zeroes. The examples below use `0.9.1`; choose the actual number only when
   the release scope is ready.
2. In `Cargo.toml`, set the version; set the same in `hexscope-core`'s
   dependency on `hexscope-inflate`, `hexscope-raster`'s on `hexscope-core`,
   and the CLI's on `hexscope-core` and `hexscope-raster`.
3. Set `VERSION` to the number chosen above. `node scripts/changelog.mjs
   "$VERSION"` writes everything in `changes/` into `CHANGELOG.md` under that
   version, newest first, adds its compare link at the bottom and removes the
   files. Read it through once.
4. Open a pull request; CI checks that the changelog has the new version.
5. Once it is merged and deployed, tag it and publish the release notes:

   ```sh
   VERSION=0.9.1
   git tag -a "v$VERSION" -m "hexscope $VERSION" && git push origin "v$VERSION"
   gh release create "v$VERSION" --title "hexscope $VERSION" --notes-file <(sed -n "/^## \[$VERSION\]/,/^## \[/p" CHANGELOG.md | sed '$d')
   ```

   Publishing the release builds the command for Linux (x86-64 and ARM,
   static), macOS (Apple silicon and Intel) and Windows, and attaches each
   with its checksum and a build attestation
   ([`release.yml`](../.github/workflows/release.yml)); the GitHub Action
   at that tag downloads them. Check the release page lists ten files.

6. Once the release lists its ten files, update the Homebrew tap
   ([MykytaStel/homebrew-hexscope](https://github.com/MykytaStel/homebrew-hexscope))
   from the checksums they carry. Open and merge a tap pull request, then
   install the formula and run its test:

   ```sh
   BRANCH="release/hexscope-$VERSION"
   (cd ../homebrew-hexscope && git switch -c "$BRANCH")
   node scripts/homebrew-formula.mjs "$VERSION" > ../homebrew-hexscope/Formula/hexscope.rb
   (cd ../homebrew-hexscope && git add Formula/hexscope.rb && git commit -m "hexscope $VERSION" && git push -u origin "$BRANCH")
   gh pr create --repo MykytaStel/homebrew-hexscope --base main --head "$BRANCH" --title "hexscope $VERSION" --body "Update to the published v$VERSION checksums."
   # After the tap pull request is merged:
   brew update
   brew install mykytastel/hexscope/hexscope # first install
   brew upgrade mykytastel/hexscope/hexscope
   brew test mykytastel/hexscope/hexscope
   ```

7. Publishing to crates.io is a separate decision each time. From a clean
   checkout of the tagged commit, run CI for the exact commit, then package
   every publishable crate with:

   ```sh
   cargo publish --workspace --locked --dry-run
   ```

   This dry run packages and verifies the four publishable crates without
   uploading them. The workspace-only WebAssembly bridge has `publish =
   false` and is not one of those four. Repeat the dry run on every release
   tag: success on an older version is not evidence for a newer package.
   Keep `--no-verify` out of this check; CI separately compiles and tests the
   workspace sources.
   After deciding to publish, run `cargo publish --workspace --locked`. Cargo
   publishes `hexscope-inflate`, `hexscope-core`, `hexscope-raster` and
   `hexscope-cli` in dependency order.
