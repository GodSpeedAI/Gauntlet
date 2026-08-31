---
name: domain-formation
description: Form and sharpen a project's domain representation. Use when terminology is ambiguous, concepts or boundaries need clarification, concrete scenarios need to be modeled, domain decisions need to be preserved, or a project needs a stable representation before planning or formalization. Uses the project's configured domain representation when one exists; otherwise falls back to CONTEXT.md and sparse ADRs. DomainForge SEA formalization is handled separately by the domainforge-sea skill.
---

# Domain Formation

## Purpose

Actively form and sharpen the domain representation that downstream planning, implementation, and formalization depend on.

This skill is concerned with **meaning before encoding**.

Use it to:

* identify domain-specific concepts;
* distinguish terms that are being conflated;
* sharpen ambiguous or overloaded language;
* expose important boundaries and relationships;
* test the representation against concrete scenarios;
* reconcile stated domain behavior with existing code or evidence;
* preserve stabilized terminology and consequential decisions;
* prepare a sufficiently stable representation for downstream consumers.

Do not confuse domain formation with merely reading an existing glossary. Reading `CONTEXT.md`, a `.sea` model, or another domain artifact for vocabulary is ordinary context consumption. This skill is for **changing or extending the domain representation**.

## Core Boundary

Domain formation and formalization are separate operations.

This skill owns:

```text
fuzzy language
→ distinctions
→ concepts
→ relationships
→ boundaries
→ scenarios
→ stabilized domain representation
```

It does not own a particular formal DSL.

When DomainForge is selected for the project, use `domainforge-sea` after the relevant domain distinctions have stabilized:

```text
domain-formation
        ↓
stabilized domain representation
        ↓
domainforge-sea
        ↓
validated .sea representation
```

Do not force domain meaning to fit SEA syntax. If a stabilized distinction cannot yet be represented faithfully in SEA, preserve the domain distinction and surface the formalization gap.

DomainForge is an optional formalization backend, not a prerequisite for forming a useful domain model.

## Representation Selection

First inspect the repository for an existing domain representation or explicit project configuration.

Prefer, in order:

1. the project's declared domain representation;
2. an established repository-specific domain artifact;
3. `CONTEXT.md` plus sparse ADRs as the portable fallback.

Examples of project-specific representations may include:

* a DomainForge `.sea` model;
* a repository-defined domain schema;
* a domain graph or structured model;
* another explicitly governed representation.

Do not create a competing representation when an authoritative one already exists.

If DomainForge is configured or already in use, preserve newly stabilized meaning in the project's ordinary domain notes as needed, then use `domainforge-sea` for SEA authoring, validation, formatting, projection, or CLI work.

## Portable Fallback

When no project-specific representation exists, use:

```text
CONTEXT.md
docs/adr/
```

Most repositories need only one `CONTEXT.md` at the repository root.

If `CONTEXT-MAP.md` exists, treat the repository as having multiple domain contexts. Follow the map to the appropriate context-specific `CONTEXT.md` and ADR location.

Create files lazily:

* create `CONTEXT.md` when the first project-specific term actually stabilizes;
* create `docs/adr/` only when the first decision genuinely warrants an ADR.

Do not create documentation simply to satisfy the skill.

# Operating Loop

## 1. Inspect Existing Representation

Before inventing terminology:

* inspect the relevant domain artifact;
* inspect nearby code, schemas, interfaces, tests, or examples;
* identify existing canonical terms;
* note declared synonyms or deprecated terminology;
* identify unresolved or contradictory language.

Preserve established terminology unless evidence or the requester gives reason to revise it.

## 2. Identify the Current Representational Problem

Determine what is actually unresolved.

Typical cases include:

* one term being used for multiple concepts;
* multiple terms being used for one concept;
* unclear ownership or authority;
* unclear lifecycle or state boundaries;
* ambiguous entity relationships;
* disagreement between prose and implementation;
* a scenario that the current representation cannot explain;
* a new distinction required by observed behavior.

Do not broaden the model merely because additional concepts are imaginable.

## 3. Challenge the Glossary

When the requester uses a term inconsistently with the existing representation, surface the conflict immediately.

Example:

> Your current glossary defines `Account` as the customer's billing relationship, but here you appear to mean the authenticated user identity. Are those intended to be different concepts?

Do not silently reinterpret established terms.

## 4. Sharpen Fuzzy Language

When a term is overloaded or underspecified, propose the smallest useful distinction.

Example:

> You're using `Customer` for both the organization buying the service and the individual using it. Do we need `CustomerOrganization` and `User`, or is that distinction irrelevant to the current problem?

Do not create distinctions that have no consequence for the current work.

A distinction is useful when preserving it changes at least one of:

* expected behavior;
* authority;
* lifecycle;
* ownership;
* policy;
* interaction;
* validation;
* planning;
* implementation;
* settlement.

## 5. Test With Concrete Scenarios

Stress-test important relationships using concrete cases.

Prefer scenarios that discriminate between competing representations.

Examples:

* What happens when two users belong to the same customer organization?
* Can an order exist without a payment?
* Can a cancelled booking later be restored?
* Which actor owns the resource before and after transfer?
* What happens if the same external identifier appears in two contexts?

Do not invent scenarios merely for completeness. Use them to determine whether the current representation is sufficient.

## 6. Cross-Reference Reality

When the requester states how the domain works, inspect available implementation or evidence when practical.

Surface contradictions rather than silently choosing one source.

Example:

> The domain description says partial cancellation is supported, but the current implementation cancels the entire Order. Which one represents the intended domain?

Treat code as evidence of current implementation, not automatically as authority over intended domain meaning.

Likewise, prose is not automatically authoritative over observed behavior.

## 7. Preserve Stabilized Meaning

Once a distinction becomes sufficiently stable, record it promptly in the project's chosen representation.

Do not wait until the end of a long session and reconstruct the model from memory.

Preserve:

* canonical term;
* concise definition;
* important distinction from nearby concepts;
* deprecated or misleading synonyms when useful;
* relationships required for correct interpretation.

Do not record speculative distinctions as settled domain concepts.

If something remains unresolved, preserve it explicitly as unresolved rather than forcing premature closure.

# CONTEXT.md Fallback Rules

When using `CONTEXT.md`, treat it primarily as a concise domain glossary.

Each concept should normally contain:

* the canonical term;
* one or two sentences describing what it **is**;
* important synonyms to avoid, when relevant.

Prefer:

```markdown
### Customer

The organization that purchases the service and owns the commercial relationship.

Avoid using `Customer` to mean an individual authenticated user; use `User` instead.
```

Avoid turning `CONTEXT.md` into:

* an architecture document;
* an API reference;
* an implementation guide;
* a feature specification;
* a task list;
* a general programming glossary.

Include only terminology whose meaning is specific to the project's domain.

Group concepts when natural clusters emerge. A flat list is acceptable while the representation is still small.

# Decisions and ADRs

Use ADRs sparingly.

Create or offer an ADR only when all three conditions hold:

1. **Hard to reverse** — changing course later has meaningful cost.
2. **Surprising without context** — a future reader is likely to wonder why this choice was made.
3. **Real trade-off** — plausible alternatives existed and one was selected for a reason.

If any condition is absent, do not create an ADR.

A concise ADR is acceptable. Preserve:

* the decision;
* the important alternative or tension;
* why the selected direction won.

When using the portable fallback, number ADRs sequentially:

```text
docs/adr/0001-slug.md
docs/adr/0002-slug.md
```

Scan existing ADRs before choosing the next number.

# DomainForge Handoff

When DomainForge is selected, keep formation and SEA formalization distinct.

Before invoking or handing work to `domainforge-sea`, provide a sufficiently stable source representation containing the concepts that matter.

The handoff should preserve, when applicable:

```text
canonical concepts
definitions
relationships
roles
resources
important flows
important constraints
unresolved distinctions
source evidence or scenarios
```

The SEA formalization must preserve semantic provenance.

Do not allow formalization to silently:

* rename concepts in a way that changes meaning;
* merge distinct concepts merely because one SEA construct is convenient;
* invent entities or relationships solely to satisfy syntax;
* discard unresolved distinctions;
* turn missing information into asserted domain facts.

If SEA cannot faithfully encode something, report the mapping problem.

The correct result may be:

```text
domain representation: stable
SEA projection: unresolved
```

That is preferable to a syntactically valid but semantically false model.

# Multiple Contexts

When a repository contains multiple bounded contexts or domain regions, preserve their separate identities.

Do not merge concepts merely because they share a name.

For example:

```text
Billing.Account
Identity.Account
```

may represent different concepts even if both are casually called `Account`.

Use `CONTEXT-MAP.md`, repository structure, DomainForge namespaces, or another project-specific mechanism to record the separation when available.

Record cross-context relationships explicitly rather than collapsing the contexts into one global glossary.

# Representation Discipline

## Preserve Identity Until Aggregation Is Earned

Do not aggregate different concepts simply because they share a broader category.

For example:

```text
Customer
Supplier
```

may both be:

```text
Organization
```

but that does not make them interchangeable.

Use a higher-order category only when the distinctions being suppressed are irrelevant to the operation being performed.

## Do Not Invent Closure

If observations conflict with the current domain representation and no available distinction resolves them:

1. preserve the conflicting observations;
2. record the representations or explanations already attempted;
3. identify what remains unresolved;
4. avoid inventing a new concept merely to make the model appear complete.

A low-compression description of unresolved observations is preferable to a false abstraction.

## Change the Smallest Sufficient Representation

Prefer local clarification when it resolves the problem.

Escalate toward broader remodeling only when repeated or structurally similar residuals show that the existing representation itself is insufficient.

Do not restructure the entire domain because of one local exception.

# Relationship to Planning

Domain formation should produce enough representational stability for planning, but it should not turn into the implementation plan itself.

A plan may depend on distinctions established here.

Example:

```text
domain formation establishes:
CustomerOrganization ≠ User

therefore planning may legitimately create:
organization ownership
user membership
authorization boundaries
```

The domain representation explains what exists and what distinctions matter.

The plan determines what transformations should occur next.

# Completion Criteria

Domain formation is sufficiently complete for the current task when:

* relevant terms have stable meanings;
* important neighboring concepts are distinguishable;
* required relationships and boundaries are representable;
* known contradictions have either been resolved or explicitly preserved;
* concrete scenarios no longer expose a missing distinction required by the current work;
* downstream planning can use the representation without silently inventing domain meaning.

Do not claim the entire domain is complete.

The target is the **cheapest sufficient representation for the current horizon**.

# Evidence

When this skill materially changes the domain model, report:

* which domain artifacts changed;
* which concepts or distinctions were added, removed, merged, or renamed;
* what evidence or scenarios motivated the change;
* what remains unresolved;
* whether formalization is required next.

When DomainForge is used downstream, `domainforge-sea` owns SEA-specific validation and proof commands.

Domain formation itself should not claim that a `.sea` representation is valid merely because the semantic model appears sound.
