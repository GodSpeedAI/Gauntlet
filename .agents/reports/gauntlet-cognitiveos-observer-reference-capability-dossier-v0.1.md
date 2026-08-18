# Gauntlet × CognitiveOS: Observer Governance, Reference Integrity, and Capability Formation

**Version:** 0.1  
**Date:** 2026-08-17  
**Status:** **NOT CANON. Working synthesis, research dossier, and gated action register.**  
**Purpose:** Preserve the strongest conclusions from the Gauntlet/CognitiveOS/DomainForge discussion in a self-contained form without promoting exploratory ideas into the active Gauntlet specification or CognitiveOS canon.

---

## 0. Reading contract

This document deliberately separates what is already canonical, what has actually happened in the Gauntlet build, what follows from those facts, and what remains a proposal.

### Claim classes

| Label | Meaning |
|---|---|
| **CANON** | Already established in current Gauntlet or CognitiveOS normative/canonical sources. |
| **ESTABLISHED** | Directly observed in the current implementation/build record. |
| **DERIVED** | Follows from CANON + ESTABLISHED facts without requiring a new empirical claim. |
| **HYPOTHESIS** | Falsifiable proposal that has not yet earned promotion. |
| **CONJECTURE** | Plausible interpretation that is not yet operationalized well enough to lose. |
| **CANDIDATE REVISION** | Proposed change to a frozen or canonical representation. Must go through its normal revision process. |
| **ACTION CANDIDATE** | Possible work item. It is not automatically active. Each has explicit activation criteria. |

### Operating rule

This document must not be used to walk the active Gauntlet ruler.

The active Gauntlet build remains governed by:

1. `.agents/specs/Core-spec.md`;
2. the frozen `.wayfinder/answer-key.md`;
3. `.agents/plans/gauntlet-impl.md`;
4. `AGENTS.md`;
5. the current repository state and evidence.

Where this dossier identifies a possible spec or canonical defect, the proper output is a **CandidateRevision/reference candidate**, not an implementation shortcut.

---

# 1. Current Gauntlet implementation state

## 1.1 Established status

As of the current handoff:

- Tasks 0 through 5 are closed.
- The workspace has 385 passing tests.
- Eight answer-key Units are implemented-but-provisional.
- **Zero Units are implemented-and-proven**, because the VerificationRecord machinery does not exist until Task 11.
- Task 6 is next: StateStore, EvidenceStore, and EventLog.
- Task 20 is the first full mock loop and the beginning of self-hosting/dogfooding.
- Task 26 compares baseline profiles A/B/C/D and explicitly permits the full Gauntlet to lose.
- Task 27 performs final conformance.
- The frozen Wayfinder ruler has not been modified.
- The first entry now exists in `.agents/reports/reference-candidates.md`.

This means the correct proof posture is still:

```text
bootstrap-method evidence
≠
runtime settlement
≠
runtime capability
≠
system conformance
```

The implementation is producing increasingly strong evidence about its build discipline, but formal Unit settlement has not begun.

---

# 2. The first live structural-revision event

## 2.1 AK-049 is qualitatively different

**ESTABLISHED.**

AK-049 requires an answer-key evidence locator shaped like `tests/e2e/doctor::...`.

The implementation discovered that this required locator cannot legally exist under the active architecture/escape rules. The system did not:

- edit the frozen answer key;
- create a fake locator to satisfy the letter;
- silently ignore the requirement;
- halt the entire build.

Instead it recorded a dated entry in `reference-candidates.md` and left the ruler unchanged.

This is best described as:

> **the first observed reference-divergence / CandidateRevision event.**

It is **not yet a representational settlement**. Settlement occurs only when the candidate is eventually adjudicated and the decision is recorded.

## 2.2 What this event actually establishes

The narrow supported claim is:

> The process can detect that its frozen reference may be defective without permitting the active implementation run to rewrite that reference.

This is the first concrete evidence that the Governed Structural Revision channel can **fire**.

It does not yet establish that GSR produces good revisions. The candidate has not been accepted or rejected, and no held-out consequence has tested the eventual decision.

## 2.3 The reference can be wrong

This event changes the epistemic picture.

The reference is not Truth. It is the frozen normative representation against which a run is evaluated.

Therefore:

```text
reference integrity
≠
reference correctness
```

A frozen reference can be:

- internally coherent and correct;
- internally inconsistent;
- stale;
- incomplete;
- implementable but factually wrong;
- normatively wrong;
- correct for one intervention set and wrong for another.

Freezing prevents self-grading. It does not make the frozen object infallible.

---

# 3. The Observer role as a projected contract

## 3.1 Core insight

**DERIVED / HYPOTHESIS as a reusable domain abstraction.**

CognitiveOS already defines an Observer broadly enough to include people, teams, organizations, software agents, and multi-agent systems.

The useful projection for Gauntlet is not:

> “Gauntlet is an observer.”

It is:

> **Any system that participates in the observation contract may be treated as an Observer for that interaction, regardless of what else that system is.**

Therefore the same concrete system may validly participate as different roles:

```text
Gauntlet as controller
Gauntlet as governor
Gauntlet as settlement system
Gauntlet as observer

Prime-Agent as orchestrator
Prime-Agent as actor
Prime-Agent as observer when it emits a claim about state

Critic as observer
Verifier as observer
Doctor as observer
RealityTrace as observer
Human reviewer as observer
```

This is a projection, not an ontological identity claim.

## 3.2 Generic Observer contract

A minimal conceptual contract is:

\[
O:(T,R,E)ightarrow(\hat{T},C)
\]

where:

- \(T\) = target being observed;
- \(R\) = reference or question relative to which the observation matters;
- \(E\) = evidence available to the observer;
- \(\hat{T}\) = the observer's representation of the target;
- \(C\) = claim made from that representation.

The governing question is:

> Does \((\hat{T},C)\) preserve the consequential distinctions of \((T,R)\) strongly enough for the **type of claim** being asserted?

---

# 4. No special meta-observer type is required

## 4.1 O1 and O2 are the same kind of thing

Let O1 observe an implementation artifact.

```text
O1:
target = final artifact
reference = frozen Wayfinder
output = representation + claims
```

Let O2 observe O1's observation.

```text
O2:
target = Observation(O1)
reference = same frozen Wayfinder plus independently resolvable anchors
output = claims about O1's correspondence, scope, and evidence
```

Then:

\[
O_1\in\mathcal O
\]

and:

\[
O_2\in\mathcal O
\]

There is no need to invent `MetaObserver`, `MetaMetaObserver`, and so on as new ontological types.

The recursion is handled by applying the **same Observer grammar** to a different target.

## 4.2 What terminates the regress

The regress does not terminate because one observer is declared ultimate.

It terminates against **independently resolvable anchors**, such as:

- seeded defects;
- adjudicated non-defects;
- deterministic invariants;
- frozen fixtures;
- cryptographic digests;
- byte-identical reversion;
- model-checkable reachability;
- external physical or operational consequence;
- independently authored reference material;
- held-out historical events.

The goal is not an omniscient observer.

The goal is an observer whose scope, failure modes, and evidential authority are bounded well enough that the claim it makes is justified.

---

# 5. O2 should compare the observer to the final artifact and ruler, not the builder's story

## 5.1 Conformance object

For ordinary conformance, define:

- \(R\) = frozen reference;
- \(A_1\) = actual final artifact/current state;
- \(C_1\) = O1's claim about the relationship between \(R\) and \(A_1\).

Then O2 asks whether O1 accurately represents:

\[
R \leftrightarrow A_1
\]

The builder's narrative, reasoning, attempt count, or completion claim is not the settlement object.

## 5.2 Why the diff is not the conformance object

A Git diff answers:

> What textual or recorded change appears to have occurred?

It does not by itself establish:

- what final system now exists;
- whether inherited code composes with the change;
- what behavior is reachable;
- what runtime consequence occurs;
- whether the change actually caused the observed behavior.

Therefore:

```text
diff
= transformation provenance

final artifact
= current implementation state

observed behavior
= runtime/effective state evidence

settled consequence
= consequence evidence against the declared criterion
```

For conformance:

\[
oxed{R\leftrightarrow A_1}
\]

is primary.

## 5.3 The important refinement: not every claim settles against a state

There are at least four settlement-object types:

| Claim type | Settlement object |
|---|---|
| **State / conformance** | \(A_1\) |
| **Transition / invariant-preserving change** | \((A_0,f,A_1)\) |
| **Trace / teeth** | trajectory \(	au=(A_0,A_1,A_2,	ext{observations})\) |
| **Set-integrity / inventory** | \((S_0,S_1)\), with delta derived |

So the correct rule is:

> **Conformance settles against states. Transformation invariants settle against trajectories.**

A textual diff is only one lossy projection of a transformation.

---

# 6. Claim typing is more fundamental than observer typing

## 6.1 Calibration cannot change logical claim type

This is one of the strongest conclusions in the discussion.

Suppose a critic searches region \(S\) and finds no evasion.

What it has established is:

\[

eg\exists x\in S:\operatorname{Evasion}(x)
\]

It has **not** established:

\[
orall x:
eg\operatorname{Evasion}(x)
\]

No critic sensitivity score, model quality, or number of critics bridges that logical gap.

Therefore:

> **Calibration sets evidence strength. It does not upgrade claim type.**

## 6.2 Current wording defect

Some existing status language is stronger than the procedure that generated it. Examples include formulations like:

- “no evasion route exists”;
- “the tightest defensible slice.”

Unless the searched region or candidate class was explicitly bounded and exhausted, these should be reported as:

```text
bounded-search negative:
under threat model T,
search procedure P,
observer configuration O,
and budget B,
no evasion was found in searched region S
```

And a minimality claim should be:

```text
within declared candidate class C
under declared cost/constraint,
candidate X was minimal among evaluated candidates
```

This is a **claim-discipline defect**, not a critic-calibration defect.

## 6.3 Candidate claim types

A future Observation grammar may benefit from explicit claim typing. Candidate types include:

- existential;
- bounded-universal;
- universal;
- comparative;
- state;
- transition;
- trace;
- causal;
- normative.

The exact taxonomy is not yet canonical.

The load-bearing rule is:

> **Claim type must be declared before observing, not upgraded after the result is seen.**

---

# 7. Observer error decomposition

A useful candidate refinement is:

\[
E_O=
(
E_{	ext{perception}},
E_{	ext{reference}},
E_{	ext{inference}},
E_{	ext{evidence}},
E_{	ext{coverage}}
)
\]

## 7.1 Perception error

The observer misrepresents the target.

Example:

```text
observer says module X changed behavior
actual artifact does not contain/produce that behavior
```

## 7.2 Reference error

The observer misreads what the frozen reference requires.

## 7.3 Inference error

The observer correctly represents both target and reference but draws the wrong conclusion.

## 7.4 Evidence error

The cited evidence does not entail the claim.

The Task-5 BOM failure is the archetype:

```text
evidence:
parser returned success / contract found

claim:
configuration layer was applied

actual:
BOM caused the layer to be silently discarded
```

The instrument reported a proposition its predicate did not establish.

## 7.5 Coverage error

The observer fails to inspect a consequential part of the target or relevant evidence surface.

---

# 8. The discrepancy triangle

For a conformance observation, let:

- \(R\) = reference;
- \(A\) = actual artifact/state;
- \(C\) = observer's representation/claim.

Then inspect:

\[
R\leftrightarrow A
\]

\[
A\leftrightarrow C
\]

\[
R\leftrightarrow C
\]

A simplified diagnostic table:

| Reference vs artifact | Artifact vs observer | Candidate locus |
|---|---|---|
| coherent | coherent | observation/reference agreement |
| diverges | coherent | implementation divergence |
| coherent | diverges | observer divergence |
| diverges | diverges | ambiguous; cheapest discriminating test required |
| reference internally inconsistent and observer exposes it | n/a | reference-candidate event |

AK-049 is the first observed instance of the final row.

This is a candidate extension to CognitiveOS diagnostic closure and the Math Layer failure classifier.

---

# 9. Purpose, reference, effective state, instrumentation, consequence

The discussion exposed a deeper chain than “reference vs implementation.”

```text
PURPOSE / DESIRED CONSEQUENCE
          ↓
REFERENCE
          ↓
EFFECTIVE SYSTEM
          ↓
INSTRUMENTED REPRESENTATION
          ↓
ACTUAL CONSEQUENCE
```

Each edge can fail independently.

## 9.1 Purpose ≠ Reference

The reference is internally coherent but encodes the wrong desired behavior.

This is the hardest current blind spot.

## 9.2 Reference ≠ Effective

Implementation/configuration fails to realize the reference.

## 9.3 Effective ≠ Instrumented

The measuring apparatus reports the wrong effective state.

The BOM incident is this shape.

## 9.4 Instrumented ≠ Consequence

Internal observation does not correspond to what actually happened in the world.

This is where RealityTrace or another external consequence observer matters.

## 9.5 Consequence ≠ Purpose

The system faithfully implemented the reference and produced the predicted consequence, but that consequence did not create the desired progress.

This exposes a reference-direction problem, not necessarily an implementation problem.

---

# 10. The consistent-and-wrong reference blind spot

## 10.1 Common-reference correlation

Critics, deterministic gates, tests, verifiers, and doctors can differ radically in implementation while still share the same dominant failure cause:

\[
R
\]

If all of them evaluate conformance to the same wrong predicate, they can agree perfectly on the wrong answer.

Therefore:

> **Implementation diversity does not imply epistemic independence.**

Independence must be about distinct failure mechanisms, including:

- reference lineage;
- evidence lineage;
- predicate/derivation lineage;
- model lineage;
- prompt lineage;
- tool/instrument lineage.

## 10.2 Two different ways a reference defect can surface

### Internal inconsistency

Two reference commitments conflict.

AK-049 is this shape.

### External divergence

The reference is internally coherent but fails against external consequence and an independently anchored desired outcome or normative criterion.

RealityTrace can supply:

> what actually happened.

It does **not** by itself supply:

> whether what happened was the right thing to want.

A coherent-but-wrong reference requires both:

1. independent consequence observation; and
2. an independent purpose/outcome/normative criterion against which consequence can be judged.

This is why CognitiveOS's separation between descriptive settlement and governance/normative acceptance remains load-bearing.

---

# 11. Critic, gate, verifier, doctor, and RealityTrace are not equivalent epistemic roles

All can satisfy the Observer contract, but their typical functions differ.

## 11.1 Exploratory observer

Typical implementation: blind critic.

Purpose:

- search open-ended failure space;
- find novel evasion classes;
- find vacuity;
- challenge minimality assumptions;
- generate candidate distinctions.

Its strength is frontier search, not proof of universal absence.

## 11.2 Discriminating observer

Typical implementation: deterministic gate, test, model checker, teeth harness.

Purpose:

- decide a bounded predicate;
- repeatedly discriminate known valid/invalid states;
- provide cheap reusable checks.

## 11.3 Entailment observer

Typical implementation: verifier.

Purpose:

- determine whether cited evidence actually satisfies the declared evidence requirement for a specific claim.

## 11.4 State observer

Typical implementation: doctor/config/runtime probe.

Purpose:

- report effective internal state.

## 11.5 Consequence observer

Typical implementation: RealityTrace or an independent external instrument.

Purpose:

- record what actually occurred outside the internal self-description boundary.

These are not exclusive classes. A single system can assert multiple claim types.

The safer primitive is therefore **claim type**, not a rigid observer taxonomy.

---

# 12. The critic's most valuable role may be check generation

A strong pattern already exists in the bootstrap organization:

```text
unknown failure class
→ critic discovers witness
→ failure distinction becomes explicit
→ deterministic predicate/check is built
→ teeth proves the check bites
→ future validation cost becomes very low
```

This suggests:

```text
critic
= expensive exploratory search

deterministic check
= metabolized discrimination
```

The critic may therefore contribute more value by discovering **new check-design hypotheses** than by adding one more vote to a settlement claim.

This matters because three correlated LLM critics may add less epistemic value than one new deterministic check that permanently covers a newly discovered failure class.

---

# 13. Critic calibration: E−1

## 13.1 Why calibration is needed

Observed critic yield has declined across parts of the build.

That trend is compatible with multiple explanations:

- the artifacts genuinely hardened;
- critic sensitivity changed;
- task domains changed;
- builders learned the visible search surface;
- prompts/models changed.

A “critic found nothing” result is not interpretable until the critic's ability to detect known cases is understood.

## 13.2 Positive controls

Re-inject previously found defects one at a time into closed artifacts, for example:

- RoleId grammar bypass;
- foreign-run settlement hole;
- BOM config-layer discard;
- `[env] RUSTFLAGS` channel;
- comment-in-directive evasion;
- I/O-kind conflation;
- duplicate-key fingerprint collision;
- explicit `--config` NotFound fallback.

Dispatch fresh blind critics without telling them a defect was seeded.

## 13.3 Negative controls

Use **adjudicated non-defects**, not merely “clean current code.”

Examples already exist in Task 4 and Task 5 accepted-with-doc decisions.

A critic that raises these again as critical without new evidence is a known false positive relative to the current reference/adjudication.

## 13.4 Endpoints

Candidate measures:

\[
Se=P(C^+\mid D^+)
\]

\[
Sp=P(C^-\mid D^-)
\]

plus:

- sensitivity by defect class;
- specificity;
- class blindness;
- pairwise correlated blindness across critic configurations;
- marginal defect-class coverage added by an additional critic.

## 13.5 What calibration does not buy

Even a highly sensitive critic cannot prove a universal absence claim.

Calibration tells you:

> how much evidential weight a bounded search result deserves.

It does not change:

> what kind of proposition was actually established.

---

# 14. Corroboration and correlated blindness

## 14.1 Candidate defect in CognitiveOS Math Layer

The current Math Layer settlement-reliability model includes corroboration \(K\) and uses language such as “independent signals.”

The term **independent** is not operationally defined strongly enough for multi-agent systems.

This creates a candidate failure mode:

```text
same model lineage
same prompt lineage
same evidence
same reference
three agreeing outputs
→ treated as three independent signals
```

when their effective information may be close to one signal.

## 14.2 Candidate revision R-2

A possible future revision would require declared signal lineage, including at least:

- reference version;
- evidence set;
- predicate/derivation path;
- model family/version where relevant;
- prompt-contract version where relevant;
- tool/instrument lineage.

Tier-0 conservative posture:

> **Undeclared independence should not receive multi-source corroboration credit.**

Do not operationalize numeric correlation discounts until calibration data exists.

## 14.3 Important qualification

A deterministic gate and an LLM critic are not automatically independent.

They may encode the same wrong reference predicate.

Distinct implementation substrate is not enough.

---

# 15. Minimum viable Observer record

Do not introduce Tier-4 reliability fields before measurements exist.

A safe minimum conceptual ObserverProfile is:

```text
observer_id
observer_version
declared_scope
known_unavailables
lineage
calibration_status
```

where:

```text
calibration_status =
  uncalibrated
  | calibrated@{date,suite_version}
  | stale
```

`uncalibrated` is the honest default.

A separate Observation record would contain:

```text
observer_ref
target_ref
reference_ref
claim
claim_type
evidence_ref
observed_scope
```

Numeric sensitivity, specificity, drift scores, or reliability weights should be added only after a calibration process exists that can populate them honestly.

---

# 16. Observability horizon

A useful extension candidate is the **observability horizon**:

> the bounded set of claims about its own action and environment that an organization can currently establish with sufficient observer coverage and settlement reliability.

It must be indexed over a declared claim distribution. Do not define it as “everything the organization could know.”

Current Gauntlet example:

```text
effective/internal state observation:
increasing coverage

instrumented vs effective mismatch:
some coverage, with Task-5 evidence

instrumented vs external consequence:
near-zero coverage until RealityTrace or equivalent external observation is integrated
```

Observer portfolio value is not headcount. It is the consequential claim/defect space covered with sufficiently independent failure mechanisms.

---

# 17. Adjudicated distinction cases as a real identifiability corpus

Tasks 2 through 5 already contain repeated examples of a common structure:

```text
too coarse
→ dangerous evasion / semantic collapse

too fine
→ legitimate behavior becomes impossible / false positive

accepted boundary
→ minimum currently defensible distinction

residual coarsening
→ explicitly named cases still collapsed
```

Examples include:

- Annex-B `"command"` exemption;
- composition-root carve-out;
- §8.6 defaults exemption;
- `GAUNTLET_RESULT_PATH` exclusion;
- policy-stem contract;
- F1 command-shape rule;
- E4 timeout classification.

These are better described as **adjudicated distinction cases** than as ground truth.

Why:

- an adjudication can be faithful to a defective reference;
- AK-049 proves reference defect is possible;
- disagreement can therefore have three loci:
  1. automated procedure error;
  2. adjudication error;
  3. reference error.

This makes the corpus more valuable, not less. An automated disagreement search can become a **reference-audit mechanism** and candidate generator.

---

# 18. DomainForge canonical-middle-layer hypothesis

This remains a research hypothesis, not a Gauntlet implementation requirement.

## 18.1 Basic model

Let the consequential latent purposeful system be \(Z\).

Observed evidence arrives through heterogeneous projections:

\[
X^{(j)}=P_j(Z)+\epsilon_j
\]

Examples:

- stories;
- epics;
- workflows;
- policies;
- code;
- tests;
- traces;
- interviews;
- interfaces.

Reverse semantic modeling produces candidate canonical model \(G\):

\[
R:\{X^{(j)}\}_{j\in T}ightarrow G
\]

Forward projections generate:

\[
\hat{X}^{(j)}=D_j(G)
\]

## 18.2 Non-identifiability

A user story is generally a lossy projection. Distinct real systems can emit the same story.

Therefore reverse modeling does not uniquely recover reality from a single lossy source.

The honest output is an **identified set / equivalence class** until discriminating evidence arrives.

`UNKNOWN` is therefore not merely modesty. It can represent genuine non-identifiability.

## 18.3 Corrected canonical target

A useful candidate formal target is an indexed consequential quotient, strengthened toward stochastic bisimulation:

\[
G^*_{\mathcal U,\mathcal J,Y}
\]

The index matters:

- \(\mathcal U\) = intervention class;
- \(\mathcal J\) = required projection/predicate set;
- \(Y\) = consequential variable(s).

There is no universal canonical representation independent of what distinctions matter for the intended interventions and consequences.

## 18.4 Practical sufficiency test

Do not attempt to estimate high-dimensional Shannon mutual information at Tier 0.

Instead freeze a decoder class \(\mathcal V\) and compare held-out loss:

\[
L_{\mathcal V}(Y\mid G)
\]

versus:

\[
L_{\mathcal V}(Y\mid G,X)
\]

If giving the decoder the raw evidence in addition to \(G\) materially improves prediction, then \(G\) omitted usable consequential information for that decoder/task.

---

# 19. Simulation decomposition

A clean separation remains:

```text
DomainForge
→ lawful possibility / semantic structure

RealityTrace
→ empirical dynamics / external consequence

Simulator
→ executable transition model

Gauntlet
→ governed experiment and settlement
```

A simulator can be represented as:

\[
M_G=(S,A,T_G,O,Gv)
\]

DomainForge may constrain:

- states \(S\);
- actions \(A\);
- governance \(Gv\);
- lawful/support structure of transitions.

It cannot infer empirical probabilities merely from semantic correctness.

Those require observed reality, experiments, or explicit assumptions.

## 19.1 Simulation ladder

From cheapest to strongest:

1. structural simulation;
2. scenario simulation;
3. population/operational simulation;
4. agent-in-the-loop simulation;
5. counterfactual simulation.

Counterfactual claims require causal identification, not merely replay.

---

# 20. Capability formation as discovery-to-discrimination conversion

The bootstrap process contains a measurable candidate capability signal even before Task 11:

```text
failure class discovered
→ predicate extracted
→ permanent deterministic check
→ teeth
→ future validation becomes cheaper
```

Candidate dispositions for discovered defect classes:

- **Converted** - became a permanent deterministic discriminator;
- **Documented** - accepted known limitation / false-positive shape;
- **Deferred** - real but trigger-bound or currently uneconomic;
- **Adjudicated non-defect** - argued not to be a defect under current reference.

A raw conversion ratio can be computed, but should not be Goodharted. Not every discovered defect deserves mechanization.

A better long-run question is:

> Of the defect classes worth mechanizing, how many became reliable reusable discriminators before consequential recurrence?

## 20.1 Recurrence-after-conversion

This may be more informative than raw conversion rate.

If a converted check catches only the original payload but misses structurally novel variants, then the organization memorized an example rather than acquiring defect-class discrimination.

Analogy to metabolization:

```text
verbatim repeat improves
but novel same-class does not
→ memoization

novel same-class becomes cheaper/reliable
→ capability
```

---

# 21. Observer portfolio value is submodular

The marginal value of another observer depends on what the existing portfolio already covers.

If A and B detect the same defect classes with the same blind spots, adding B contributes little.

If C covers an otherwise uncovered consequential class, C may be more valuable even with lower standalone sensitivity.

Therefore the orchestration question becomes:

> **Which additional observer most expands the current observability horizon for the claim/problem at hand?**

not:

> How many critics should run?

Exploratory observers remain necessary because deterministic observers cover known classes. The exploratory observer is pointed toward the unknown region and can turn an unknown-unknown into a known-unknown that can later be mechanized.

---

# 22. Epistemic throughput and adjudication capacity

This is the strongest new systems-level constraint from the discussion.

## 22.1 The architecture can manufacture uncertainty faster than it resolves it

Let:

\[
\lambda
=
	ext{rate at which well-formed adjudication candidates arrive}
\]

and:

\[
\mu
=
	ext{rate at which authoritative adjudications are completed}
\]

Then backlog approximately evolves as:

\[
Q_{t+1}=\max(0,Q_t+\lambda_t-\mu_t)
\]

If:

\[
E[\lambda]>E[\mu]
\]

then unresolved representational debt grows without bound even if every upstream mechanism works correctly.

The failure would look like rigor:

- more honest UNKNOWN states;
- more candidate revisions;
- more observer disagreements;
- better claim typing;
- better provenance;
- larger ledgers.

Then settlement eventually blocks because the contested distinctions were never adjudicated.

## 22.2 Binding currency

For this layer the binding payment currency may be:

> **operator adjudication attention**

not tokens, GPU, memory, or wall-clock.

This is an important CognitiveOS projection: a path can be technically possible and epistemically visible while remaining unaffordable because authoritative adjudication capacity is exhausted.

---

# 23. Candidate-adjudication governor

The current residual ledger already demonstrates a useful control pattern:

```text
residual discovered
→ record it
→ do not spend attention immediately
→ future task enters its trigger
→ residual becomes blocking
→ diagnose/pay now
```

This trigger rule has already fired multiple times.

A candidate projection for reference candidates is:

```text
reference candidate discovered
→ record
→ map dependency footprint
→ remain dormant while no active settlement depends on it
→ activate when a dependent Unit/capability approaches settlement
→ adjudication becomes blocking
```

## 23.1 Required representation

A flat candidate list will eventually become a backlog.

The useful structure is a dependency graph:

```text
CandidateRevision
→ contested distinction
→ affected Units
→ affected verification claims
→ affected capabilities
→ migration/revalidation surface
```

## 23.2 Timing qualification

“Wait until settlement” is not always optimal.

Some candidates have growing delay cost, especially when immutable history is being written or downstream migration cost compounds.

Candidate priority may therefore depend on:

- dependency reach;
- settlement proximity;
- delay cost;
- irreversibility;
- recovery/migration cost.

This is still the same CognitiveOS rule:

> surface uncertainty early without promoting it early.

## 23.3 Diagnostic/adjudication budget

CognitiveOS Math Layer already bounds diagnostic loops by:

- test count;
- payment fraction;
- planning-cycle time.

A future candidate-adjudication process could project the same invariant upward, but this should be proposed and tested rather than inserted into Gauntlet by convenience.

---

# 24. Potential CognitiveOS canonical revision candidates

None of these are canon merely because this discussion found them persuasive.

## R-1: refine diagnostic failure classes

Candidate additions:

- observer perception error;
- observer reference error;
- observer inference error;
- observer evidence-entailment error;
- observer coverage error;
- explicit reference-defect / reference-divergence class.

**Activation criterion:** at least several real divergence events can be cleanly distinguished by these subclasses, including at least one reference event such as AK-049.

**Disconfirmation:** if the subclasses cannot be distinguished reliably or do not change the cheapest corrective action, the refinement is decorative.

## R-2: define corroboration independence

Candidate change to settlement reliability:

- require lineage declarations for corroborating signals;
- treat undeclared independence conservatively;
- distinguish implementation diversity from failure-mechanism independence;
- eventually calibrate correlated blindness empirically.

**Activation criterion:** before multi-observer agreement is used materially to raise settlement reliability or capability promotion in a multi-agent environment.

**Disconfirmation:** lineage/correlation-aware corroboration does not predict downstream reliability better than the simpler current rule.

## R-3: claim typing for observations

Candidate rule:

> observers declare claim type before observation; evidence must be appropriate to that claim type.

**Activation criterion:** repeated mis-typing or overstatement appears in operational records, or observation records become first-class runtime objects.

**Disconfirmation:** claim typing adds cost without reducing overclaiming, incorrect settlement, or diagnostic ambiguity.

## R-4: bounded observability horizon

Candidate construct:

> observability is estimated over a bounded declared claim distribution, not “all knowable facts.”

**Activation criterion:** when observer portfolios become numerous enough that orchestration must choose which observers to run.

**Disconfirmation:** the construct does not improve observer selection, failure diagnosis, or cost-adjusted settlement.

## R-5: epistemic-throughput / adjudication-capacity projection

Candidate construct:

> authoritative adjudication attention is an explicit payment currency and candidate backlogs are governed by dependency-triggered activation.

**Activation criterion:** candidate backlog starts growing or any Unit approaches settlement with unresolved reference candidates in its dependency cone.

**Disconfirmation:** candidate generation remains below adjudication capacity without governance, or the governor causes more delay/payment than it saves.

---

# 25. Potential Gauntlet action register

Current operator posture: **do not disturb the active build merely because these ideas exist.** The Gauntlet continues on its existing spec/plan unless an activation condition below becomes true and the proper authority/revision process is used.

## G-A1 - Correct bounded-search wording

**Type:** documentation/claim-discipline action  
**Status:** potential, low-risk  
**Activation criteria:**

- a maintained status/handoff document is being edited anyway; or
- bounded-search claims are about to be used as evidence for promotion/settlement.

**Action:**

Replace universal/minimality wording with explicitly bounded claim forms unless the search space/candidate class was actually exhaustive and declared.

**Do not activate if:** it would distract an active implementation task merely to polish historical prose.

**Settlement:** fresh reader can distinguish witness-established, bounded-search, and universal claims without inference.

---

## G-A2 - File attribution/reference candidate for append-only history

**Type:** CandidateRevision/reference candidate, not implementation change  
**Activation criteria:**

- Task 6 begins writing durable append-only records whose attribution cannot later be recovered; and
- existing schema/reference does not preserve attribution required for later settlement claims; and
- the Jolli incident remains relevant evidence.

**Action:**

File a reference candidate describing the attribution gap, delay cost, competing designs, and affected Units. Do not add an `actor` field by fiat.

**Settlement:** candidate accepted/rejected through normal revision process with migration/recovery consequences stated.

---

## G-A3 - Candidate-to-Unit dependency mapping

**Type:** governance/representation candidate  
**Activation criteria:**

- reference-candidate count > 1; or
- any Unit approaches formal settlement while an open candidate may affect it; or
- candidate backlog is no longer trivially inspectable.

**Action:**

Represent which Units/claims depend on each contested distinction.

**Settlement:** the system can answer “does this open candidate block this Unit?” without manual full-ledger rereading.

---

## G-A4 - Adjudication trigger/budget policy

**Type:** CandidateRevision / governance projection  
**Activation criteria:**

- active candidate arrival rate approaches operator adjudication capacity; or
- candidate backlog grows across multiple tasks; or
- settlement is blocked by unresolved candidates.

**Action:**

Project the demonstrated residual-trigger rule into candidate adjudication, with explicit treatment of delay/irreversibility cost.

**Do not activate if:** candidate volume remains tiny and manual adjudication is cheaper.

**Settlement:** candidate backlog does not grow uncontrollably and no dependent Unit silently settles through a live contested distinction.

---

## G-A5 - E−1 blind-critic calibration

**Type:** research experiment  
**Activation criteria:**

- critic non-findings will be used as material support for “no evasion found,” minimality, or vacuity claims; or
- before E0 uses critic-hardened adjudication cases as its evidence corpus; or
- before critic agreement is given settlement-reliability weight.

**Action:**

Run paired positive/negative controls using re-injected historical defects and adjudicated non-defects.

**Settlement:** sensitivity/specificity by defect class are measured for a frozen critic configuration.

**Failure response:** downgrade only critic-dependent claims; deterministic gates/teeth remain intact.

---

## G-A6 - Discovery-to-discrimination conversion audit

**Type:** analysis, not runtime modification  
**Activation criteria:** anytime the build is paused enough to inspect historical data without affecting implementation.

**Action:**

Classify discovered defect classes as Converted / Documented / Deferred / Adjudicated non-defect; measure conversion latency and recurrence-after-conversion.

**Settlement:** a report distinguishes true defect-class mechanization from one-off payload memorization.

---

## G-A7 - Minimal Observer profile/Observation record

**Type:** architecture candidate  
**Activation criteria:**

- Task 9 observers or Task 11 verifier need first-class observer provenance beyond existing records; or
- E−1 produces calibration data that needs a durable home.

**Minimum fields:**

```text
ObserverProfile:
  observer_id
  observer_version
  declared_scope
  known_unavailables
  lineage
  calibration_status

Observation:
  observer_ref
  target_ref
  reference_ref
  claim
  claim_type
  evidence_ref
  observed_scope
```

**Do not activate if:** existing Core-spec records already satisfy the actual settlement need.

**Settlement:** observer claims can be traced to declared scope and calibration state without invented numeric reliability.

---

## G-A8 - Shared-evidence fan-out scrutiny at Task 11

**Type:** verification discipline  
**Activation criteria:** Task 11 verification/evidence binding exists and a single evidence artifact is cited by multiple Units.

**Action:**

Treat fan-out as a scrutiny trigger, not a defect. Check entailment per Unit and issue separate valid VerificationRecords.

**Settlement:** no Unit receives PASS merely because another Unit legitimately cites the same artifact.

---

## G-A9 - RealityTrace/external-consequence requirement

**Type:** architecture dependency / research boundary  
**Activation criteria:**

- any claim moves from “conforms to frozen reference” to “corresponds to external reality”; or
- E5 predictive simulation is attempted; or
- self-improvement/structural revision is to be judged against operational consequence.

**Action:**

Require an external consequence observation path independent of the acting/conformance stack.

**Important:** external consequence still requires an independently anchored purpose/normative criterion to determine whether the consequence was desirable.

**Settlement:** at least one coherent-but-wrong-reference scenario can be detected because external consequence and desired outcome disagree with the internally valid reference.

---

## G-A10 - Preserve current build order and proof boundaries

**Type:** standing guardrail  
**Activation:** always.

Do not:

- retrofit Observer theory into current tasks without a reference-backed need;
- treat Task closure as Unit settlement;
- change the ruler;
- add new dimensions because they seem cheaper “now”;
- claim Gauntlet capability before Task 11/20/26/27 evidence;
- let this research dossier outrank the Core spec.

---

# 26. CognitiveOS research/action register

## C-A1 - Keep Observer-as-role projection as an extension candidate

**Activation criteria:** another materially different domain (outside Gauntlet software critique) exhibits the same target/reference/evidence/claim grammar and the abstraction improves diagnosis.

**Settlement:** at least two or three domains reuse the contract without domain-specific special pleading.

---

## C-A2 - Test claim typing

**Activation criteria:** enough observation records exist to compare typed vs untyped reporting.

**Test:** does predeclared claim typing reduce overclaiming, incorrect PASS, or ambiguous diagnostic closure?

**Retire if:** it adds bureaucracy without measurable epistemic benefit.

---

## C-A3 - Evaluate R-1 observer-error classifier

**Activation criteria:** collect several real cases for each candidate error class.

**Test:** blind diagnosis using the refined classifier versus current failure-mode table.

**Retire if:** the refinement does not improve failure-locus accuracy or correction selection.

---

## C-A4 - Evaluate R-2 corroboration independence

**Activation criteria:** E−1 produces paired observer outcomes across defect classes.

**Test:** compare raw agreement count vs lineage/correlation-aware estimate for downstream correctness.

**Retire if:** richer independence modeling does not improve reliability prediction.

---

## C-A5 - Measure epistemic throughput

Candidate quantities:

\[
\lambda_D=	ext{new distinctions discovered}
\]

\[
\lambda_C=	ext{valuable distinctions mechanized}
\]

\[
\lambda_Q=	ext{new unresolved candidate questions}
\]

\[
\mu_A=	ext{authoritative adjudications completed}
\]

**Activation criteria:** enough events exist for rates to be meaningful, or backlog visibly appears.

**Failure signatures:**

### Artisanal cognition

\[
\lambda_D\gg\lambda_C
\]

The organization repeatedly rediscovers and re-reasons the same failure classes.

### Epistemic paralysis

\[
\lambda_Q^{active}\gg\mu_A
\]

The organization produces uncertainty faster than it can settle it.

---

## C-A6 - Preserve the Purpose → Reference → Effective → Instrumented → Consequence chain

**Activation criteria:** next revision of CognitiveOS diagnostic/settlement theory.

**Question:** does explicitly separating these loci improve diagnosis over current categories?

**Retire if:** the extra distinctions cannot be operationally discriminated.

---

## C-A7 - Test observability horizon

**Activation criteria:** observer selection itself becomes a routing problem.

**Bounded reference:** define a claim distribution for the domain.

**Measure:** what fraction/diversity of consequential claims have spendable, sufficiently reliable observation paths?

**Retire if:** it does not improve observer selection or expose meaningful zero-coverage regions.

---

# 27. DomainForge / capability-formation research ladder

This ladder is intentionally separate from the active Gauntlet build.

## A - Adjudication governor

**Activation:** before scaling any experiment that generates many reference candidates.

## B - Claim-type retrofit

**Activation:** when maintaining the relevant status/research artifacts; no experiment required.

## C - Conversion-rate / recurrence audit

**Activation:** anytime historical ledger extraction is affordable.

## E−1 - Critic calibration

**Activation:** before critic absence/minimality claims become load-bearing or before E0.

## E0 - Identifiability / disagreement screen

Use existing adjudicated distinction cases rather than authoring only synthetic worlds.

Primary output:

> disagreement set + diagnostic locus, not a simplistic accuracy score.

A disagreement may indicate:

- procedure error;
- adjudication error;
- reference error;
- unresolved ambiguity.

## E0b - Identification efficiency

Give competing procedures the same priced evidence menu.

Ask:

> Which method shrinks the identified set most per unit of evidence-acquisition payment?

## E1 - Structural simulation

Project `.sea` into deterministic structural checks:

- reachability;
- deadlock;
- unauthorized transitions;
- policy contradiction;
- evidence-free settlement;
- failure with no recovery.

This is the cheapest simulation layer and does not require empirical transition probabilities.

## E2 - Payment-aware cognition routing

On the real Jetson pipeline, compare:

```text
always-largest-model
vs
cheapest-sufficient cognition routing
```

with fixed quality gates.

Measure:

- settlement quality;
- tokens;
- wall time;
- peak memory;
- model swap/load cost;
- energy where available.

## E3 - Burden/metabolization trajectory

### Runtime level

Activation: Gauntlet Task 11+ with actual settlement records.

Measure whether search/validation burden falls on **novel same-class** work with accuracy/recovery/gate integrity intact.

### Bootstrap-organization level

Historical interruption/recovery data may generate a hypothesis, but it cannot settle runtime capability. Keep observer levels separate.

## E4 - Six-cell semantic-formation benchmark

Factor representation, evidence, and control.

The decisive comparison is:

```text
raw + observed evidence + Gauntlet
vs
canonical DomainForge + observed evidence + Gauntlet
```

If the canonical layer adds no payment-adjusted value once observation and governance exist, the strong middle-layer claim loses.

## E5 - Predictive simulation validity

Activation:

- RealityTrace or equivalent external consequence observation exists;
- held-out trajectories can be frozen;
- empirical dynamics are available.

Compare DomainForge-constrained simulation against simpler baselines at equal compute.

Do not make counterfactual claims without causal identification.

---

# 28. Kill criteria worth preserving

## DomainForge canonical-middle-layer claim

Retire/restrict the strong claim if, across materially different domains:

- held-out projection reconstruction does not beat lighter structured baselines;
- consequence prediction does not improve;
- payment-adjusted settlement does not improve;
- raw + observation + governance performs as well as canonical + observation + governance.

DomainForge may remain valuable as a DSL, code generator, governance representation, or projection engine even if the deeper claim fails.

## Observer-governance extensions

Retire/restrict if:

- claim typing does not reduce overclaiming or diagnostic ambiguity;
- calibration is too unstable across observer versions to be useful;
- lineage-aware independence predicts no better than raw agreement;
- observer metadata creates more false certainty than it removes;
- O2 adds cost without improving detection of observer/reference/evidence errors.

## Adjudication governor

Retire/restrict if:

- candidate backlog never becomes consequential;
- trigger/dependency bookkeeping costs more than direct manual adjudication;
- delay-cost decisions become more complex than the disputes themselves.

---

# 29. What not to build or promote yet

Keep dormant unless evidence activates them:

- a rich numeric Observer reliability schema;
- automatic settlement-confidence weighting from uncalibrated observers;
- a universal “meta-observer” service;
- a CapabilityPackage object distinct from existing skill + ledger structures;
- full probabilistic organizational simulation;
- agent society simulation;
- counterfactual simulation without causal identification;
- automatic structural revision;
- automatic reference adjudication;
- learned burden/correlation weights at Tier 0;
- a new Gauntlet surface or field merely because retrofit cost will be higher later;
- canonical CognitiveOS edits based only on conceptual elegance.

---

# 30. Watch list as the Gauntlet continues

These events have unusually high information value.

## W1 - AK-049 adjudication

When the first reference candidate is accepted or rejected, record:

- decision;
- authority;
- rationale;
- evidence;
- whether held-out consequence was used;
- migration implications;
- whether the original Unit/ruler changes between runs.

This will be the first actual test of the **exit** from the reference-candidate state.

## W2 - Next internally consistent reference defect

AK-049 was detectable because of internal conflict.

Watch for a requirement that is:

- coherent;
- implementable;
- verifiable;
- but later shown wrong by external consequence or purpose.

That is the stronger GSR test.

## W3 - Task 11 promotion pressure

Watch whether provisional Units receive genuinely per-Unit evidence binding or whether shared rationale begins to substitute for entailment.

## W4 - Critic yield vs critic calibration

Do not read declining critical count as maturity until critic sensitivity and task-class confounds can be bounded.

## W5 - Defect-class recurrence after mechanization

When a supposedly converted failure class returns through a novel shape, record whether the check learned the class or only the original witness.

## W6 - Candidate backlog

Watch:

```text
candidate arrival
candidate activation
adjudication completion
```

If active candidates grow faster than they clear, operator attention has become the binding currency.

## W7 - Genuine bootstrap-to-runtime transfer

A real field-to-core event would require:

1. a bootstrap failure not already covered by the frozen plan/spec;
2. a candidate runtime requirement traceable to that failure;
3. promotion through normal reference-revision governance.

The existing crash-recovery symmetry does **not** qualify because crash injection was already in the plan.

## W8 - Task 20 self-hosting

This is the first point where Gauntlet starts governing its own continued build. Keep bootstrap-organization evidence separate from runtime evidence.

## W9 - Task 26 baselines

This is the first direct test of whether full Gauntlet earns its payment against simpler orchestration profiles.

If full Gauntlet loses to a simpler profile, simplify.

## W10 - External consequence coverage

Do not let internal conformance evidence masquerade as evidence that the reference is correct about the external world.

---

# 31. Compact architecture

The current synthesis can be compressed to:

```text
PURPOSE / DESIRED CONSEQUENCE
          ↓
FROZEN REFERENCE
          ↓
TRANSFORM
          ↓
FINAL ARTIFACT / EFFECTIVE STATE
          ↓
OBSERVE
          ↓
TYPED CLAIM + EVIDENCE
          ↓
COMPARE
          ↓
VERIFY ENTAILMENT
          ↓
SETTLE / RESIDUAL / UNKNOWN
          ↓
DIAGNOSTIC CLOSURE
          ↓
CORRECT LOCALLY
      or
CANDIDATE REVISION
          ↓
ADJUDICATION GOVERNOR
          ↓
STRUCTURAL REVISION BETWEEN RUNS
          ↓
EXTERNAL CONSEQUENCE
          ↓
PURPOSE / NORMATIVE CHECK
          ↓
DEVELOPMENTAL MEMORY
          ↓
REPEATED VARIATION
          ↓
METABOLIZED CAPABILITY
```

The Observer role can enter at multiple points.

The important rule is:

> **No participant gains epistemic authority merely because it emits a claim. Its claim must be typed, scoped, evidenced, and appropriate to the observer's actual discrimination capability.**

---

# 32. The strongest conceptual conclusions

## 32.1 Gauntlet is not trying to install a final source of truth

The architecture increasingly assumes that any layer can be wrong:

```text
reference can be wrong
implementation can be wrong
gate can be wrong
critic can be wrong
verifier can be wrong
instrument can be wrong
environment can change
purpose can be misrepresented
```

The goal is not to find the one component that cannot fail.

The goal is to prevent any one component from having enough unchecked authority to manufacture certainty.

## 32.2 Observation is a governed capability

The clean scalable abstraction is:

> **Any system that claims to observe something enters the same observation domain and must earn the authority associated with that claim type.**

This applies locally to one critic and globally to an organization of heterogeneous observers.

## 32.3 Exploration and discrimination form an epistemic developmental loop

```text
exploratory cognition
→ novel failure distinction
→ bounded claim
→ deterministic discriminator
→ teeth
→ cheap future validation
```

Where this happens successfully under variation, expensive judgment is converted into reusable observational substrate.

## 32.4 Epistemic flow must be governed, not just epistemic quality

A system can fail by:

- seeing too little;
- believing too much;
- mechanizing too slowly;
- or generating well-formed uncertainty faster than it can adjudicate it.

The last failure mode is easy to mistake for rigor.

## 32.5 Reality is still the final external adversary

Frozen references, calibrated observers, deterministic gates, and strong verification can prove increasingly strong **internal conformance** claims.

They cannot by themselves establish that a coherent reference correctly represents the external world or the desired consequence.

For that, the system must eventually close the loop through:

```text
external consequence
+
independent desired-direction / normative criteria
```

---

# 33. Source basis

This dossier was distilled from the following source classes:

## Active Gauntlet sources

- `CURRENT_STATUS.md`, 2026-08-17
- `Core-spec.md`, Gauntlet Runtime Specification v2
- `gauntlet-impl.md`, Implementation Plan - Reference-Anchored Gauntlet Runtime
- `AGENTS.md`

## CognitiveOS sources

- Cognitive Operating System Manifesto v13.1
- CognitiveOS Mathematical Specification Layer v2.3
- CognitiveOS Computational Theory of Metabolization / Transformation-Route Measurement Theory
- Governed Structural Revision Theory v0.1

## Research synthesis sources

- *GodSpeed Capability Formation Architecture & Falsification Program* v0.1
- Addendum A - Status Delta, Three Deflations, and E−1
- Addendum B - Conceded Corrections, the Observer Role, and a Canonical Defect in ρ
- Addendum C - Claim Typing, the Reference Blind Spot, and the Constraint Nobody Has Priced
- the discussion that produced and critiqued those documents

---

# 34. Current disposition

**No implementation action is required merely because this document exists.**

The active Gauntlet should continue against its frozen reference and implementation plan.

Use this dossier as:

- a durable research handoff;
- a candidate-action registry;
- a watch list;
- a source for future CandidateRevisions;
- a set of activation criteria;
- a guard against accidentally promoting attractive interpretations into canon.

The next thing that matters is not adding more architecture.

It is watching which of these distinctions **actually become consequential as Gauntlet continues to build and begins to settle its own claims.**
