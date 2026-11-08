/** One RFC 4180-compatible CSV row, safe to open in a spreadsheet. */
export function csvRow(values: readonly (string | number)[]): string {
  return values.map((value) => {
    const text = String(value);
    const safe = /^[\t\r ]*[=+\-@]/.test(text) ? `'${text}` : text;
    return /[",\r\n]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
  }).join(",");
}
