import { describe, expect, it } from "bun:test";
import type { AffordanceObservation } from "./affordance";
import {
  parseSemanticEnvelopeDocument,
  projectSemanticEnrichment,
  type SemanticEnrichment,
} from "./domainforge-adapter";
import { FocusRef } from "./focus";
import { type InspectorSection, publishLensData, renderInspector } from "./inspector";
import type { ShellSession } from "./shell";
import type { SxrLensProjection } from "./sxr-adapter";
import {
  FULL_DIMS,
  modelWiredSession,
  observationFor,
  seededModel,
  shellText,
} from "./testing/shell-harness";

/**
 * T20 Inspector LENS WIRING suites (T14/T14B obligation discharged here).
 *
 * T14's `renderSemanticSection` and T14B's `renderSxrEvidenceSection`
 * produced InspectorSection records that were never appended to the live
 * `renderInspector` section list. The wiring is DATA-SOURCE-GATED: a section
 * appears only when the snapshot CARRIES the projected lens data (published
 * via `publishLensData`, the read-only counterpart of T13's
 * `publishTerminalProvider`); when no lens is carried, no section is
 * fabricated — the pre-existing section list is unchanged.
 *
 * DSN-008 record shapes stay pinned by T12's opentui-shell suites and the
 * T14B composition matrix; these suites pin the WIRING only.
 */

const FOCUS = FocusRef.for("gauntlet", "WorkItem", "u-root");

/** An absent-state enrichment (exactly what a host that probed nothing publishes). */
const ABSENT_ENRICHMENT: SemanticEnrichment = Object.freeze({
  status: "absent" as const,
  reason: "no_model_output" as const,
  detail: "no DomainForge envelope output was provided for this Case",
});

const ABSENT_LENS: SxrLensProjection = Object.freeze({
  status: "absent" as const,
  reason: "no_ledger_export" as const,
  detail: "no SXR ledger export was provided for this Case",
});

/**
 * A minimal PROJECTED lens: the honest empty projection of a repo export with
 * zero rows (shape-true, no fabricated ledger content).
 */
const PROJECTED_EMPTY_LENS: SxrLensProjection = Object.freeze({
  status: "projected" as const,
  repo_id: "01T20WIRINGREPO",
  questions: Object.freeze([]),
  thresholds: Object.freeze([]),
  snapshots: Object.freeze([]),
  runs: Object.freeze([]),
  evidence: Object.freeze([]),
  settlements: Object.freeze([]),
  uninterpreted: Object.freeze([]),
  rows: Object.freeze([]),
});

/** A bound enrichment from the REAL pinned DomainForge capture (same bytes T14B pins). */
function boundEnrichmentFromRealCapture(): SemanticEnrichment {
  const parse = parseSemanticEnvelopeDocument(REAL_DF_DOCUMENT);
  if (parse.status !== "parsed") {
    throw new Error("the pinned DF fixture must parse");
  }
  return projectSemanticEnrichment(parse);
}

const OBSERVATION_SEED: AffordanceObservation = observationFor(seededModel());

function sectionIds(sections: readonly InspectorSection[]): string[] {
  return sections.map((section) => section.id);
}

describe("T20 inspector lens wiring: the gate", () => {
  it("without published lens data the section list is exactly the pre-existing one", () => {
    const model = seededModel();
    const snapshot = model.getSnapshot();
    const result = renderInspector({ snapshot, focus: FOCUS, observation: OBSERVATION_SEED });
    const ids = sectionIds(result.sections);
    expect(ids).not.toContain("semantic");
    expect(ids).not.toContain("sxr-evidence");
    // The unwired baseline order is intact.
    expect(ids).toEqual([
      "identity",
      "state",
      "relations",
      "authority",
      "evidence",
      "provenance",
      "affordances",
    ]);
    model.close();
  });

  it("published ABSENT lens data renders the typed absent sections at their wired position", () => {
    const model = seededModel();
    const published = publishLensData(model.getSnapshot(), {
      semantic: ABSENT_ENRICHMENT,
      sxr_lens: ABSENT_LENS,
    });
    const result = renderInspector({
      snapshot: published,
      focus: FOCUS,
      observation: OBSERVATION_SEED,
    });
    const ids = sectionIds(result.sections);
    expect(ids).toContain("semantic");
    expect(ids).toContain("sxr-evidence");
    // Wired between the provenance context and the affordance actions.
    expect(ids.indexOf("provenance")).toBeLessThan(ids.indexOf("semantic"));
    expect(ids.indexOf("semantic")).toBeLessThan(ids.indexOf("sxr-evidence"));
    expect(ids.indexOf("sxr-evidence")).toBeLessThan(ids.indexOf("affordances"));

    const semantic = result.sections.find((section) => section.id === "semantic");
    const reality = result.sections.find((section) => section.id === "sxr-evidence");
    const flat = (section: InspectorSection | undefined): string =>
      (section?.lines ?? []).map((line) => line.spans.map((span) => span.text).join("")).join("\n");
    expect(flat(semantic)).toContain("ABSENT (no_model_output)");
    expect(semantic?.records).toEqual([]);
    expect(flat(reality)).toContain("ABSENT (no_ledger_export)");
    expect(flat(reality)).toContain("not recreated");
    expect(reality?.records).toEqual([]);
    model.close();
  });

  it("a projected lens reaches the live section list with its records (DSN-008 shapes intact)", () => {
    const model = seededModel();
    const published = publishLensData(model.getSnapshot(), {
      sxr_lens: PROJECTED_EMPTY_LENS,
    });
    const result = renderInspector({
      snapshot: published,
      focus: FOCUS,
      observation: OBSERVATION_SEED,
    });
    const reality = result.sections.find((section) => section.id === "sxr-evidence");
    expect(reality).toBeDefined();
    const flat = (reality?.lines ?? [])
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(flat).toContain("01T20WIRINGREPO");
    expect(flat).toContain("0 committed ledger record(s)");
    // No semantic lens published: that section stays absent.
    expect(sectionIds(result.sections)).not.toContain("semantic");
    model.close();
  });

  it("a bound semantic enrichment renders its model identity and concept records in the live list", () => {
    const model = seededModel();
    const published = publishLensData(model.getSnapshot(), {
      semantic: boundEnrichmentFromRealCapture(),
    });
    const result = renderInspector({
      snapshot: published,
      focus: FOCUS,
      observation: OBSERVATION_SEED,
    });
    const semantic = result.sections.find((section) => section.id === "semantic");
    expect(semantic).toBeDefined();
    const flat = (semantic?.lines ?? [])
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(flat).not.toContain("ABSENT");
    expect(flat).toContain("domainforge-core");
    expect(flat).toContain("User");
    expect((semantic?.records ?? []).length).toBeGreaterThan(0);
    model.close();
  });

  it("canonical fields survive publication byte-identically (publish adds own properties only)", () => {
    const model = seededModel();
    const snapshot = model.getSnapshot();
    const published = publishLensData(snapshot, {
      semantic: ABSENT_ENRICHMENT,
      sxr_lens: ABSENT_LENS,
    });
    expect(published.semantic).toBe(ABSENT_ENRICHMENT);
    expect(published.sxr_lens).toBe(ABSENT_LENS);
    expect(published.units).toEqual(snapshot.units);
    expect(published.generation).toBe(snapshot.generation);
    model.close();
  });
});

describe("T20 inspector lens wiring: the live shell path", () => {
  it("the shell's Inspector surface renders the wired sections only when the snapshot carries them", () => {
    const model = seededModel();
    const session: ShellSession = modelWiredSession(model, FULL_DIMS);
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.selectFocus(focus);

    // Without publication: no SEMANTIC / REALITY TRACE headings anywhere.
    const bare = session.render(model.getSnapshot(), {
      observation: observationFor(model),
    });
    const bareText = shellText(bare, "Inspector").join("\n");
    expect(bareText).not.toContain("SEMANTIC");
    expect(bareText).not.toContain("REALITY TRACE");

    // With publication: both lens headings render in the live Inspector.
    const published = publishLensData(model.getSnapshot(), {
      semantic: boundEnrichmentFromRealCapture(),
      sxr_lens: PROJECTED_EMPTY_LENS,
    });
    const wired = session.render(published, { observation: observationFor(model) });
    const wiredText = shellText(wired, "Inspector").join("\n");
    expect(wiredText).toContain("SEMANTIC");
    expect(wiredText).toContain("domainforge-core");
    expect(wiredText).toContain("REALITY TRACE");
    expect(wiredText).toContain("01T20WIRINGREPO");
    model.close();
  });
});

const REAL_DF_DOCUMENT =
  '{"envelope":{"application_contract":{"entities":[],"enums":[],"operations":[],"records":[]},"canonicalization_version":"domainforge-canonicalization/v1","compiler_interpretation_version":"domainforge-interpretation/v1","import_graph":[],"language_schema_version":"domainforge-ast/v3","modules":[{"logical_id":"valid-basic.sea","namespace":"auth","semantic_content_hash":"sha256:8493cc0b18621b8721cb736c7f7e8d7a2376e2f095e15ff350c5f0bbb12b7e29"}],"namespace_bindings":[{"logical_id":"valid-basic.sea","namespace":"auth"}],"resolved_references":[],"semantic_declarations":[{"declaration":{"data":{"annotations":{},"domain":"auth","name":"User"},"kind":"entity"},"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Role"},"kind":"entity"},"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"domain":"auth","name":"Token","unit":{"data":{"id":{"data":{"id":"7a50d9d2-89e2-5607-bca5-34464284bea8"},"kind":"concept"}},"kind":"declaration"}},"kind":"resource"},"id":{"data":{"id":"a81c1fec-c10a-5541-bd81-98c64b10f402"},"kind":"concept"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"72f70c63-6f3e-5061-8854-42e540bbac87"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:0054e494b25408165361fa7d0fb1e090a7aee22d7dce2c9f188bc87e24451543","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"},{"declaration":{"data":{"annotations":{},"from_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"},"quantity":"1","resource":{"data":{"id":{"data":{"id":"a9034d4e-9276-56a8-8543-9ee845011958"},"kind":"concept"}},"kind":"declaration"},"to_entity":{"data":{"id":{"data":{"id":"5ad6a959-5924-5cf9-af04-7b502ea74784"},"kind":"concept"}},"kind":"declaration"}},"kind":"flow"},"id":{"data":{"content_hash":"sha256:e2f61281af6ce70dd96268e342cb31a0832af79be550799152f986dd2fd1af66","duplicate_index":0,"kind":"flow"},"kind":"occurrence"},"logical_module_id":"valid-basic.sea"}],"semantic_packs":[]},"inputs":{"interpretation_version":"domainforge-interpretation/v1","language_schema_version":"domainforge-ast/v3","semantic_pack_set_hash":"sha256:6e5312099a8d89e1d271bd89d9fcb36b031069bd8dad72be397af6b331d85c40","source_set_hash":"sha256:199407284bab273208ccea52deb56197c582816a668ee726d7ab1ab3797d42b5"},"producer":{"name":"domainforge-core","version":"0.16.0"},"schema_version":"domainforge-semantic-envelope/v1","self_hash":"sha256:1641b0d368b1f26eb6f3768a71d14b7f0cf697b4419204d65ed1911c515ef97e","semantic_closure_hash":"sha256:1a1b7053205bba72249ad04a199a216392cc803f850e171b8e3afcc945be786c"}';
