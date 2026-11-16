/**
 * Normalise a subprocess `signalCode` to the signal name string (or null).
 * Bun reports a name such as "SIGTERM"; @types/bun 1.4.4 widened the type to
 * `number | Signals | null`, so a numeric code is rendered as its decimal
 * string rather than being rejected by the string-typed record fields.
 */
export function signalName(code: number | string | null | undefined): string | null {
  if (code === null || code === undefined) return null;
  return typeof code === "number" ? String(code) : code;
}
