# hexscope-cli

The `hexscope` command-line tool checks what files reveal, and makes cleaned,
repaired or redacted copies. Files stay on your machine.

Install the published crate from crates.io:

```sh
cargo install --locked hexscope-cli
```

Check a file and fail the command when it reveals a location:

```sh
hexscope check --fail-on location photo.jpg
```

Save a clean copy, repair a damaged image, or redact text in a PDF:

```sh
hexscope clean --out clean/ photo.jpg report.pdf
hexscope repair --out repaired/ broken.png
hexscope redact --text "Private Name" --out redacted/ report.pdf
```

Use `--json` for one JSON record per file or `--sarif` for a SARIF 2.1.0
report that GitHub Code Scanning can ingest. `--help` lists all commands,
options and exit codes. By default, `check` exits non-zero for reveals,
hidden content or damage; choose `--fail-on` to tune a CI gate.

The command-line tool uses the same parser as the Hexscope web app. It does
not upload files or send telemetry.

Licensed under either MIT or Apache-2.0, at your option.
