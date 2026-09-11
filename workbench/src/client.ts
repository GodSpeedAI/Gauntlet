/**
 * Read-only HTTP client for `gauntlet serve` operator surface.
 * Conforms to spec §20.2 and plan T00A read requirements.
 *
 * Exposes ONLY read operations (state, runs, units, escalated).
 * Emits zero mutations. Handles connection refusal or missing data gracefully.
 */

export interface UnitSnapshot {
  readonly unit_id: string;
  readonly status: string;
  readonly tier?: string | undefined;
  readonly iteration?: number | undefined;
}

export interface VerificationFact {
  readonly unit_id: string;
  readonly outcome: "pass" | "fail" | "inconclusive" | string;
  readonly evidence: string;
  readonly verification: string;
}

export interface SettlementFact {
  readonly unit_id: string;
  readonly verification: string;
  readonly evidence: string;
}

export interface BudgetSpentFacts {
  readonly rounds: number;
  readonly wall_seconds: number;
  readonly tokens: number;
  readonly cost_usd_micro: number;
}

export interface RuntimeSnapshot {
  readonly run: string | null;
  readonly escalated: readonly string[];
  readonly units: readonly UnitSnapshot[];
  readonly active?: readonly string[] | undefined;
  readonly budget_remaining?: {
    readonly usd_micro: number;
    readonly tokens: number;
  } | null;
  readonly run_derivation?: string | undefined;
  readonly fetched_at: string;
  readonly status: "connected" | "disconnected" | "empty";
}

export interface RunView {
  readonly run_id: string;
  readonly iteration: number | null;
  readonly units: readonly UnitSnapshot[];
  readonly open_tickets: readonly string[];
  readonly verifications: readonly VerificationFact[];
  readonly settlements: readonly SettlementFact[];
  readonly budget_spent: BudgetSpentFacts | null;
  readonly fetched_at: string;
}

export interface UnitView {
  readonly unit_id: string;
  readonly unit: UnitSnapshot | null;
  readonly verifications: readonly VerificationFact[];
  readonly settled: boolean;
  readonly denial_reason: string;
  readonly fetched_at: string;
}

export interface CapabilitiesResponse {
  readonly protocol_version: string;
  readonly server_version: string;
  readonly capabilities: readonly string[];
  readonly unsupported_operations: readonly string[];
}

export interface EvidenceRequiredFact {
  readonly kind: string;
  readonly locator: string;
  readonly producer: string;
}

export interface EscalationFact {
  readonly ticket_id: string;
  readonly run_id: string;
  readonly unit_id: string | null;
  readonly reason: string;
  readonly status: string;
}

export interface UnitInspectionView {
  readonly unit_id: string;
  readonly tier: string | null;
  readonly claim: string | null;
  readonly authority: string | null;
  readonly blast_radius: string | null;
  readonly verify_by: string | null;
  readonly evidence_required: EvidenceRequiredFact | null;
  readonly depends_on: readonly string[];
  readonly status: string | null;
  readonly settled: boolean;
  readonly verifications: readonly VerificationFact[];
  readonly escalation: EscalationFact | null;
  readonly evidence_content: string | null;
  readonly precondition_explanation: string | null;
  readonly fetched_at: string;
}

export interface MutationResult {
  readonly ok: boolean;
  readonly status: number;
  readonly code?: string | undefined;
  readonly message?: string | undefined;
  readonly data?: unknown;
}

export class GauntletReadClient {
  private readonly baseUrl: string;

  constructor(bindAddress = "127.0.0.1:4000") {
    const url =
      bindAddress.startsWith("http://") || bindAddress.startsWith("https://")
        ? bindAddress
        : `http://${bindAddress}`;
    this.baseUrl = url.replace(/\/+$/, "");
  }

  /**
   * Fetch current runtime snapshot (`GET /api/v1/state`).
   */
  async fetchState(): Promise<RuntimeSnapshot> {
    try {
      const res = await fetch(`${this.baseUrl}/api/v1/state`, {
        signal: AbortSignal.timeout(5000),
        method: "GET",
        headers: { Accept: "application/json" },
      });

      if (!res.ok) {
        return this.createEmptySnapshot("disconnected");
      }

      const data = (await res.json()) as Record<string, unknown>;
      const run = typeof data["run"] === "string" ? data["run"] : null;
      const escalated = Array.isArray(data["escalated"])
        ? data["escalated"].filter((x): x is string => typeof x === "string")
        : [];
      if (
        !Array.isArray(data["units"]) ||
        !(data["run"] === null || typeof data["run"] === "string")
      ) {
        return this.createEmptySnapshot("disconnected");
      }
      const unitsRaw = data["units"];
      const units: UnitSnapshot[] = unitsRaw.map((u: unknown) => {
        if (typeof u !== "object" || u === null) throw new Error("invalid unit");
        const item = u as Record<string, unknown>;
        const id = item["id"] ?? item["unit_id"];
        if (typeof id !== "string" || id.length === 0 || typeof item["status"] !== "string")
          throw new Error("invalid unit");
        return {
          unit_id: id,
          status: item["status"],
          tier: typeof item["tier"] === "string" ? item["tier"] : undefined,
          iteration: typeof item["iteration"] === "number" ? item["iteration"] : undefined,
        };
      });

      return {
        run,
        escalated,
        units,
        run_derivation:
          typeof data["run_derivation"] === "string" ? data["run_derivation"] : undefined,
        fetched_at: new Date().toISOString(),
        status: run || units.length > 0 ? "connected" : "empty",
      };
    } catch {
      return this.createEmptySnapshot("disconnected");
    }
  }

  /**
   * Fetch detailed view for a single run (`GET /api/v1/runs/{id}`).
   */
  async fetchRun(runId: string): Promise<RunView | null> {
    try {
      const res = await fetch(`${this.baseUrl}/api/v1/runs/${encodeURIComponent(runId)}`, {
        method: "GET",
        headers: { Accept: "application/json" },
      });

      if (!res.ok) {
        return null;
      }

      const data = (await res.json()) as Record<string, unknown>;
      const unitsRaw = Array.isArray(data["units"]) ? data["units"] : [];
      const units: UnitSnapshot[] = unitsRaw.map((u: unknown) => {
        const item = u as Record<string, unknown>;
        return {
          unit_id: String(item["unit_id"] ?? "unknown"),
          status: (item["status"] as UnitSnapshot["status"]) ?? "pending",
          tier: typeof item["tier"] === "string" ? item["tier"] : undefined,
        };
      });

      const verificationsRaw = Array.isArray(data["verifications"]) ? data["verifications"] : [];
      const verifications: VerificationFact[] = verificationsRaw.map((v: unknown) => {
        const item = v as Record<string, unknown>;
        return {
          unit_id: String(item["unit_id"] ?? ""),
          outcome: String(item["outcome"] ?? "inconclusive"),
          evidence: String(item["evidence"] ?? ""),
          verification: String(item["verification"] ?? ""),
        };
      });

      const settlementsRaw = Array.isArray(data["settlements"]) ? data["settlements"] : [];
      const settlements: SettlementFact[] = settlementsRaw.map((s: unknown) => {
        const item = s as Record<string, unknown>;
        return {
          unit_id: String(item["unit_id"] ?? ""),
          verification: String(item["verification"] ?? ""),
          evidence: String(item["evidence"] ?? ""),
        };
      });

      const openTickets = Array.isArray(data["open_tickets"])
        ? data["open_tickets"].filter((x): x is string => typeof x === "string")
        : [];

      return {
        run_id: runId,
        iteration: typeof data["iteration"] === "number" ? data["iteration"] : null,
        units,
        open_tickets: openTickets,
        verifications,
        settlements,
        budget_spent: null,
        fetched_at: new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  /**
   * Fetch detailed view for a single unit (`GET /api/v1/units/{id}`).
   */
  async fetchUnit(unitId: string): Promise<UnitView | null> {
    try {
      const res = await fetch(`${this.baseUrl}/api/v1/units/${encodeURIComponent(unitId)}`, {
        method: "GET",
        headers: { Accept: "application/json" },
      });

      if (!res.ok) {
        return null;
      }

      const data = (await res.json()) as Record<string, unknown>;
      const unitObj = data["unit"] as Record<string, unknown> | null;
      const unit: UnitSnapshot | null = unitObj
        ? {
            unit_id: String(unitObj["unit_id"] ?? unitId),
            status: (unitObj["status"] as UnitSnapshot["status"]) ?? "pending",
            tier: typeof unitObj["tier"] === "string" ? unitObj["tier"] : undefined,
          }
        : null;

      const verificationsRaw = Array.isArray(data["verifications"]) ? data["verifications"] : [];
      const verifications: VerificationFact[] = verificationsRaw.map((v: unknown) => {
        const item = v as Record<string, unknown>;
        return {
          unit_id: String(item["unit_id"] ?? unitId),
          outcome: String(item["outcome"] ?? "inconclusive"),
          evidence: String(item["evidence"] ?? ""),
          verification: String(item["verification"] ?? ""),
        };
      });

      return {
        unit_id: unitId,
        unit,
        verifications,
        settled: Boolean(data["settled"]),
        denial_reason: "unavailable (T01 read contract required)",
        fetched_at: new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  /**
   * Fetch server capabilities handshake (`GET /api/v1/capabilities`).
   */
  async fetchCapabilities(): Promise<CapabilitiesResponse | null> {
    try {
      const res = await fetch(`${this.baseUrl}/api/v1/capabilities`, {
        signal: AbortSignal.timeout(5000),
        method: "GET",
        headers: { Accept: "application/json" },
      });

      if (!res.ok) {
        return null;
      }

      const data = (await res.json()) as Record<string, unknown>;
      return {
        protocol_version: String(data["protocol_version"] ?? "1.0.0"),
        server_version: String(data["server_version"] ?? "unknown"),
        capabilities: Array.isArray(data["capabilities"])
          ? data["capabilities"].filter((x): x is string => typeof x === "string")
          : [],
        unsupported_operations: Array.isArray(data["unsupported_operations"])
          ? data["unsupported_operations"].filter((x): x is string => typeof x === "string")
          : [],
      };
    } catch {
      return null;
    }
  }

  /**
   * Fetch deep unit inspection (`GET /api/v1/units/{id}/inspection`).
   * Spec §20.2, REQ-STATE-001, REQ-STATE-005.
   */
  async fetchInspection(unitId: string, runId?: string): Promise<UnitInspectionView | null> {
    try {
      const query = runId ? `?run=${encodeURIComponent(runId)}` : "";
      const res = await fetch(
        `${this.baseUrl}/api/v1/units/${encodeURIComponent(unitId)}/inspection${query}`,
        {
          method: "GET",
          signal: AbortSignal.timeout(5000),
          headers: { Accept: "application/json" },
        },
      );

      if (!res.ok) {
        return null;
      }

      const data = (await res.json()) as Record<string, unknown>;
      const evidenceRaw = data["evidence_required"] as Record<string, unknown> | null;
      const evidenceRequired: EvidenceRequiredFact | null = evidenceRaw
        ? {
            kind: String(evidenceRaw["kind"] ?? ""),
            locator: String(evidenceRaw["locator"] ?? ""),
            producer: String(evidenceRaw["producer"] ?? ""),
          }
        : null;

      const escalationRaw = data["escalation"] as Record<string, unknown> | null;
      const escalation: EscalationFact | null = escalationRaw
        ? {
            ticket_id: String(escalationRaw["ticket_id"] ?? ""),
            run_id: String(escalationRaw["run_id"] ?? ""),
            unit_id: typeof escalationRaw["unit_id"] === "string" ? escalationRaw["unit_id"] : null,
            reason: String(escalationRaw["reason"] ?? ""),
            status: String(escalationRaw["status"] ?? ""),
          }
        : null;

      const verificationsRaw = Array.isArray(data["verifications"]) ? data["verifications"] : [];
      const verifications: VerificationFact[] = verificationsRaw.map((v: unknown) => {
        const item = v as Record<string, unknown>;
        return {
          unit_id: String(item["unit_id"] ?? unitId),
          outcome: String(item["outcome"] ?? "inconclusive"),
          evidence: String(item["evidence"] ?? ""),
          verification: String(item["verification"] ?? ""),
        };
      });

      return {
        unit_id: String(data["unit_id"] ?? unitId),
        tier: typeof data["tier"] === "string" ? data["tier"] : null,
        claim: typeof data["claim"] === "string" ? data["claim"] : null,
        authority: typeof data["authority"] === "string" ? data["authority"] : null,
        blast_radius: typeof data["blast_radius"] === "string" ? data["blast_radius"] : null,
        verify_by: typeof data["verify_by"] === "string" ? data["verify_by"] : null,
        evidence_required: evidenceRequired,
        depends_on: Array.isArray(data["depends_on"])
          ? data["depends_on"].filter((x): x is string => typeof x === "string")
          : [],
        status: typeof data["status"] === "string" ? data["status"] : null,
        settled: Boolean(data["settled"]),
        verifications,
        escalation,
        evidence_content:
          typeof data["evidence_content"] === "string" ? data["evidence_content"] : null,
        precondition_explanation:
          typeof data["precondition_explanation"] === "string"
            ? data["precondition_explanation"]
            : null,
        fetched_at: new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  /**
   * Submit an operator mutation intent (`POST /api/v1/units/{id}/resolve` or `retry`).
   * Tests operation boundary authentication and actor validation (REQ-OPS-003, D1, Attack 1).
   */
  async mutateUnit(
    unitId: string,
    action: "resolve" | "retry",
    options?: {
      operator?: string;
      token?: string;
      note?: string;
    },
  ): Promise<MutationResult> {
    try {
      const headers: Record<string, string> = {
        Accept: "application/json",
        "Content-Type": "text/plain",
      };
      if (options?.operator !== undefined) {
        headers["X-Gauntlet-Operator"] = options.operator;
      }
      if (options?.token !== undefined) {
        headers["Authorization"] = `Bearer ${options.token}`;
      }

      const res = await fetch(
        `${this.baseUrl}/api/v1/units/${encodeURIComponent(unitId)}/${action}`,
        {
          method: "POST",
          headers,
          body: options?.note ?? "",
        },
      );

      let data: unknown = null;
      let code: string | undefined;
      let message: string | undefined;
      try {
        data = await res.json();
        if (typeof data === "object" && data !== null) {
          const rec = data as Record<string, unknown>;
          if (typeof rec["code"] === "string") code = rec["code"];
          if (typeof rec["message"] === "string") message = rec["message"];
        }
      } catch {
        // body might not be JSON
      }

      return {
        ok: res.ok,
        status: res.status,
        code,
        message,
        data,
      };
    } catch (err) {
      return {
        ok: false,
        status: 0,
        message: err instanceof Error ? err.message : String(err),
      };
    }
  }

  private createEmptySnapshot(status: "disconnected" | "empty"): RuntimeSnapshot {
    return {
      run: null,
      escalated: [],
      units: [],
      fetched_at: new Date().toISOString(),
      status,
    };
  }
}
