import type { BatchFormatCoverage } from "./batch-model";
import { csvRow } from "./csv";

/** Stable, path-free report for sharing aggregate parser coverage. */
export function batchCoverageReport(rows: readonly BatchFormatCoverage[], totalFiles: number): string {
  const formats = [...rows]
    .sort((a, b) => a.format < b.format ? -1 : a.format > b.format ? 1 : 0)
    .map(({ format, files, complete, partial, unsupported }) => ({
      format,
      files,
      complete,
      partial,
      unsupported,
    }));
  const files = formats.reduce((total, row) => total + row.files, 0);
  return JSON.stringify({
    schema: "hexscope.batch-coverage",
    version: 1,
    files: {
      total: totalFiles,
      read: files,
      not_read: totalFiles - files,
    },
    formats,
  }, null, 2);
}

/** A spreadsheet-friendly coverage table with no per-file details. */
export function batchCoverageCsv(rows: readonly BatchFormatCoverage[], totalFiles: number): string {
  const formats = [...rows].sort((a, b) => a.format < b.format ? -1 : a.format > b.format ? 1 : 0);
  const read = formats.reduce((total, row) => total + row.files, 0);
  const complete = formats.reduce((total, row) => total + row.complete, 0);
  const partial = formats.reduce((total, row) => total + row.partial, 0);
  const unsupported = formats.reduce((total, row) => total + row.unsupported, 0);
  return [
    csvRow(["Format", "Files", "Complete", "Partial", "Unsupported", "Total files", "Read", "Not read"]),
    csvRow(["Total", read, complete, partial, unsupported, totalFiles, read, totalFiles - read]),
    ...formats.map((row) => csvRow([
      row.format,
      row.files,
      row.complete,
      row.partial,
      row.unsupported,
      totalFiles,
      read,
      totalFiles - read,
    ])),
  ].join("\r\n");
}
