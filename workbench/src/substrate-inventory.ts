/**
 * Substrate inventory RE-QUALIFICATION against the real installed prime-agent
 * binary (plan T24; discharges T10's trigger-record MAJOR 2; REQ-PRIME-008
 * provenance mapping; prereg D3 / intervention class A2).
 *
 * Core Invariants:
 *   - THREE-WAY provenance, per feature, never merged: what the REAL INSTALLED
 *     binary exposes (probed fact — `--help`/`version`/short real invocations),
 *     what the T04/T10 IN-PROCESS ADAPTER MODEL exposes (the layer T05/T10
 *     honestly recorded), and what the workbench ADMITS through its canonical
 *     path. A layer's honest "unsupported" is never overwritten by another
 *     layer's abundance: T10's recordings stay true at their own layer, and the
 *     real binary's surface is recorded HERE without promoting it to the model.
 *   - NO CANONICAL TRUTH: every row carries `canonical_direction`,
 *     `canonical_settlement`, and `canonical_capability` as LITERAL `false`
 *     (the same type-level seal as `PrimeSubstrateRecord`). Exposure on any
 *     substrate layer is execution provenance only — never Gauntlet direction,
 *     settlement, or a capability claim (REQ-PRIME-008).
 *   - BOUNDED claims: "not in probed CLI surface" is a statement about the
 *     probed surface (`--help`/`version`/short invocations of the pinned
 *     binary), never a universal absence claim (AGENTS.md: absence claims are
 *     bounded by the declared search procedure).
 *   - Evidence-anchored: every probed answer names its probe commands; the
 *     deterministic probe corpus is pinned under PROBE_CAPTURES with SHA-256
 *     digests of the captured outputs (T14B's CAPTURES discipline).
 *
 * This module is data + typed queries. It spawns nothing at import time; the
 * only process-spawning helper (`probePrimeSurface`) runs when called — from
 * the env-gated battery (`GAUNTLET_T24_REAL_PRIME=1`) or explicit tooling.
 */

// --- Qualified binary identity -------------------------------------------------

/** The identity of the real binary this requalification probed. */
export interface PrimeBinaryIdentity {
  readonly binary: string;
  readonly version: "0.8.0";
  /** Build id reported by the background service (`status`) for this install. */
  readonly build_id: "8d7deea-dirty";
  /** sha256 of the launcher at ~/.local/bin/prime-agent (prefix; full digest in evidence). */
  readonly launcher_sha256: string;
}

/**
 * The binary qualified on 2026-09-09/10 (this host). A DIFFERENT version is a
 * requalification signal, not a pass: `qualifyPrimeBinary` records the mismatch
 * and the gated battery records the lifecycle as recorded-unqualified.
 */
export const QUALIFIED_PRIME_BINARY: PrimeBinaryIdentity = Object.freeze({
  binary: "/home/sprime01/.local/bin/prime-agent",
  version: "0.8.0" as const,
  build_id: "8d7deea-dirty" as const,
  launcher_sha256: "38e3475ad4a1772155f709586f60ef6ad1214d4c05f4776d04611b0041ce530e",
});

// --- Probe corpus (deterministic subset: digests re-assertable) -----------------

/** One recorded probe: the command, its captured output file, and its digest. */
export interface ProbeCapture {
  readonly id: string;
  readonly command: string;
  /** Path relative to the repository root of the captured output. */
  readonly capture: string;
  readonly sha256: string;
}

/**
 * Deterministic probe captures (stable per binary build; a test may re-verify
 * these digests against the evidence files when present). Lifecycle artifacts
 * (run transcripts, agent inventory) are recorded in the evidence narrative and
 * CAPTURES.sha256 but are intentionally NOT here: they are not byte-stable.
 */
export const PROBE_CAPTURES: readonly ProbeCapture[] = Object.freeze([
  Object.freeze({
    id: "P-VERSION",
    command: "prime-agent --version",
    capture: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/version.txt",
    sha256: "a66780da23103beaf12432b418508966b31a2a3a787f9468a5b6cdaa8667c1ef",
  }),
  Object.freeze({
    id: "P-HELP-RUN",
    command: "prime-agent --help",
    capture: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/help.txt",
    sha256: "4dd6715ff756a277a96dab0ec880293d74bfeb31055115299e08a1c38a4c64dd",
  }),
  Object.freeze({
    id: "P-HELP-SCHEDULE",
    command: "prime-agent schedule --help",
    capture: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/schedule-help.txt",
    sha256: "5bd4e6a1e671b4ebc24883a4458a72e2637b23b65dd94352d8b30566c9063e61",
  }),
  Object.freeze({
    id: "P-HELP-SEND",
    command: "prime-agent send --help",
    capture: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/send-help.txt",
    sha256: "f77bdb9179c6e8f98db99d7b68a4af06f65d1f47a2adde2fd26741cc25d505f6",
  }),
  Object.freeze({
    id: "P-HELP-STOP",
    command: "prime-agent stop --help (plus attach/list in the same capture)",
    capture:
      ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/stop-attach-list-help.txt",
    sha256: "c53c2c8fb8e45f693a6e32b49106f837d31f20c0a37843bb5e50193897ebec99",
  }),
  Object.freeze({
    id: "P-HELP-ATTACH",
    command: "prime-agent attach --help (same capture as P-HELP-STOP)",
    capture:
      ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/stop-attach-list-help.txt",
    sha256: "c53c2c8fb8e45f693a6e32b49106f837d31f20c0a37843bb5e50193897ebec99",
  }),
  Object.freeze({
    id: "P-HELP-LIST",
    command: "prime-agent list --help (same capture as P-HELP-STOP)",
    capture:
      ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/stop-attach-list-help.txt",
    sha256: "c53c2c8fb8e45f693a6e32b49106f837d31f20c0a37843bb5e50193897ebec99",
  }),
  Object.freeze({
    id: "P-SEND-REFUSED",
    command: 'prime-agent send <agent> --follow-up -- "msg" (executed; drift capture)',
    capture:
      ".agents/reports/gauntlet-tui-prime-workbench/evidence/T24/captures/send-follow-up-refused.txt",
    sha256: "25fe155a1fcb6135ea6d293c3784f77880c69bfa74aa66cd68565876171b71da",
  }),
]);

// --- The three-way provenance map ----------------------------------------------

/**
 * Every mapped feature: the seven REQ-PRIME-008 substrate kinds (each carries
 * its `PrimeSubstrateKind` class) plus the delegation/session controls the T10
 * trigger-record names for the real 0.8.0 binary (send/stop/attach/list/resume,
 * child budget, delivery-mode control, admission, mailbox, result contract).
 */
export type SubstrateFeatureId =
  | "goal"
  | "schedule"
  | "autonomous_mode"
  | "heartbeat"
  | "compaction"
  | "refinement"
  | "harness_state"
  | "child_budget"
  | "child_session_admission"
  | "mailbox_communication"
  | "mailbox_delivery_mode_control"
  | "result_contract_completion"
  | "session_stop"
  | "session_attach"
  | "session_list"
  | "session_resume";

/** How the real installed binary exposes the feature (probed fact). */
export type RealBinaryExposureStatus =
  /** `--help` names it AND a short real invocation executed it on this host. */
  | "exposed_executed"
  /** `--help` names it; lifecycle execution not exercised on this host. */
  | "exposed_probed"
  /** `--help` documents it but the executed implementation refuses it (drift). */
  | "documented_but_refused"
  /** No token in the probed CLI surface (`--help`/`version` corpus). Bounded claim. */
  | "not_in_probed_cli_surface";

export interface ProbeRef {
  readonly probe_id: string;
  readonly detail: string;
}

export interface RealBinaryExposure {
  readonly status: RealBinaryExposureStatus;
  /** The probes the answer is anchored in (PROBE_CAPTURES ids or lifecycle artifacts). */
  readonly probes: readonly ProbeRef[];
  readonly detail: string;
}

/** How the T04/T10 in-process adapter model exposes the feature. */
export type AdapterModelExposureStatus = "exposed" | "not_exposed";

export type AdapterModelSource =
  | "substrateInventory"
  | "delegationCapabilityFacts"
  | "PrimeHostAdapter_surface"
  | "PrimeWorkbenchHost_surface";

export interface AdapterModelExposure {
  readonly status: AdapterModelExposureStatus;
  readonly source: AdapterModelSource;
  /** For not_exposed: the recorded honest reason at this layer (unchanged by T24). */
  readonly reason: string | null;
}

/** What the workbench admits for the feature through its canonical path. */
export type WorkbenchAdmissionClass =
  /** `requestSubstrateStrategy` refuses; recorded unsupported, never simulated. */
  | "refused_unsupported_substrate"
  /** Workbench-side typed policy (RecursionPolicy), not a substrate capability. */
  | "workbench_policy_enforced"
  /** Rides the ONE canonical admission path (gateway -> route -> adapter). */
  | "admitted_via_canonical_path"
  /** Delivered immediately; no deferred/steer/follow-up mode control admitted. */
  | "delivered_immediately_unsupported_mode"
  /** The canonical surface serves no stop operation (REQ-OPS-003). */
  | "no_canonical_stop_registered"
  /** Recovery of topology/session refs only from qualified records (REQ-PRIME-010). */
  | "reattach_from_qualified_records_only";

export interface WorkbenchAdmission {
  readonly admission: WorkbenchAdmissionClass;
  readonly detail: string;
}

/** One row of the three-way provenance map. Canonical fields are LITERAL false. */
export interface SubstrateProvenanceRow {
  readonly feature: SubstrateFeatureId;
  /** The REQ-PRIME-008 substrate kind this feature is an instance of, if any. */
  readonly req_prime_008_class:
    | "goal"
    | "schedule"
    | "autonomous_mode"
    | "heartbeat"
    | "compaction"
    | "refinement"
    | "harness_state"
    | null;
  readonly real_binary: RealBinaryExposure;
  readonly adapter_model: AdapterModelExposure;
  readonly workbench: WorkbenchAdmission;
  readonly provenance_note: string;
  /** LITERAL false: never Gauntlet direction. */
  readonly canonical_direction: false;
  /** LITERAL false: never Gauntlet settlement. */
  readonly canonical_settlement: false;
  /** LITERAL false: never a Gauntlet capability claim. */
  readonly canonical_capability: false;
}

const MODEL_UNSUPPORTED = "not_exposed_by_installed_prime_adapter_model" as const;

/**
 * The map itself. Probed on 2026-09-09/10 against the qualified binary
 * (QUALIFIED_PRIME_BINARY) on this host; adapter-model answers are the T05/T10
 * recordings UNCHANGED; workbench answers are the typed admission classes.
 */
export const SUBSTRATE_PROVENANCE_MAP: readonly SubstrateProvenanceRow[] = Object.freeze([
  Object.freeze({
    feature: "goal",
    req_prime_008_class: "goal",
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({ probe_id: "P-HELP-RUN", detail: "--goal <objective> documented" }),
        Object.freeze({
          probe_id: "L-AUTONOMOUS-GOAL",
          detail: "real headless run with --goal 'compute correctly' completed and answered 42",
        }),
      ]),
      detail: "seeds a persistent goal for a new root session; executed for real on this host",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "substrateInventory",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "refused_unsupported_substrate",
      detail:
        "requestSubstrateStrategy('goal', ...) returns the typed unsupported outcome; no admitted strategy maps onto goals",
    }),
    provenance_note:
      "T10's adapter-model recording stays true at its layer; the real binary's goal surface is provenance, never canonical Case direction (REQ-PRIME-008).",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "child_budget",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({ probe_id: "P-HELP-RUN", detail: "--goal-token-budget <n> documented" }),
        Object.freeze({
          probe_id: "L-AUTONOMOUS-GOAL",
          detail: "real run carried --goal-token-budget 5000",
        }),
      ]),
      detail: "positive token budget for --goal; the T10 trigger-record's 'child-budget' surface",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "delegationCapabilityFacts",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "workbench_policy_enforced",
      detail:
        "RecursionPolicy.max_children_per_parent enforced as the typed child_budget_exhausted pre-delegation refusal — workbench policy, never presented as a Prime capability",
    }),
    provenance_note:
      "Budgets exist on both layers but are DIFFERENT mechanisms: the binary's token budget is substrate provenance; the workbench's child budget is policy enforced before any delegation drive.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "autonomous_mode",
    req_prime_008_class: "autonomous_mode",
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-RUN",
          detail: "--autonomous, --autonomous-gate, --autonomous-*-limits documented",
        }),
        Object.freeze({
          probe_id: "L-AUTONOMOUS-GOAL",
          detail:
            "real run with --autonomous --autonomous-gate 'true' executed (python tool computed 42)",
        }),
      ]),
      detail: "continue-until-gates-pass loop with typed limits; executed for real",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "substrateInventory",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "refused_unsupported_substrate",
      detail:
        "requestSubstrateStrategy('autonomous_mode', ...) refuses; autonomous loops in the workbench remain bounded by configured budgets, never by an admitted substrate loop",
    }),
    provenance_note:
      "The real binary's autonomous loop is exactly the surface REQ-PRIME-008 guards: recorded, never admitted as canonical execution authority.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "schedule",
    req_prime_008_class: "schedule",
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-SCHEDULE",
          detail: "schedule list|add|cancel documented",
        }),
        Object.freeze({
          probe_id: "L-SCHEDULE-ADD",
          detail:
            "real cron job created against an isolated daemon: 'Scheduled <job-id> next=2027-01-01T10:00:00.000Z'",
        }),
        Object.freeze({
          probe_id: "P-SCHEDULE-INVALID",
          detail: "invalid cron refused: 'Cron schedule did not match within one year: 0 0 31 2 *'",
        }),
      ]),
      detail:
        "one-time/recurring prompt scheduling through the background daemon; add executed for real. DRIFT: list/cancel reject --daemon-socket (bound to the default socket) while add accepts it — surface inconsistency recorded, not masked",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "substrateInventory",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "refused_unsupported_substrate",
      detail: "requestSubstrateStrategy('schedule', ...) refuses",
    }),
    provenance_note:
      "A scheduled prompt is a Prime-side execution substrate; it can never constitute a Gauntlet settlement schedule.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "heartbeat",
    req_prime_008_class: "heartbeat",
    real_binary: Object.freeze({
      status: "not_in_probed_cli_surface",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-RUN",
          detail: "no heartbeat token in the full --help corpus",
        }),
      ]),
      detail:
        "no CLI surface in the probed corpus; the installed bundle contains internal heartbeat-schedule helpers (compactHeartbeatSchedule) — internal presence noted, no probed CLI control. Bounded claim: probed surface only",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "substrateInventory",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "refused_unsupported_substrate",
      detail:
        "requestSubstrateStrategy('heartbeat', ...) refuses; heartbeats remain recorded liveness signals that never imply success",
    }),
    provenance_note:
      "T10's liveness-never-completion invariant holds on the real substrate too: the real agent stayed live AND non-complete through a misleading 'done' message (L-MISLEAD-GROWTH).",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "compaction",
    req_prime_008_class: "compaction",
    real_binary: Object.freeze({
      status: "not_in_probed_cli_surface",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-RUN",
          detail: "no compaction token in the probed corpus",
        }),
      ]),
      detail: "no CLI surface in the probed corpus (bounded claim)",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "substrateInventory",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "refused_unsupported_substrate",
      detail: "requestSubstrateStrategy('compaction', ...) refuses",
    }),
    provenance_note: "Unchanged from T05/T10 at every layer probed.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "refinement",
    req_prime_008_class: "refinement",
    real_binary: Object.freeze({
      status: "not_in_probed_cli_surface",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-RUN",
          detail: "no refinement token in the probed corpus",
        }),
      ]),
      detail: "no CLI surface in the probed corpus (bounded claim)",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "substrateInventory",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "refused_unsupported_substrate",
      detail: "requestSubstrateStrategy('refinement', ...) refuses",
    }),
    provenance_note: "Unchanged from T05/T10 at every layer probed.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "harness_state",
    req_prime_008_class: "harness_state",
    real_binary: Object.freeze({
      status: "not_in_probed_cli_surface",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-RUN",
          detail: "no harness-state token in the probed corpus",
        }),
      ]),
      detail:
        "no harness-state control in the probed corpus; the binary's session management (-c/-r/--fork/session export) is recorded under session_resume, not as a REQ-PRIME-008 harness-state control (bounded claim)",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "substrateInventory",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "refused_unsupported_substrate",
      detail: "requestSubstrateStrategy('harness_state', ...) refuses",
    }),
    provenance_note: "Unchanged from T05/T10 at every layer probed.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "child_session_admission",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "L-JSON-SESSION",
          detail:
            "real headless child: --mode json emitted the session record with the real session id + model; session file created under --session-dir",
        }),
        Object.freeze({
          probe_id: "L-AGENT-LIST",
          detail:
            "real daemon agents listed with live identity: id, sessionId (UUID), sessionFile, rlmDepth 0, runtimeKind top-level",
        }),
      ]),
      detail:
        "real child processes with real session UUIDs minted by Prime itself; the parent/child EDGE among them is workbench-recorded (no third-party parent binding exists in the probed surface)",
    }),
    adapter_model: Object.freeze({
      status: "exposed",
      source: "delegationCapabilityFacts",
      reason: null,
    }),
    workbench: Object.freeze({
      admission: "admitted_via_canonical_path",
      detail:
        "gateway-gated PrimeDelegationDispatcher: canonical accepted receipt BEFORE child spawn; the handle is never the result (REQ-PRIME-003)",
    }),
    provenance_note:
      "The one feature where all three layers agree — and the battery proves the composed path on real processes (fresh-target leg a).",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "mailbox_communication",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-SEND",
          detail: "prime-agent send <agent> <message> documented",
        }),
        Object.freeze({
          probe_id: "L-SEND-RECEIPT",
          detail:
            "real delivery receipt: agentmsg_<uuid>, deliveryStatus 'delivered', target activeSessionId/sessionId",
        }),
      ]),
      detail: "message delivery to a live agent executed for real",
    }),
    adapter_model: Object.freeze({
      status: "exposed",
      source: "delegationCapabilityFacts",
      reason: null,
    }),
    workbench: Object.freeze({
      admission: "admitted_via_canonical_path",
      detail:
        "deliverOperatorMessage delivers as kind 'communication' with establishes_completion LITERALLY false (REQ-PRIME-005)",
    }),
    provenance_note:
      "Communication on every layer; the real-substrate leg (misleading 'done' + continued liveness) shows the BINARY also refuses to complete on message content alone — the contract assumption holds on the real substrate.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "mailbox_delivery_mode_control",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "documented_but_refused",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-SEND",
          detail: "send --help documents --steer and --follow-up",
        }),
        Object.freeze({
          probe_id: "P-SEND-REFUSED",
          detail:
            "executed: prime-agent send <agent> --follow-up -- 'msg' → 'Error: Unknown option for send: --follow-up'",
        }),
      ]),
      detail:
        "HELP/IMPLEMENTATION DRIFT in the installed build: the documented delivery-mode options are refused by the argument parser. Recorded as found — the surface is documented, the execution refuses",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "PrimeWorkbenchHost_surface",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "delivered_immediately_unsupported_mode",
      detail:
        "OperatorMessagePlan.delivery_mode_control is the literal 'unsupported_delivered_immediately'",
    }),
    provenance_note:
      "Fresh-target finding: the workbench's honest 'delivered immediately' matches what the binary's implementation actually does — the documented modes do not execute.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "result_contract_completion",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "not_in_probed_cli_surface",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-RUN",
          detail: "no result-contract token in the probed corpus",
        }),
      ]),
      detail:
        "completion-by-contract is the GAUNTLET-side boundary: no result-contract surface in the probed CLI corpus (bounded claim about the probed surface, not about internal session state)",
    }),
    adapter_model: Object.freeze({
      status: "exposed",
      source: "delegationCapabilityFacts",
      reason: null,
    }),
    workbench: Object.freeze({
      admission: "admitted_via_canonical_path",
      detail:
        "completionStatus derives complete ONLY from resultContract.schema_valid; basis is otherwise 'none'",
    }),
    provenance_note:
      "Completion remains a Gauntlet contract on every layer — the real binary cannot complete a unit by existing, messaging, or staying live.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "session_stop",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({ probe_id: "P-HELP-STOP", detail: "prime-agent stop <agent> documented" }),
        Object.freeze({
          probe_id: "L-STOP",
          detail: "real stop executed: 'ok', agent removed from the live list",
        }),
      ]),
      detail: "the SUBSTRATE has real termination; executed for real",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "PrimeHostAdapter_surface",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "no_canonical_stop_registered",
      detail:
        "canonicalStopAvailability(): the canonical surface serves no cancellation/stop (REQ-OPS-003); detach issues cancel_calls_issued LITERALLY 0 (REQ-PRIME-009)",
    }),
    provenance_note:
      "The sharpest three-way split: the substrate can stop work, the adapter model has no such call, and the workbench admits no operator termination until a canonical stop exists. T24 changes none of that.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "session_attach",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "exposed_probed",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-ATTACH",
          detail: "prime-agent attach <agent> documented",
        }),
        Object.freeze({
          probe_id: "L-SURVIVAL",
          detail:
            "a daemon agent stayed live after its creating TUI process was killed, then accepted delivery from a fresh client",
        }),
      ]),
      detail:
        "attach itself needs an interactive terminal (not exercised headless); the underlying detach-survival + fresh-client reattachment WAS exercised: the agent outlived its TUI",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "PrimeWorkbenchHost_surface",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "reattach_from_qualified_records_only",
      detail:
        "PrimeWorkbenchHost.detach is read-only projection; reattachToPrime rebuilds from qualified records, never host memory (REQ-PRIME-010)",
    }),
    provenance_note:
      "The real daemon is the substrate counterpart of REQ-VERIFY-005: work survives host loss, and a fresh client reads authoritative state.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "session_list",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-LIST",
          detail: "prime-agent list [--all] [--json] documented",
        }),
        Object.freeze({
          probe_id: "L-AGENT-LIST",
          detail: "live agent inventory read from a fresh process",
        }),
      ]),
      detail:
        "authoritative agent inventory executed for real (--json: id, sessionId, sessionFile, lifecycle, activity, rlmDepth)",
    }),
    adapter_model: Object.freeze({
      status: "exposed",
      source: "PrimeHostAdapter_surface",
      reason: null,
    }),
    workbench: Object.freeze({
      admission: "reattach_from_qualified_records_only",
      detail:
        "projectHostToSessionRecords projects the adapter's real sessions; the battery projects the BINARY's list --json the same way — records, not screens",
    }),
    provenance_note:
      "List output is a qualified Prime record source for reattachment (REQ-PRIME-010) — read, never mutated.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
  Object.freeze({
    feature: "session_resume",
    req_prime_008_class: null,
    real_binary: Object.freeze({
      status: "exposed_executed",
      probes: Object.freeze([
        Object.freeze({
          probe_id: "P-HELP-RUN",
          detail: "-c/--continue, -r/--resume [path|id], --fork <path|id> documented",
        }),
        Object.freeze({
          probe_id: "L-CONTINUE",
          detail:
            "fresh real process with -c resumed the prior session and answered '42' from its recorded context",
        }),
      ]),
      detail:
        "cross-process session continuity through Prime's OWN saved records, executed for real",
    }),
    adapter_model: Object.freeze({
      status: "not_exposed",
      source: "PrimeHostAdapter_surface",
      reason: MODEL_UNSUPPORTED,
    }),
    workbench: Object.freeze({
      admission: "reattach_from_qualified_records_only",
      detail:
        "the in-model adapter reconstructs read-only topology from exported records only; it cannot resume a session it does not know (T10 informational, re-proven fresh)",
    }),
    provenance_note:
      "The T10 cross-process boundary is REAL-substrate-true for the binary (records-based resume works) and still true for the in-model adapter (nothing crosses a process without records). Both recorded; neither masked.",
    canonical_direction: false,
    canonical_settlement: false,
    canonical_capability: false,
  }),
]);

/** The map, in declared order (deterministic). */
export function substrateProvenanceMap(): readonly SubstrateProvenanceRow[] {
  return SUBSTRATE_PROVENANCE_MAP;
}

/** One row by feature id, or null when the id is not mapped. */
export function substrateProvenanceRow(feature: SubstrateFeatureId): SubstrateProvenanceRow | null {
  return SUBSTRATE_PROVENANCE_MAP.find((row) => row.feature === feature) ?? null;
}

/** All REQ-PRIME-008 substrate kinds that have a mapped row. */
export function reqPrime008MappedKinds(): readonly string[] {
  return SUBSTRATE_PROVENANCE_MAP.flatMap((row) =>
    row.req_prime_008_class === null ? [] : [row.req_prime_008_class],
  );
}

// --- Live probe helper (gated use only) -----------------------------------------

export interface PrimeSurfaceProbe {
  readonly version: string;
  readonly help: string;
  /** The surface tokens whose presence the map's exposed answers depend on. */
  readonly tokens: Readonly<Record<string, boolean>>;
}

const SURFACE_TOKENS: readonly string[] = Object.freeze([
  "--goal",
  "--goal-token-budget",
  "--autonomous",
  "--autonomous-gate",
  "-c, --continue",
  "-r, --resume",
  "--fork",
  "schedule",
  "send",
  "stop",
  "attach",
  "list",
]);

/**
 * Re-run the deterministic surface probes against the installed binary. Used by
 * the gated battery to verify the qualified identity before any lifecycle step;
 * spawns two short-lived processes (`--version`, `--help`) and nothing else.
 */
export async function probePrimeSurface(
  binary: string = QUALIFIED_PRIME_BINARY.binary,
): Promise<PrimeSurfaceProbe> {
  const run = async (args: readonly string[]): Promise<string> => {
    const proc = Bun.spawn([binary, ...args], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const [out, err, code] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ]);
    if (code !== 0) {
      throw new Error(`probe '${args.join(" ")}' exited ${code}: ${err.slice(0, 400)}`);
    }
    // The installed launcher writes --version (and, depending on version,
    // other banner output) to STDERR; stdout and stderr are both probe output.
    return out.length > 0 ? out : err;
  };
  const version = (await run(["--version"])).trim();
  const help = await run(["--help"]);
  const tokens: Record<string, boolean> = {};
  for (const token of SURFACE_TOKENS) {
    tokens[token] = help.includes(token);
  }
  return Object.freeze({
    version,
    help,
    tokens: Object.freeze(tokens),
  });
}

/**
 * The qualification decision for running the gated battery on this host:
 * binary present, version matches the qualified identity, and every surface
 * token the map's `exposed` answers rely on is present. A mismatch is a
 * requalification signal (prereg R1 direction) — record honestly, never mask.
 */
export async function qualifyPrimeBinary(): Promise<
  { qualified: true; probe: PrimeSurfaceProbe } | { qualified: false; detail: string }
> {
  let probe: PrimeSurfaceProbe;
  try {
    probe = await probePrimeSurface();
  } catch (error) {
    return {
      qualified: false,
      detail: `prime binary not runnable on this host: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
  if (probe.version !== QUALIFIED_PRIME_BINARY.version) {
    return {
      qualified: false,
      detail: `installed prime-agent version '${probe.version}' != qualified '${QUALIFIED_PRIME_BINARY.version}'; requalification required before the lifecycle battery may run`,
    };
  }
  const missing = SURFACE_TOKENS.filter((token) => !probe.tokens[token]);
  if (missing.length > 0) {
    return {
      qualified: false,
      detail: `probed --help surface is missing expected tokens: ${missing.join(", ")}`,
    };
  }
  return { qualified: true, probe };
}
