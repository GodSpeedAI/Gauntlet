-- Migration 0005 — the durable run/iteration -> observation-bundle binding.
-- This is what terminates observation provenance in durable state: a pinned,
-- correctly hashed bundle is this run's observation because THIS row says so,
-- not because the document is well formed and names the run.
--
-- The primary key makes the binding write-once per (run, iteration): a second
-- bundle for the same iteration is a constraint violation the adapter reports,
-- never an overwrite, so a later substitute cannot displace the bundle a
-- comparison was opened over.
-- The binding records the bundle's CONTENT digest as well as its ref. A ref
-- alone terminates provenance only as far as the evidence store serving it is
-- honest; with the digest bound, the comparison port re-derives it from the
-- bytes it is handed and refuses anything that is not the recorded content.
CREATE TABLE observation_bindings (
    run_id            TEXT NOT NULL,
    iteration_no      INTEGER NOT NULL,
    evidence_ref      TEXT NOT NULL,
    observation_digest TEXT NOT NULL,
    PRIMARY KEY (run_id, iteration_no)
);
