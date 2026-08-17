-- Migration 0001 — the durability spine (spec §6.2 EventLog, §16.3).
--
-- One table, append-only: the adapter issues INSERT and SELECT against it
-- only. Offsets are INTEGER PRIMARY KEY AUTOINCREMENT, so they are strictly
-- increasing and never reused — the monotonicity the port contract pins.
-- `kind` is denormalized beside the canonical JSON payload so replay filters
-- (settlement recovery) never parse to select.
CREATE TABLE event_log (
    offset  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id  TEXT    NOT NULL,
    kind    TEXT    NOT NULL,
    payload TEXT    NOT NULL,
    at      INTEGER NOT NULL
);
