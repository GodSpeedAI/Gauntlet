# Task 10 static audit — D1–D3

**Scope/method.** Read-only inspection of the current worktree, the D1–D3
checkpoint claims, builder brief, plan, frozen-reference material, and Rust
source/tests. No compilation or tests were invoked. Later Task-10 slices are
present in the same worktree, so this audit does not treat current line counts
as point-in-time D1–D3 counts or independently confirm any reported green
result.

## Result: NOT READY to treat all D1–D3 claims as proven

### Blocking: comparison criteria are not frozen with the reference

Spec §9.5 requires the comparison contract to be frozen with the reference and
says a mid-run rubric change is ruler movement (`.agents/specs/Core-spec.md:669-671`).
D2's new artifact only *states* this in a comment
(`.gauntlet/comparison-contract.yaml:1-8`; checkpoint `builder-checkpoints.md:86-92`).
The actual frozen lock records only the answer-key and manifest digests
(`.gauntlet/reference.lock:1-10`), and the manifest's only `source_refs` entry
is the Core spec (`.wayfinder/reference.yaml:15-17`). Neither names nor
digests `comparison-contract.yaml`.

The implementation likewise loads an arbitrary path at use time
(`crates/gauntlet-app/src/compare/contract.rs:344-350`); the typed value carries
only a version string, dimensions, and rubric (`:114-118, 337-341`), and the
envelope takes that caller-supplied value (`compare/envelope.rs:86-105`). A
same-version contract/rubric replacement is therefore neither pinned to a
`ReferenceVersion` nor detected. This contradicts the D2 “frozen-with-the-
reference” claim and leaves an I1/ruler-movement path. It needs a recorded
reference/candidate decision and a freeze/binding design, rather than a header
comment.

### Major: two required contract declarations are accepted then discarded

§9.5 requires the artifact to declare what comparators receive and what counts
as a residual (`Core-spec.md:669-671`). The parser stores only item *counts*
for `comparator_receives` and `residual_definition`
(`compare/contract.rs:145-150, 199-200, 309-325`); `ComparisonContract` and
`to_contract_view` discard their text entirely (`:114-140`). Thus an artifact
whose declarations disagree with the actual envelope is accepted as long as
both lists are nonempty. No static test demonstrates their values are checked
against the §13.2 allowlist or used to determine residuals. D2’s “strict …
typed ComparisonContract” claim is consequently limited to syntax/presence,
not the required comparison semantics.

### Major: P2 check proves banned key spellings, not banned content/provenance

The envelope exposes raw caller-controlled strings for `reference_view`,
constraints, and residual detail (`compare/envelope.rs:86-105`; constraint
strings are unconstrained at `gauntlet-ports/src/comparator.rs:440-465`).
They are serialized verbatim (`comparator.rs:548-565`). `assert_blind` only
searches for twelve JSON **key** spellings (`compare/envelope.rs:258-304`).
Therefore builder reasoning/identity/etc. can be placed in an allowed field's
string value and pass the proof. The unit test checks a claims canary absent
from the reduced observation (`:398-420`) and poisons by adding a banned key
(`:522-540`); neither exercises this value-channel counterexample.

This does not refute the narrower claims-facet exclusion: reduction through
`VerificationInputs` is visible at `:151-190`. It does mean D2's stronger
“EXACTLY allowlist / no extra context” and AK-026 readiness are unproven until
inputs are provenance-bound/typed or the stated guarantee is explicitly
limited to field shape.

### Major: D3 silently uses `observed`, not required `observed_locator`

The frozen spec and D3 brief require merge key
`(dimension, expected, observed_locator)` (`Core-spec.md:943-947`; builder
brief `:143-144`). The checkpoint instead claims `(dimension, expected,
observed)` (`builder-checkpoints.md:142-144`), and code implements precisely
that key (`compare/aggregate.rs:306-323`). Annex B.4 exposes an `observed`
field (`Core-spec.md:1956-1962`), but neither the codec nor the checkpoint
records a reconciliation that defines it as a locator. This changes duplicate
identity when observations have the same rendered text but different locators.
It requires an explicit frozen-reference ambiguity/candidate decision, or a
schema/type that retains the locator; it should not be silently substituted.

### Moderate: D1 shared-suite digest assertion is weaker than claimed

The D1 brief requires the returned digest to equal the envelope's **serialized
digest** (`builder-brief.md:103-104`). The shared-suite fixture assigns a
literal digest without deriving it from `to_json` (`ports/contract/comparator.rs:93-116`),
then only compares the returned field to that literal (`:133-180`). The D2
builder unit test does recompute a builder-produced envelope
(`app/compare/envelope.rs:422-437`), but the D1 shared contract suite does not
prove the stated serialized-digest coupling for arbitrary port adapters.

### Moderate: D1 contains explicitly acknowledged next-slice scope creep

D1 records adding D5-only `ComparatorError::TimedOut`/`ContractMismatch`
(`builder-checkpoints.md:13-25`) and a new D5 adapter workspace member
(`:66-71`; `Cargo.toml:30`). The brief assigns the model adapter to D5
(`builder-brief.md:166-194`), not D1. The new member also caused a global
architecture-policy exception for bare `model` (`denylist.rs:83-92, 228-248`).
The rationale may be sound, but it is a cross-cutting policy and slice-scope
change, not the D1 domain/ports deliverable. It needs explicit orchestrator
approval/recording separate from the implementation checkpoint.

### Minor: D3 test inventory is internally inconsistent

D3 says “Tests (6)” but lists five names (`builder-checkpoints.md:163-168`).
Current `aggregate.rs` has five `#[test]` functions (`:535, 656, 690, 749,
787`); the reported app count rises 152→157, also consistent with five. Amend
the checkpoint; static review cannot verify the claimed gate outputs
(`builder-checkpoints.md:170-171`).

## Frozen-reference mutation check

No direct edit to `.wayfinder/**` or `.agents/specs/Core-spec.md` is present in
the current worktree (`git diff -- .wayfinder .agents/specs` was empty). D1's
extra `envelope_digest` wire field is explicitly sanctioned by the D1 brief
(`builder-brief.md:71-75`) despite Annex B.4 omitting it. The problem above is
not a direct frozen-file edit; it is the unbound new comparison-criteria
artifact and the unrecorded `observed_locator` reconciliation.
