# CandidateRevision: `evidence_required.producer: instrument` ambiguity

Carried forward, unresolved, from
`.agents/reports/task26-followup-independent-verification.md` §10. This
repair did not touch it — it does not block D+critic apparatus
readiness (no admission, dispatch, recording, or isolation path this
repair verified depends on its resolution) — and per this task's own
instruction it is recorded here rather than silently reinterpreted.

## The question

Does `evidence_required.producer: instrument` (Core-spec §14.3, §5.3.2)
constrain:

A. the producer of the BYTES at the evidence locator;
B. the process that originally produced the underlying evidence;
C. the observation that corroborated the artifact;
D. the producer of the deciding `VerificationRecord`;
E. some explicitly defined combination of the above?

## Why it remains open

Core-spec §14.3's exact text: *"the evidence's producing instrument
matches `evidence_required.producer`"* — paired, in the same rule list,
with a distinct condition about the evidence ref resolving. This reads
as attaching to the evidence AT THE LOCATOR (readings A/B), and is
checked as a PRECONDITION of `VerificationRecord` validity — so it
cannot coherently denote the `VerificationRecord` itself (reading D):
a record cannot be a precondition of its own validity, and the sibling
`evidence_producer_mismatch` outcome class would then be unreachable.

The previous (invalid) follow-up's adjudication argued for reading D
("the verification record that satisfies settlement is produced by the
verification instrument... NOT by the builder"), which the independent
verifier correctly identified as a REDEFINITION of the term rather than
a reading of §14.3, made unfalsifiable by construction (every
`VerificationRecord` is instrument-produced under that reading, by
definition).

## The concrete chain this repair observed (not adjudicated)

On the `gadget-task`/`gadget-task-dcritic` fixtures' honest-success
scenario — the only leg that settles — `scripts/check.sh`'s own
documented behavior: the builder child writes the evidence report at its
declared locator on the honest shapes; `check.sh` only writes its OWN
report there "WHEN AND ONLY WHEN no produced artifact stands there." So
the bytes the sole settlement in this fixture family rests on are
builder-authored, under units declaring `producer: instrument`. This
repair did not change this behavior, does not adjudicate whether it is
conformant, and flags it here exactly as the independent verifier's
report already did.

## Disposition

**SPEC AMBIGUITY / CandidateRevision** — recommend Core-spec §14.3 be
clarified to state explicitly which of A–E it means, and that the
evidence store's provenance recording for builder-written locator bytes
that an instrument corroborates (rather than originates) be audited
against whichever reading is chosen. Not resolved by, and not blocking,
this apparatus repair.
