# Gauntlet Project Domain Model Layout

Load this reference when creating, restructuring, composing, or materially changing the project-level DomainForge domain model — not for one-off syntax questions (`write-in-sea.md`) or CLI flag lookups (`cli-commands.md`).

This is a Gauntlet-local convention layered on top of DomainForge's real SEA substrate. It does not change DomainForge itself, and every mechanic described below (imports, `export`, `ConceptChange`, semantic packs) is a verified DomainForge capability, not an invention for this convention.

## Existing projects

If a project already has a governed `.sea` layout — even a single-file `.sea/domain.sea`, or a different directory shape — do not silently reorganize it to match this convention. Surface the difference (what exists, what this convention would change) and get migration authorized through the active Gauntlet process before touching files. This convention is for a new Gauntlet/DomainForge integration, or one explicitly asked to migrate.

## Filesystem layout

```text
<project-root>/
├── .sea/
│   ├── domain-core.sea
│   ├── domain-<subdomain>.sea
│   └── ...
│
└── .agents/
    └── reports/
        └── domain-model/
            ├── FORMALIZATION.md
            └── MODEL-MANIFEST.yaml
```

`.sea/` is the authoritative, project-root-relative home for the formal model. `.agents/reports/domain-model/` holds derived, developmental evidence about that model — never an alternative copy of it.

Do not require multiple subdomain files when the domain has not earned decomposition. A project with one small, coherent domain keeps everything in `domain-core.sea` alone; `domain-<subdomain>.sea` files appear only once a real boundary exists (see below).

## `domain-core.sea`

The authoritative aggregate entry point. Two responsibilities only:

1. hold the small, stable cross-domain kernel — concepts, identities, invariants, dimensions/units, cross-domain relationships, or aggregate policies whose meaning is genuinely shared across bounded domains;
2. import and compose whatever `domain-<subdomain>.sea` modules currently constitute the model.

It must not become a flattened copy of every subdomain, and must not become a junk drawer for a concept merely because more than one module currently mentions it. A concept belongs in core only when evidence shows its meaning is genuinely shared across domain boundaries and changing it would affect multiple bounded domain representations. Core changes should be relatively rare — a churny core is a sign concepts landed there that should have stayed local to one subdomain.

## `domain-<subdomain>.sea`

Bounded domain representations, one file per earned boundary:

```text
.sea/domain-customer.sea
.sea/domain-payments.sea
.sea/domain-fulfillment.sea
```

Create a subdomain module only when a real domain boundary has been earned — materially distinct vocabulary, lifecycle, authority, ownership, invariants, policies, rate of independent change, or semantic identity. Do not split files merely because a model is long, a code package happens to exist, or an organizational team owns some piece of the implementation — none of those are domain evidence.

Preserve subdomain identity when composing through `domain-core.sea`. Importing or aggregating modules must never silently collapse concepts that remain distinct in their owning domains (the same rule `SKILL.md`'s Semantic Fidelity Rules state for formalization generally, applied here to composition specifically).

## Composition and imports

DomainForge's parser resolves imports between `.sea` files by relative path, transitively, with no registry file required — this is tested behavior (`domainforge-core`'s module resolver: `resolves_named_relative_import_into_one_symbol_table`), not an assumption. For sibling files in the same directory this is exactly what `.sea/domain-core.sea` needs:

```sea
@namespace "logistics"
@version "1.0.0"

import { Customer, CustomerTier } from "./domain-customer.sea"
import { Shipment } from "./domain-fulfillment.sea"
```

A subdomain module exports the concepts core (or another subdomain) needs to reference:

```sea
@namespace "logistics.customer"

export Entity "Customer"
export Entity "CustomerTier"
```

`NamespaceRegistry::discover` (the mechanism behind `.sea-registry.toml`) walks up from the entry file and simply returns "no registry" when none exists — it is fully optional for this layout. Only add a `.sea-registry.toml` if the project later needs namespace-string imports (`import { X } from "some.namespace"`) across directories that relative paths can't reach; don't add one speculatively.

A relative import that would escape the resolution root (e.g. `../outside.sea` reaching above the entry file's root) is rejected as unresolved by the parser — keep subdomain files inside `.sea/`, not scattered elsewhere, so composition stays resolvable.

**Validate the aggregate to validate the whole model.** `domainforge validate --format human .sea/domain-core.sea` resolves the full import graph (`resolve_filesystem_graph`), so validating `domain-core.sea` validates every subdomain it imports. There is no separate multi-file validation command to remember — the entry point's own imports do the composition.

## Hash-linked and developmental lineage

Two DomainForge mechanisms exist for this, and only these two — do not invent or assume a third:

**`ConceptChange` for a single concept's meaning shift.** A SEA-level declaration (`references/write-in-sea.md` → "Track Evolution") carrying `@from_version`, `@to_version`, `@migration_policy`, and optionally `@breaking_change`. Use it inside `domain-core.sea` or the owning subdomain module when a concept's *meaning* — not just its text — changes between model states.

**Semantic packs for whole-model, content-hashed versioning.** `domainforge pack build` (`references/cli-commands.md` → "Pack Commands") computes a `meaning_fingerprint` (a content hash over the pack's meaning) and a `source_graph_hash`, and — when built with `--previous-pack <path>` — DomainForge itself enforces the lineage rule: if the fingerprint changed from the previous pack, `meaning_version` MUST increase, or the build is rejected with a `MeaningVersionNotBumped` diagnostic. `domainforge pack diff --old <path> --new <path>` classifies exactly what changed between two pack states: `ADD` (additive), `DEF` (definitional change), `DEP` (deprecating), `BRK` (breaking), `GOV` (governance-critical), `SIG` (signature only). `pack sign`/`pack verify` make a given model state tamper-evident.

This is the real mapping for the requested semantic:

```text
Model_n
→ settled concept/domain change             (ConceptChange, when one concept's meaning shifted)
→ changed module content → changed pack fingerprint
→ meaning_version bump enforced by `pack build --previous-pack`
→ `pack diff` classifies the delta
→ Model_n+1
```

## Distinguish the three kinds of change

```text
text changed  ≠  model structure changed  ≠  concept meaning changed
```

- **Text changed** — reformatting, comments, whitespace, `domainforge fmt`. No `ConceptChange`, no version bump.
- **Model structure changed** — a new field, a new relation, a new policy that doesn't redefine an existing concept's meaning. Normal edit; validate as usual.
- **Concept meaning changed** — an existing concept now means something materially different (a field's semantics shifted, an invariant loosened or tightened, an entity's identity boundary moved). This is the case that needs a `ConceptChange` declaration and, if the project builds packs, a `meaning_version` bump.

Do not treat every textual edit as a concept change, and do not let a genuine meaning change pass as an ordinary edit merely because the diff looks small.

## `MODEL-MANIFEST.yaml`

`.agents/reports/domain-model/MODEL-MANIFEST.yaml` — machine-oriented derived state, not an alternative domain representation. Only fields that can be deterministically or reliably derived belong here.

```yaml
aggregate:
  path: .sea/domain-core.sea
  sha256: <hash>

modules:
  - path: .sea/domain-customer.sea
    sha256: <hash>
    domain: customer

  - path: .sea/domain-payments.sea
    sha256: <hash>
    domain: payments

domainforge:
  validation: PASS
  # Include pack/meaning-version/fingerprint fields only when the project
  # actually builds a semantic pack for this model (see above) — do not
  # populate them from an aspiration of doing so later.

source_representation:
  # references to the domain-formation artifacts used

unresolved:
  # references or identifiers only; do not reproduce speculative domain facts
```

**On the `sha256` fields:** these are plain content hashes of each `.sea` file, computed with ordinary tooling (e.g. `sha256sum`) when the manifest is generated — they are Gauntlet's own change-detection bookkeeping, not a DomainForge CLI output. DomainForge's parser does compute a per-module source hash internally (`domainforge-core`'s module resolver), but that value is a private implementation detail (`pub(crate)`, unused outside the crate) with no CLI surface — do not tell a project to "get the hash from DomainForge"; generate it directly. If the project has adopted semantic packs, the pack-level `source_graph_hash` and `meaning_fingerprint` are the DomainForge-native equivalent for the whole aggregate and belong under `domainforge:`, not `modules:`.

Prefer generating this file with a small script/task-runner target over hand-editing it — the hashes exist specifically to catch a file changing without the manifest being regenerated.

## `FORMALIZATION.md`

`.agents/reports/domain-model/FORMALIZATION.md` — developmental/provenance evidence, not a glossary and not another copy of the domain model. The `.sea` files are the formal model; this report explains how and why that model state was reached. Cover only what's consequential:

- source representation used;
- why the current module boundaries exist (what earned each subdomain split, or why the model is still core-only);
- important `source concept → SEA construct` mappings;
- important cross-domain composition decisions;
- formalization-discovered ambiguities;
- questions returned to `domain-formation`;
- material concept changes (each backed by a `ConceptChange` declaration and, where applicable, a pack `meaning_version` bump);
- validation evidence (commands run, PASS/FAIL);
- unresolved boundaries;
- downstream affordances now available.

Do not duplicate every entity, relationship, or policy already represented in SEA — if it's answerable by reading the `.sea` files, it doesn't belong in this report.
