# Task 21 — falsifier round 1 adjudication (orchestrator)

Falsifier: blind executing, fresh instance. Verdict: **NOT_CONFIRM
(0 blocking / 0 major / 3 minor / 4 informational)**. Report:
`falsifier-round1.md`. Per the standing semantics, 0 blocking / 0 major
means no full re-round is owed; the minors and the two commissioned-
surface residues get one correction slice (COR-1), then a focused
round 2 (re-attack the corrections + regression sweep, no full
re-commission).

| Finding | Sev | Decision | Route |
|---|---|---|---|
| F-1 re-entry queue-write failure claims "the settle committed" when that call committed nothing | MINOR | **CORRECT** — misleading failure locus (§21.2); re-entry message must name the already-durable truth | COR-1 + test |
| F-4 raw ANSI escapes from the request target land in the access log | MINOR | **CORRECT** — escape/debug-format the target like the §20.3 action log already does | COR-1 + test |
| F-5 unit read on a non-durable `?run=` serves 200 with default `status:"open"` while the run view 404s the same anchor | MINOR | **CORRECT** — unit-scoped read refuses with the run view's own `not_found` when the anchored run is not durable (the falsifier's fix shape); actions already refuse `run_not_durable`, so this is read-parity only | COR-1 + test |
| F-2 forgery ladder refused at every stage; raw-durable-store staging is the honest boundary | INFO | **RECORD AT SETTLEMENT** — the closure restates the boundary (durable store = runtime's sole write domain, §16.4) | closure |
| F-3 COR-F1 world cannot be ridden by the operator path (no durable PASS record exists to ride) | INFO | **RECORD AT SETTLEMENT** — defense in depth confirmation | closure |
| F-6 coarse denial heuristic + silent empty-units degradation | INFO | **PARTIALLY CORRECT** — the matcher/doc half is RES-04, already in COR-1; the empty-units ambiguity is the established read posture, recorded | COR-1 (matcher) + closure (ambiguity) |
| F-7 accept-loop message says "continuing" while it breaks | INFO | **CORRECT** — same locus as RES-03; classification fix covers message + doc + behavior | COR-1 |
| bind residue: any-resolved-address loopback check but binds the FIRST address (untriggered locally, no DNS control) | INFO | **CORRECT** — commissioned D9 surface (refuses non-loopback binds); require ALL resolved addresses loopback | COR-1 |
| `units//resolve` 405-mislabeled-as-GET (empty unit segment) | INFO | **RECORD ONLY** — cosmetic; empty-segment requests are malformed anyway | closure |

## Combined COR-1 scope (with the critic adjudication)

F-1, F-4, F-5, F-7, bind-all-addresses, RES-01 (D7 log-field parity),
RES-03 (accept-error classification), RES-04 (matcher/doc drift),
RES-05 (field-order comment), RES-08 (stale header), RES-10 (typed 400
on non-string note), RES-11 (feature-edge comment), RES-02's disclosure
half (acceptance output names settled unit and resolved ticket as
separate disclosed facts; the binding itself stays ports-revision-bound).

## Executed-evidence notes for the record

- The falsifier independently confirmed the positive path, exactly-once,
  idempotent replay, ambiguous/corrupt ticket refusals, the
  queue-write-failure window's honest recovery, the refusal-path
  zero-write audit (file hashes + sqlite diffs), read/write ticket
  racing (typed refusals only), HTTP cap/method/identity/body handling,
  §21.3 surface-failure isolation, feature topology (both help
  snapshots byte-identical), retry token semantics (D5), derivation
  steering refusal (`run_not_durable`), and bind policy on every
  directly-specifiable address.
- Its probe bug (Content-Length > body) produced a bonus executed
  datum: the per-request deadline closes the connection and the server
  keeps serving.
