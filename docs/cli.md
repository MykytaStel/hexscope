# The command line's contract

What `hexscope` on the command line keeps from one version to the next:
its commands and options, its exit status, and the shape of what it writes
for other programs to read. A script, a CI job or the
[GitHub Action](../action.yml) can rely on everything below.

Tests hold this page to the program: `crates/hexscope-cli/tests/contract.rs`
runs the command on every test file in the repository and fails when it
prints an option, a format, a kind of finding or a reason this page does
not list, or when a JSON or SARIF object gains or loses a field.

**Changes.** Until 1.0, a change to anything on this page is a minor
version with a line in the [changelog](../CHANGELOG.md)
([releasing.md](releasing.md)). From 1.0 on, taking something away or
changing what it means needs a major version. Adding is not a break: a new
format, kind of finding, rule, reason or field can come in any minor
version, so a reader should pass over names it does not know.

**Not part of it**: the lines written for people — the text output of
`check`, `clean`, `repair` and `redact`, everything on standard error, the
`label` and `text` of a finding, the words of a message. They are worded
for people and get better as the wording does.

## Commands

<!-- commands:start -->
| Command | What it does | What it writes |
| --- | --- | --- |
| `hexscope check PATH...` | What each file gives away, and what is wrong with it | Nothing; it reports |
| `hexscope PATH...` | The same as `check` | Nothing |
| `hexscope clean PATH...` | Copies without what they give away | `NAME-clean.EXT` beside each file, or into `--out` |
| `hexscope repair PATH...` | Copies of damaged files with what survived | `NAME-repaired.EXT` |
| `hexscope redact --text WORDS PATH...` | PDFs with `WORDS` blacked out of their pages | `NAME-redacted.EXT` |
| `hexscope batch --out DIR PATH...` | Clean copies of a folder, each read back, and a receipt | `DIR/000001-clean.EXT`…, `DIR/hexscope-report.json` |
| `hexscope mosaic PATH...` | What a set of photos has in common: a serial, a place, an order of dates | Nothing |
| `hexscope film --recipe JSON --out DIR PATH...` | One film recipe applied to a roll of scans | `DIR/000001-film.jpg`…, `DIR/hexscope-film-report.json` |
| `hexscope --help` | The usage | — |
| `hexscope --version` | `hexscope X.Y.Z` | — |
<!-- commands:end -->

`PATH` is a file or a folder. A folder is read through, its entries in
sorted order, leaving out `.git`, `node_modules`, `target`, `.venv` and
`__pycache__`. A symbolic link is never followed: meeting one stops the run
with exit status 2, so a run that skipped files cannot look complete.
`batch`, `mosaic` and `film` take up to 1000 files of up to 50 MiB each.
Every copy is written whole or not at all, and no command changes a file
it was given unless `clean --in-place` says so.

## Options

<!-- options:start -->
| Option | Commands | Meaning |
| --- | --- | --- |
| `--fail-on WHAT` | check | Exit 1 when a file has any of `WHAT`, a comma-separated list of `reveals`, `hidden`, `damage`, `oddity`, `misnamed` and [kinds of finding](#kinds-of-finding). Default `reveals,hidden,damage`; `none` never fails |
| `--json` | check, mosaic; clean with `--verify` | JSON instead of text |
| `--sarif` | check | One SARIF 2.1.0 log for all files |
| `--all` | check | Also list files hexscope does not read, and files with nothing to say |
| `--in-place` | clean | Replace each file with its clean copy |
| `--notes` | clean | Also empty Excel's and PowerPoint's comments and speaker notes |
| `--verify` | clean | Read each written copy back and say what was removed, remains, or could not be checked |
| `--out DIR` | clean, repair, redact, batch, film | Where the copies go; for one file with `clean` or `repair`, it can name the copy itself |
| `--text WORDS` | redact | What to black out, wherever it appears; give it more than once for more |
| `--recipe JSON` | film | The [film recipe](#film-recipe) to apply |
| `--format jpeg\|tiff16` | film | 8-bit JPEG (the default) or 16-bit TIFF |
| `--help` | — | The usage |
| `--version` | — | The version |
<!-- options:end -->

An option the command does not know is an error (exit status 2), never
ignored.

## Exit status

| Command | 0 | 1 | 2 |
| --- | --- | --- | --- |
| `check` | No file has what `--fail-on` names | A file has it | A usage error, a path that cannot be read, a symbolic link |
| `clean` | Every copy that could be made was written; files with nothing to remove, or of a kind it does not clean, do not count | A copy could not be made; with `--verify`, also a finding still present or not checked, or no copy at all | A usage error; two files would get one name; a copy would overwrite a file given; a file that cannot be read or written |
| `repair`, `redact` | As `clean` | A copy could not be made | As `clean`; `repair --in-place`; `redact` without `--text` |
| `batch` | Every copy written and every finding confirmed removed | A file not copied, or a finding present or not checked | A usage error; the output folder already holds a receipt or a copy |
| `mosaic` | Every file read as a whole photo | A file could not be checked | A usage error |
| `film` | Every copy written | A copy not written | A usage error; an invalid recipe; the output folder already holds a copy or a report |

`hexscope` with nothing after it prints the usage and exits 2; `--help` and
`--version` exit 0.

## check --json

One JSON object per file, one per line, in the order the files were read:

```json
{"file":"photos/beach.jpg","format":"jpeg","problems":[],"reveals":[{"kind":"location","text":"48.85840, 2.29450"},{"kind":"camera","text":"Pixel 9"}]}
```

| Field | Value |
| --- | --- |
| `file` | The path, as given or as found in a folder |
| `format` | One of the [formats](#formats) |
| `problems` | What is wrong with it: `label` (for people), `concern` (`damage`, `hidden` or `oddity`), and where — `offset` and `length` in bytes |
| `reveals` | What it gives away: `kind` (one of the [kinds of finding](#kinds-of-finding)) and `text`, the value as the file holds it |
| `misnamed` | Only when its name says another format: `ext` (the name's extension, lower case, no dot), `is` (what the file is, in words) and `fits` (the extension that fits, with its dot) |

`text` is what the file says — a name, a place, a serial number. JSON is
for your own machine; the [annotations](#in-github-actions) leave values
out.

## check --sarif

One [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html)
log on one line, for GitHub code scanning or any tool that reads SARIF. It
has one run; its driver is `hexscope` with its version and the rules its
results use.

| Rule | Level | Location |
| --- | --- | --- |
| `damage` | `error` | The file and the damaged bytes (`byteOffset`, `byteLength`) |
| `hidden` | `warning` | The file and the bytes |
| `oddity` | `note` | The file and the bytes |
| `misnamed` | `note` | The file |
| `reveals/KIND` | `warning` | The file; `KIND` is one of the [kinds of finding](#kinds-of-finding) |

A result's message is for people and, for `reveals/KIND`, holds the value.

## In GitHub Actions

When `GITHUB_ACTIONS` is `true`, `check` also writes a workflow command for
each file that fails: `::error` for damage and what is hidden, `::warning`
for what it gives away, `::notice` for an oddity or a misleading name. Each
names the file and the kind (`title=hexscope: reveals location`) and never
the value, so values stay out of the runner's log. The text output is left
out for the same reason.

With `--json` or `--sarif` the commands go to standard error, where the
runner reads them too, so that standard output holds only the report and a
report saved from it stays valid.

GitHub shows at most 10 annotations of a level for each step; a file that
gives away more shows its first ten. For every finding, upload the
`--sarif` report to code scanning.

## clean --verify --json

One object per file given, one per line. It holds kinds and reasons, never
values or the file's contents:

```json
{"source":"beach.jpg","output":"beach-clean.jpg","operation_state":"written","schema_version":1,"removed":[{"kind":"location","reason":"removed","unexpected":false}],"present":[],"unchecked":[]}
```

| Field | Value |
| --- | --- |
| `source`, `output` | The paths; `output` is `null` when no copy was made |
| `operation_state` | `written` or `not_created` |
| `schema_version` | `1` |
| `removed`, `present`, `unchecked` | Findings by what became of them: `kind`, `reason` (one of the [reasons](#reasons)) and `unexpected` (`true` when it is in the copy but was not in the file) |

A finding is `removed` only when the copy was read whole, the finding is
not in it, and that format and kind are among those
[confirmed removal](verification-coverage.md) covers; otherwise it stays
`unchecked`. A copy that could not be checked at all has one `unchecked`
item of kind `file` with the reason `verification_skipped`.

### Reasons

<!-- reasons:start -->
| Reason | Meaning |
| --- | --- |
| `removed` | It was in the file and is not in the copy |
| `still_present` | The same finding is in the copy |
| `value_changed_same_kind` | The copy holds this kind with another value |
| `no_stable_value` | It has no value to compare |
| `coverage_incomplete` | Removal of this format and kind is not confirmed yet |
| `parse_incomplete` | The file or the copy was not read whole |
| `unexpected_output` | It is in the copy but was not in the file |
| `no_comparable_findings` | Nothing to compare |
| `verification_skipped` | The check did not run |
<!-- reasons:end -->

## batch receipt

`DIR/hexscope-report.json`, without file names or values — a file is known
by its place in the sorted list of inputs, as is its copy
(`000001-clean.jpg` for the first):

```json
{"schema":"hexscope.batch-receipt","version":1,"total":2,"written":2,"failed":0,"removed":5,"present":0,"unchecked":1,
 "files":[{"index":1,"operation_state":"written","verification":{"schema_version":1,"removed":[…],"present":[],"unchecked":[]}}]}
```

`verification` is the same object as in
[clean --verify --json](#clean---verify---json), without the paths.

## mosaic --json

One object, with indexes into the sorted list of inputs and no names or
values:

```json
{"schema":"hexscope.photo-mosaic","version":1,"total":3,"serial_groups":[[1,2]],"location_groups":[],"located_files":0,"local_date_order":[1,2],"utc_date_order":[],"location_radius_metres":100,"inferences":false,"input_total":4,"not_checked_indexes":[4]}
```

`total` counts the photos read whole; `serial_groups` are files that share a
serial number; `location_groups` are files within `location_radius_metres`
of the first file of their group; `local_date_order` sorts the files whose
dates carry no time zone by when they were taken, and `utc_date_order` the
files whose dates do, in UTC.
`inferences` is always `false`: what files share is not proof of who took
them or where they live.

## film

### Film recipe

The file `--recipe` names, at most 16 KiB, as the film lab on the page
saves it:

```json
{"schema":"hexscope.film-recipe","version":1,"algorithm":"srgb-density-v1",
 "settings":{"mode":"color-negative","crop":[0.1,0.1,0.1,0.1],"exposure":0,"contrast":1,"baseColor":[220,140,85]}}
```

`mode` is `positive`, `color-negative` or `mono-negative`; `crop` is the
share cut from the left, top, right and bottom, each 0 to 0.45; `exposure`
is −2 to 2 stops; `contrast` 0.5 to 2; `baseColor` is the film base as
sRGB, each 1 to 255, or `null` to measure it from the first scan.

### Film report

`DIR/hexscope-film-report.json`: `schema` `hexscope.film-roll`, `version`
`1`, `algorithm`, `total`, `written`, `failed`, and per file its `index`,
`operation_state`, `width`, `height`, `source_depth`, `output_depth`,
`limited`, `profile_present`, `color_assumption`, `bytes_readback`,
`sha256` of the copy and `metadata_verification` — or, for a scan that was
not done, `index`, `operation_state` and `check`.

## Formats

<!-- formats:start -->
`jpeg`, `png`, `webp`, `gif`, `heif` (HEIC and AVIF), `tiff`, `video`
(MP4, MOV, M4A), `pdf`, `zip` (ZIP archives, Word, Excel, PowerPoint,
OpenDocument, EPUB, APK), `office97` (Word, Excel and PowerPoint 97–2003),
`msg` (Outlook), `cfb` (other compound files), `rtf`, `eml`, `wasm`,
`unknown`.
<!-- formats:end -->

## Kinds of finding

The kinds `reveals` and `--fail-on` use, by the files they come from. A kind
keeps its name and meaning; new ones are added as hexscope learns to read
more.

<!-- kinds:start -->
| Files | Kinds |
| --- | --- |
| Photos and videos | `location`, `camera`, `lens`, `serial`, `owner`, `place`, `taken`, `caption`, `software`, `original`, `history`, `thumbnail`, `shutter`, `uptime`, `linked`, `screenshot`, `ai`, `credentials`, `prompt` |
| Who made a document, and when | `author`, `editor`, `title`, `subject`, `keywords`, `description`, `category`, `status`, `company`, `manager`, `label`, `custom`, `created`, `modified`, `printed`, `revisions`, `editing`, `application`, `producer`, `template` |
| Where it has been | `folder`, `printer`, `links`, `downloaded`, `tags`, `photo`, `photoplace` |
| What a reader does not see | `covered`, `hiddentext`, `earlier`, `updates`, `deleted`, `tracked`, `comments`, `notes`, `hiddensheets`, `hiddencells`, `hiddenslides`, `form`, `attachments`, `embedded`, `encryption` |
| What a PDF does when opened | `scripts`, `launch`, `submits`, `opens` |
| Links | `weblinks`, `linkmismatch`, `linkidn` |
| Email | `sentfrom`, `computer`, `mailer`, `timezone`, `replyto`, `returnpath`, `auth`, `authfail`, `riskyfile` |
| WebAssembly | `paths`, `sourcemap`, `debug`, `debuginfo`, `names`, `language`, `toolchain`, `sdk`, `imports` |
<!-- kinds:end -->

`--fail-on` also takes the groups `reveals` (any kind), `hidden`, `damage`,
`oddity` and `misnamed`.
