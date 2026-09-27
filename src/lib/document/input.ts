/** Numeric input helpers (R27): a legitimate 0 must survive parsing; only
 * NaN/empty input falls back to the default. The old `parseFloat(v) || def`
 * pattern silently turned 0 into the default. Pure logic — tested under node. */

export function numOr(value: string, fallback: number): number {
  const n = parseFloat(value);
  return Number.isFinite(n) ? n : fallback;
}

export function intOr(value: string, fallback: number): number {
  const n = parseInt(value, 10);
  return Number.isFinite(n) ? n : fallback;
}

export function clampInt(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

/** Checkbox/radio export values the current fill_form backend expresses as
 * checked (truthy). Anything else is preserved as-is and never guessed (R10). */
const TRUTHY_EXPORT_RE = /^(yes|true|1|on|checked)$/i;

export function isTruthyExportValue(value: string): boolean {
  return TRUTHY_EXPORT_RE.test(value.trim());
}
