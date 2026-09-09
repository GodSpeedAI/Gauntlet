/**
 * Semantic theme tokens and glyph formatters.
 * Conforms to DSN-001, DSN-002, DSN-003, and DSN-004.
 */

export interface StatusDescriptor {
  readonly glyph: string;
  readonly label: string;
  readonly category: "settlement" | "evidence" | "execution" | "readiness";
  readonly emphasis: "normal" | "bold" | "dim";
}

/**
 * Maps unit/system states to distinct glyph + label + emphasis tuples (DSN-002).
 * Color degrades gracefully to plain text without loss of distinction.
 */
export function formatStatus(status: string): StatusDescriptor {
  switch (status.toLowerCase()) {
    case "pass":
    case "settled":
      return {
        glyph: "✓",
        label: "PASS",
        category: "settlement",
        emphasis: "bold",
      };
    case "fail":
      return {
        glyph: "✗",
        label: "FAIL",
        category: "evidence",
        emphasis: "bold",
      };
    case "inconclusive":
      return {
        glyph: "?",
        label: "INCONCLUSIVE",
        category: "evidence",
        emphasis: "normal",
      };
    case "running":
      return {
        glyph: "▶",
        label: "RUNNING",
        category: "execution",
        emphasis: "bold",
      };
    case "blocked":
      return {
        glyph: "■",
        label: "BLOCKED",
        category: "readiness",
        emphasis: "normal",
      };
    default:
      return {
        glyph: "·",
        label: status.toUpperCase(),
        category: "readiness",
        emphasis: "dim",
      };
  }
}

/**
 * Format an evidence reference with source identity and outcome (DSN-008).
 */
export function formatEvidenceItem(
  unitId: string,
  outcome: string,
  evidenceRef: string,
  verificationRef: string,
): string {
  const st = formatStatus(outcome);
  const ev = evidenceRef || "unavailable";
  const ver = verificationRef || "unavailable";
  return `[${st.glyph} ${st.label}] Unit: ${unitId} | Ev: ${ev} | Ver: ${ver}`;
}
