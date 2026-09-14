-- Migration 0011 — evidence productions keyed per ITERATION (task26
-- correction; spec 14.3 condition 3, "produced in this run at or after the
-- transform it certifies"). 0006 keyed the table write-once per (run,
-- evidence ref): a re-production of byte-identical content at a LATER
-- iteration had no row of its own to land on, so the verification chain's
-- recency check read the ref's ORIGINAL iteration forever — a genuine,
-- freshly re-produced artifact verified stale (§14.3) even though the
-- instrument that made it had just run again, this iteration, and the
-- corroborating facet said so.
--
-- The row's IDENTITY was never the problem — the same evidence ref, the
-- same content digest, the same producer and locator ARE one fact, and a
-- differing one is still refused exactly as before. The problem was
-- collapsing every iteration's re-affirmation of that fact onto one row.
-- The fix: `iteration_no` joins the primary key. A re-production of
-- identical content at a NEW iteration is a NEW row (a new attempt's own
-- production, append-only, never overwriting the prior iteration's row);
-- the same iteration is the pre-existing idempotent no-op; an iteration at
-- or before an existing row for that ref is still refused (a stale or
-- replayed claim never re-certifies). The pre-crash row and a resumed
-- pass's own later-iteration production of the identical bytes now stand
-- SIDE BY SIDE, byte-level intact, exactly as the recovery forensics
-- require (COR-4 adjudication).
--
-- Existing rows carry their own `iteration_no` already (0006), so they
-- migrate unchanged, byte-level: same columns, same values, a widened
-- primary key, table rebuilt (SQLite cannot ALTER a PRIMARY KEY), old table
-- dropped, new table renamed. No row is edited, reordered, or lost.
CREATE TABLE evidence_productions_0011 (
    run_id           TEXT NOT NULL,
    evidence_ref     TEXT NOT NULL,
    producer_name    TEXT NOT NULL,
    producer_version TEXT NOT NULL,
    locator          TEXT NOT NULL,
    iteration_no     INTEGER NOT NULL,
    produced_at      INTEGER NOT NULL,
    event_log_offset INTEGER NOT NULL,
    content_digest   TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (run_id, evidence_ref, iteration_no)
);

INSERT INTO evidence_productions_0011 (
    run_id, evidence_ref, producer_name, producer_version, locator, iteration_no,
    produced_at, event_log_offset, content_digest
)
    SELECT run_id, evidence_ref, producer_name, producer_version, locator, iteration_no,
           produced_at, event_log_offset, content_digest
    FROM evidence_productions;

DROP TABLE evidence_productions;

ALTER TABLE evidence_productions_0011 RENAME TO evidence_productions;

CREATE INDEX evidence_productions_locator
    ON evidence_productions (run_id, locator, event_log_offset);

-- The freshest-row-per-ref lookup (`evidence_production`, singular) now
-- needs its own ordered index: multiple rows can share (run_id,
-- evidence_ref) across iterations.
CREATE INDEX evidence_productions_ref
    ON evidence_productions (run_id, evidence_ref, event_log_offset);
