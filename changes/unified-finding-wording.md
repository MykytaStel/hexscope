### Changed

- **File metadata finding words come from one core catalogue**: the page's
  detail labels, category summaries and verdicts, plus the CLI's human
  output, annotations and SARIF descriptions, all use the same stable
  finding codes. JSON keeps the code and value for tools.
- **Verification reason codes define their report groups and default words**
  in a shared table used by both the page and CLI. JSON and saved receipts
  keep the codes without user-facing sentences.
- **Parser problems keep their stable explanation code across interfaces.**
  CLI, JSON, GitHub annotations and SARIF use the same sentence as the page;
  JSON also retains the parser's structural label and byte range.
