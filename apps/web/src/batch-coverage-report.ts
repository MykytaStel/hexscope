import type { BatchFormatCoverage } from "./batch-model";

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
