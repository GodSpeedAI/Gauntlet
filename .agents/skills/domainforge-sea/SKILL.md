---
name: domainforge-sea
description: Formalize a sufficiently stable domain representation into DomainForge SEA, then validate and operate on the resulting `.sea` model. Use when Gauntlet needs a computable domain model, SEA validation, graph/AST inspection, projection, policies, metrics, semantic packs, or DomainForge CLI operations. If formalization exposes fuzzy, contradictory, or missing domain meaning, return to `domain-formation` rather than inventing semantics inside SEA.
---

# DomainForge SEA — Gauntlet Integration

## Purpose

Translate a sufficiently stable domain representation into DomainForge's `.sea` DSL and operate on it with the DomainForge CLI.

```text
stabilized domain representation → semantic mapping → SEA formalization → validation → computable domain representation
```

This skill does **not** own initial domain formation — use `domain-formation` to determine what the domain means; use `domainforge-sea` to encode that meaning into SEA without silently changing it.

The normal Gauntlet path:

```text
wayfinder-gauntlet → domain-formation → stabilized domain representation → domainforge-sea → validated .sea model → Gauntlet planning / downstream use
```

DomainForge is an optional formalization backend. Gauntlet must remain capable of domain formation without it.

## Core Boundary

**Meaning before encoding.** SEA syntax must not determine domain meaning. Do not invent, merge, rename, split, or reinterpret domain concepts merely because it makes the `.sea` file easier to validate. A valid SEA file that misrepresents the domain is a failure — prefer `domain representation: stable / SEA formalization: unresolved` over a model that validates but silently changed meaning.

**Return to `domain-formation` when formalization exposes:** a term with multiple incompatible meanings; two concepts that appear conflated; a relationship whose direction or ownership is unclear; an entity/resource/role boundary that cannot be represented without guessing; contradictory source descriptions; a policy that can't be expressed because the underlying rule is unresolved; a scenario exposing a missing distinction; a required SEA construct that would force invention of unsupported meaning. Do not solve semantic ambiguity inside this skill unless the distinction is already evident from authoritative project evidence.

A useful handoff states: the concept/relationship/policy formalization is blocked at, what the source representation currently says, what distinction SEA requires, the evidence, and the specific domain-formation question being asked.

## Bundled References

Load only what the current task needs — this SKILL.md stays focused on procedure and Gauntlet-specific governance; syntax and CLI detail live here:

* `references/write-in-sea.md` — authoring or repairing `.sea` files: file shape, declarations, identifiers, units, policies, metrics, imports, mappings, projections, semantic validation rules, an authoring checklist, and troubleshooting.
* `references/cli-commands.md` — running DomainForge commands: every subcommand (parse, validate, project, import, pack, fmt, ...), flags, exit codes, environment variables, and scenario-based examples.
* `references/sea-dsl-ai-cheatsheet.yaml` — dense syntax, primitives, expressions, error codes, projection rules, and anti-patterns.
* `references/gauntlet-domain-model-layout.md` — creating, restructuring, composing, or materially changing a project's `.sea/` domain model: the filesystem convention, `domain-core.sea` vs. subdomain-module responsibilities, composition/import mechanics, hash-linked lineage (`ConceptChange`, semantic packs), and the `MODEL-MANIFEST.yaml`/`FORMALIZATION.md` contract.

When working inside Gauntlet, also follow `AGENTS.md`, `.agents/`, the current plan, current status, and the frozen reference/answer key. Do not allow SEA work to silently revise a frozen Gauntlet reference or ruler.

## Project Domain Model Layout

The authoritative DomainForge model for a Gauntlet project lives under project-root `.sea/`, composed from `domain-core.sea` and, once a boundary is earned, `domain-<subdomain>.sea` modules — see `references/gauntlet-domain-model-layout.md` before touching that structure. Its load-bearing invariants:

1. the authoritative DomainForge model lives under project-root `.sea/`;
2. the default aggregate entry point is `.sea/domain-core.sea`;
3. bounded domains use `.sea/domain-<subdomain>.sea`;
4. core aggregates and composes subdomains — it never becomes a flattened copy of them;
5. a subdomain boundary must be earned (distinct vocabulary, lifecycle, authority, ownership, invariants, policies, rate of independent change, or semantic identity), never created merely because a file got long;
6. semantic ambiguity discovered while working this layout returns to `domain-formation`, same as any other formalization gap (Core Boundary, above);
7. a material change to a concept's *meaning* — not just its text — uses DomainForge's own `ConceptChange` declaration and, on a project that builds semantic packs, an enforced `meaning_version` bump; it is never an unmarked edit;
8. formalization evidence belongs under `.agents/reports/domain-model/` (`FORMALIZATION.md`, `MODEL-MANIFEST.yaml`), never inside the `.sea` files themselves.

If a project already has a different governed `.sea` layout, do not silently reorganize it to match this one — surface the difference and get migration authorized first.

## Operating Loop

### 1. Determine the source representation

Identify the representation whose meaning must be preserved: output from `domain-formation`, purpose-driven artifacts (user stories, problem statements, workflow narratives), `CONTEXT.md`, a repository-specific domain model, an existing `.sea` file being revised, a Wayfinder formation artifact. Don't treat miscellaneous prose as authoritative when a governed domain representation already exists. Record which source was used.

### 2. Assess formation readiness

Ask: are the distinctions required by this formalization already stable? Check concept identity, entity/role/resource distinctions, ownership, directionality, lifecycle boundaries, cross-context identity, important flows, policy semantics, units/dimensions, authority relationships. If clear, proceed; if not, return the unresolved portion to `domain-formation`. Do not use SEA syntax as a substitute for domain clarification.

### 3. Build a semantic mapping

Before or while authoring, maintain a traceable `source concept → SEA construct → rationale` mapping for consequential concepts, e.g.:

```text
CustomerOrganization → Entity "CustomerOrganization" → persistent domain actor owning the commercial relationship
AccountManager → Role "AccountManager" → behavioral/authority role, not an independent organization
```

Do not silently map two source concepts to one SEA concept, or one source concept to several, without recording why.

### 4. Inspect nearby SEA conventions

Before inventing style, inspect existing `.sea` models, namespaces, concept naming, units, and module/import structure. Prefer local conventions when they don't conflict with current DomainForge syntax (`references/write-in-sea.md`).

### 5. Author the smallest sufficient SEA model

Encode only the domain distinctions needed for the current Gauntlet horizon — current required concepts plus required relationships plus required policies, not speculative completeness. Keep unresolved material unresolved. See `references/write-in-sea.md` for declaration order, identifiers, units, policies, and the minimal-file example. Place the model per the Project Domain Model Layout above — core versus an earned subdomain module — rather than defaulting everything into one file or splitting prematurely.

### 6. Validate immediately

After material edits, run `domainforge fmt --check <file>.sea` and `domainforge validate --format human <file>.sea` (or `--format json` when exact diagnostics or downstream tooling matter — see `references/cli-commands.md`). If validation fails, determine whether it's a SEA syntax/formalization problem or a domain representation problem by asking: *can this be corrected without changing what the domain means?* If yes, fix the SEA representation (`references/write-in-sea.md` has a troubleshooting section for the common cases). If no, return to `domain-formation`. Never repair a domain problem by changing formal semantics.

### 7. Inspect structure when needed

Use `domainforge parse <file>.sea --ast --format json` and `domainforge graph <file>.sea` when the source text validates but the resulting structure may not represent what was intended — a syntactically valid parse is not sufficient evidence of semantic fidelity.

### 8. Produce formalization evidence

Report: source domain representation used; `.sea` files created or changed; concept-to-SEA mappings that materially matter; validation commands run and their pass/fail results; unresolved semantic gaps; whether `domain-formation` needs to be re-entered. For projection, import/round-trip, or semantic-pack work, run the relevant `domainforge project` / `domainforge import` / `domainforge pack` command (`references/cli-commands.md`) and record its exit behavior as evidence, not just narration.

## Semantic Fidelity Rules

**Do not invent concepts to satisfy the DSL.** If the source domain representation does not establish a concept, do not create one merely because SEA requires a node or intermediary. Either find another faithful representation, or send the gap back to `domain-formation`.

**Do not collapse important identities.** If two concepts are distinct in the source representation, preserve their identities unless the current operation explicitly establishes a safe higher-order abstraction. `CustomerOrganization ≠ User` should not become a single `Entity "Customer"` unless that abstraction is explicitly sufficient for the current purpose.

**Do not hide unknowns.** If a relationship or policy remains unresolved, preserve that uncertainty outside the SEA assertion rather than inventing closure. State the unresolved relationship explicitly rather than asserting a guessed one that happens to validate.

**Keep domain and implementation separate.** Don't automatically encode software implementation artifacts (a database table, an API endpoint, a Rust struct, a message queue, a service class) as domain concepts — only formalize them when they genuinely participate in the modeled domain.

## Relationship to `domain-formation`

`domain-formation` and `domainforge-sea` may iterate: a stable representation goes to SEA, formalization exposes ambiguity, that ambiguity goes back to `domain-formation`, a new distinction comes back to SEA. This is not failure — formalization can expose representational defects invisible in prose. The formalizer must not independently settle those semantic questions unless the available evidence already makes the answer unambiguous.

## Relationship to Wayfinder / Gauntlet Formation

Wayfinder produces destination, fog, constraints, initial vocabulary, and unresolved questions; `domain-formation` sharpens the representation; `domainforge-sea` formalizes only distinctions that have become sufficiently stable. Do not promote Wayfinder fog or out-of-scope speculation into asserted SEA concepts. When a `.sea` model is used as a downstream Gauntlet planning representation, its semantic provenance must remain traceable to the formed domain representation.

## Gauntlet Governance

**Frozen references.** Do not revise a frozen Gauntlet ruler, answer key, or reference merely because the `.sea` projection disagrees with it. A projection mismatch may indicate a SEA mapping defect, a domain formation defect, a reference defect, or an implementation defect — determine the correction locus through the active Gauntlet process.

**Authority.** This skill may author candidate `.sea` representations, validate them, inspect AST/graph structure, report semantic mapping problems, and propose domain clarifications. It must not silently redefine the governing domain, alter frozen acceptance criteria, promote unresolved concepts into settled facts, or declare a semantic projection authoritative merely because it validates.

**Settlement.** SEA validation proves the model is accepted by DomainForge's syntax/semantic validator. It does not prove the model faithfully represents the intended domain. Faithful formalization requires both source semantic agreement and DomainForge validation.

## If the CLI Isn't Installed

```bash
cargo install --path domainforge-core --features cli --force
domainforge --help
```

Prefer the `domainforge` binary; if older documentation says `sea`, verify that binary exists locally before using it.

## Changing DomainForge Itself

This section applies only when the task is to modify DomainForge, not to use SEA — it must not make Gauntlet depend on the DomainForge source repository. Typical paths: grammar (`domainforge-core/grammar/sea.pest`), parser AST (`domainforge-core/src/parser/ast.rs`), parser tests (`domainforge-core/tests/parser_*.rs`). If primitives change, update the Rust core and the Python/TypeScript/WASM bindings per that repository's `AGENTS.md`. Use `IndexMap`, not `HashMap`, for policy-relevant deterministic collections. Flows reference IDs, not entity/resource objects. Run the narrowest relevant tests first, then the repository's full gate when shared contracts are affected.

## Formalization Completion Criteria

SEA formalization is sufficiently complete for the current Gauntlet horizon when: every consequential SEA concept maps to a stabilized source concept or an explicitly justified derived construct; important source distinctions have not been silently collapsed; unresolved domain questions remain explicitly unresolved rather than encoded as facts; the `.sea` model passes required DomainForge validation; AST/graph inspection agrees with the intended structure when that structure matters; downstream Gauntlet work can consume the model without reconstructing missing domain meaning; any semantic gap discovered during formalization has either been resolved through `domain-formation` or preserved as a declared unresolved boundary.

Do not claim the entire domain is modeled. The target is the smallest faithful computable representation sufficient for the current Gauntlet horizon.

## Final Report

When this skill materially changes a Gauntlet domain model, report: source representation; `.sea` artifacts created or changed; important `source concept → SEA construct` mappings; validation commands and PASS/FAIL results; formalization gaps; whether `domain-formation` was re-entered; remaining unresolved distinctions; downstream affordances now available.

The success condition is not "SEA validates." It is "SEA validates AND the resulting representation still means what the formed domain means."
