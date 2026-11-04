import type { Diagnostic } from "../api/types";

/**
 * The equation line a diagnostic points into, or -1. Spans are byte
 * offsets into the non-empty lines joined with newlines.
 */
export function diagnosticLine(d: Diagnostic, lines: string[]): number {
  if (!d.span) return -1;
  const enc = new TextEncoder();
  let offset = 0;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim() === "") continue;
    const end = offset + enc.encode(lines[i]).length;
    if (d.span[0] <= end) return i;
    offset = end + 1;
  }
  return -1;
}
