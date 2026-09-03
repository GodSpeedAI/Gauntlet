# Task 21 — COR-3 brief (builder, FINAL class-closing sweep)

You are building the final correction slice for plan Task 21 at
/home/sprime01/projects/gauntlet. Tree: HEAD a71eba9 + staged COR-1 +
unstaged COR-2 (leave staging as found; stack your edits on top; do
NOT commit). Read first: `.agents/reports/task21/cor3-brief.md` (this
file), `.agents/reports/task21/falsifier-round3.md` (R3-F-1/2/3 with
executed repros — the class specification), `AGENTS.md`.

## The defect class (three rounds of evidence)

Dynamic untrusted strings (argv remnants, ticket ids, operator
identities, queue-file stems, error details) interpolated RAW into
stderr/stdout output lines. Where the string can carry a newline, it
forges arbitrary following lines, including §20.3-shaped audit lines.
Where it can only carry control bytes, it still corrupts/misleads the
audit and inbox surfaces. Locus-by-locus fixes (COR-2 + addendum) left
siblings raw: `escalations_cmd.rs` dispatcher unknown-subcommand
(:34), `list` unknown-argument (:59), quarantined-stem print
(:98-105), list record lines (:114-133), and `operator_action.rs`
accepted arm (:117-124) + `escalations_cmd.rs` acceptance line
(:270-274). Full repros: R3-F-1/2/3.

## The contract — close the CLASS, not the loci

1. **One escaping discipline for the family.** Introduce a single
   helper (e.g. `fn log_quote(&str) -> String` — debug-style escaping;
   or consistent `{:?}` application via the helper) in the
   escalations/operator-action command family, and apply it to EVERY
   dynamic untrusted string in every stderr/stdout output path of
   `crates/gauntlet-cli/src/escalations_cmd.rs` and
   `crates/gauntlet-cli/src/operator_action.rs`: pre-parse refusal
   arguments, dispatcher unknown-subcommand/argument lines, list output
   (stems, ticket ids, operator fields, summaries — already escaped),
   acceptance lines, and all three §20.3 arms. Mechanical audit
   required: grep/read every `println!/eprintln!/format!` in both
   files and account for each dynamic interpolation in your report
   (escaped / trusted-constant / why-raw-is-correct). The human-facing
   NON-log consumers from COR-2's recorded decision (the CLI failure
   detail line, JSON 500 body) stay verbatim — the invariant below
   must accommodate them (they are not §20.3-shaped and the detail is
   a store error string; record the boundary).
2. **Enumeration-free teeth (the load-bearing deliverable).** New
   integration test(s) beside `operator_actions_no_bypass.rs` (or
   extending it) that:
   - seed hostile records THROUGH THE QUEUE'S OWN APIs plus argv: a
     ticket id with ESC, an operator identity with a newline
     (`--operator $'op\noperator-action: …'` through the sanctioned
     resolve — the r3 argv-reachable vector), a quarantined queue file
     whose stem carries a newline + forged §20.3 line, a hostile
     unknown subcommand/argument;
   - drive: `escalations list`, a resolve that gets REFUSED, one that
     FAILS (queue-read failure), one that ACCEPTS (re-entry on a
     settled unit), and the dispatcher/unknown-argument paths;
   - assert the INVARIANT over the full captured stderr+stdout:
     (a) zero raw bytes < 0x20 except `\n` (0x0A) and `\t` (0x09);
     (b) the number of stderr lines starting with `operator-action:`
         equals the number of actions actually driven (forged §20.3
         lines break the count);
     (c) every legitimate line's dynamic fields land in the escaped
         form (greppable, human-readable — the r3-confirmed property).
   Failing-first: run the invariant against the CURRENT tree — it must
   be RED (r3's loci are live). Then fix, then GREEN. That RED run is
   your proof the teeth bite; include it verbatim in the report.
3. **No behavior change beyond output escaping.** Exit codes, refusal
   codes, durable writes, JSON API semantics: all byte-identical. The
   API's server-side §20.3 line (in `durable_surface.rs`/`serve_cmd.rs`
   paths) — check whether it interpolates any raw record field; if it
   feeds from the same untrusted records, include it and say so; if it
   is already covered by COR-1's ActionLogContext shape, say that.

## Gates (green on your final tree; one compiler at a time)

```
flock .agents/reports/task21/compile.lock -c '<cmd> && sccache --stop-server'
```

1. `cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass` (12/12 today)
2. `cargo nextest run -p gauntlet-cli --test cli_surface --test escalations_cli` (7/7 today)
3. `cargo nextest run -p gauntlet-adapter-surface-json` (11/11 today)
4. `cargo build -p gauntlet-cli --no-default-features`
5. `just lint && just boundaries && just ruler`
(Do NOT run the full `just test` — the orchestrator's closure battery
runs it.)

## Guardrails

- Do not touch `.wayfinder/`, `.gauntlet/`, `.agents/specs/`, `.jolli/`,
  either `cli_help*.txt` snapshot, or files outside the two command
  files + their tests (+ `durable_surface.rs` ONLY if the audit in
  item 3 finds it feeds raw record fields into a log sink — record it
  either way).
- Do not commit; leave staging as found. No new dependencies.
- Exit codes and refusal vocabularies are frozen by earlier rounds' tests.

## Result contract

`.agents/reports/task21/cor3-report.md`: the per-interpolation audit
table (file:line → escaped/trusted/why), the invariant test's RED
pre-fix + GREEN post-fix evidence, gate evidence post-last-edit,
deviations. Maintain `cor3-progress.md` declare-first. Final message:
completion + gate numbers + report path only.
