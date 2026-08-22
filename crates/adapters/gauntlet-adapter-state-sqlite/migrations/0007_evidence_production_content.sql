-- Migration 0007 — the production record's CONTENT digest (Task 11
-- correction round 1, B-M4). The 0006 record named the evidence ref but
-- nothing bound the examined bytes to the pinned bytes: the instrument
-- verified the LOCATOR's bytes while the record and pin named the STORE
-- ref. This column carries the digest of the bytes the producer ingested
-- at the locator, so the verification chain can re-derive the digest of
-- whatever the store serves at the ref and refuse bytes that are not the
-- recorded content (the ObservationBinding content-digest discipline,
-- mirrored on the production surface).
--
-- DEFAULT '' exists only to satisfy ALTER TABLE on rows written between
-- 0006 and this migration (none exist: Task 11 has no production writer
-- yet); the strict decoder refuses an empty digest as Corrupt, never a
-- degraded record.
ALTER TABLE evidence_productions
    ADD COLUMN content_digest TEXT NOT NULL DEFAULT '';
