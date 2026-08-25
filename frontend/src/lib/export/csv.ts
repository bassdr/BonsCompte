export type CsvValue = string | number | boolean | null | undefined;

/**
 * Build an RFC 4180 CSV string. Fields containing commas, quotes or newlines
 * are quoted; quotes are doubled. Rows end with CRLF for Excel compatibility.
 */
export function toCsv(headers: string[], rows: CsvValue[][]): string {
  const escapeField = (value: CsvValue): string => {
    if (value === null || value === undefined) return '';
    const s = String(value);
    if (/[",\n\r]/.test(s)) {
      return '"' + s.replace(/"/g, '""') + '"';
    }
    return s;
  };
  const lines = [headers, ...rows].map((row) => row.map(escapeField).join(','));
  return lines.join('\r\n') + '\r\n';
}

/**
 * Trigger a browser download of the given rows as a UTF-8 CSV file.
 * A BOM is prepended so Excel detects the encoding (accents survive).
 */
export function downloadCsv(filename: string, headers: string[], rows: CsvValue[][]): void {
  const blob = new Blob(['\uFEFF' + toCsv(headers, rows)], {
    type: 'text/csv;charset=utf-8'
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}

/** bonscompte-<name>-<YYYY-MM-DD>.csv */
export function csvFilename(name: string, date?: string): string {
  const datePart = date ?? new Date().toISOString().slice(0, 10);
  return `bonscompte-${name}-${datePart}.csv`;
}
