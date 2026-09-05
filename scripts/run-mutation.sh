#!/usr/bin/env bash
# run-mutation.sh — AK-070's mutation-teeth runner (plan Task 24, phase 6).
#
# Reads .agents/reports/defended-mechanisms.txt (the machine-readable teeth
# map) and runs cargo-mutants scoped to the DEFENDED CODE, testing every
# mutant against the `antigaming` suite. Exits non-zero unless every
# examined mutant is caught (missed == 0) — the plan gate's
# `assert d['missed']==0`.
#
# Recorded divergences from the plan's gate spelling (the charter's
# recon fact 4 class; both recorded at closure):
#
# 1. THE PLAN'S `cargo mutants --file-list <manifest>` DOES NOT EXIST:
#    cargo-mutants 27.1.0 has no --file-list option. The real surface is
#    repeatable -f/--file globs plus -F/--examine-re regexes over the
#    names `--list` shows. This script reads the manifest and scopes per
#    file with -f.
# 2. SCOPE = THE DEFENDED LOCI, NOT WHOLE FILES. AK-070's claim is that
#    the proof/anti-gaming suites detect defects in the MECHANISMS THEY
#    GUARD — not that the suites catch every mutant of twelve whole
#    source files (most of which guard nothing this suite claims). Each
#    manifest entry's source_location line sits inside the defended
#    function; the SCOPE table below records, per entry, the mutant
#    line-span (or function pattern) examined, in the same
#    `path:span|exclude` shape, mechanically applied with -F/-E. Excluded
#    mutants (if any) carry their justification as a comment right in
#    defended-mechanisms.txt — the manifest stays the single source of
#    truth.
# 3. The per-mutant test run is scoped to `--test antigaming`:
#    AK-058's claim is about THIS suite (the deep proofs
#    g6/g7/g10/p1..p12 are separate settled targets), and this is the
#    honest minimal proof of the teeth. RECORDED DIVERGENCE (the recon
#    fact 4 class): the plan gate's `antigaming::` positional does NOT
#    work on this nextest — a positional containing `::` parses as the
#    filter DSL's package selector, matches no test (the binary name is
#    not a `::`-component of the nextest test id: ids display as
#    `gauntlet-cli::antigaming <fn>`), and a silently-empty run would
#    report EVERY mutant "caught" through the no-tests error. The
#    binary-scoped `--test antigaming` runs exactly the suite and is the
#    gate the plan's filter spelling intended.
# 4. cargo-mutants PINS the per-mutant test invocation to the mutated
#    package (`--package=<mutant-pkg>@<version>`), but the `antigaming`
#    target lives in gauntlet-cli. Without appending `-p gauntlet-cli` to
#    the test args, nextest finds no such target in the mutated package
#    and fails with "no tests to run" — reporting EVERY mutant "caught"
#    through a run in which ZERO tests executed. The appended `-p
#    gauntlet-cli` puts the suite's target in scope for every shard
#    (verified: the Test phase runs the 18 tests, ~10s, and fails with
#    nextest's test-failure exit under a live mutant).
#
# Operational discipline (AGENTS.md + task24 addenda): the whole run
# takes the repository compile lock once (flock); sccache is stopped
# inside the same flock scope at exit; cargo-mutants runs at -j 2
# (memory-constrained host); mutants.out is scratch and never committed.
#
# Re-runnable: shards run in the order below; outcomes accumulate in
# .agents/reports/task24/mutation-scratch/ and the merged
# .agents/reports/mutation.json is rewritten at the end.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

MANIFEST=".agents/reports/defended-mechanisms.txt"
OUT_JSON=".agents/reports/mutation.json"
SCRATCH_DIR=".agents/reports/task24/mutation-scratch"
LOCK=".agents/reports/task24/compile.lock"
JOBS="${MUTATION_JOBS:-2}"

# ---------------------------------------------------------------------------
# The scope table: one row per shard.
#   file|span_start:span_end|exclude_regex(optional)
# Spans are the --list line ranges of the defended function that the
# manifest entry names; each derives from the entry's source_location.
# ---------------------------------------------------------------------------
SCOPE_TABLE=(
  # G1 contract.rs:347 — the rubric-coverage loop inside ComparisonContract::parse,
  # WIDENED (cor1, F-4) to the full parse epilogue including the
  # supported-version gate at 307 (whose `!=` -> `==` mutant the g1
  # test's valid-contract assertion catches).
  "crates/gauntlet-app/src/compare/contract.rs|307:360|"
  # G2 provider.rs:318 — verify_unchanged digest compare; G10 provider.rs:267 — freeze tripwire
  "crates/gauntlet-app/src/reference/provider.rs|260:325|"
  # G3 producer_authority.rs:701 + G12 :715 — SettlementCommitAuthority::admit
  "crates/gauntlet-ports/src/producer_authority.rs|681:790|"
  # G4 promotion.rs:486 — settled_with_real_variation. EXCLUDED: the
  # `-> bool with false` body mutant can only OVER-refuse variation (the
  # fail-safe direction) — it can never let a false promotion through, so
  # it is outside the anti-gaming claim. Recorded per the charter's
  # manifest-recorded-justification rule.
  "crates/gauntlet-app/src/memory/promotion.rs|489:501|settled_with_real_variation -> bool with false"
  # G5 gate.rs:798 — regression_verdict (was 780; the cor2 E-2 discharge
  # above it shifted lines +18 — addresses re-pinned to the final tree)
  "crates/gauntlet-app/src/promote/gate.rs|799:818|"
  # G6 observation_digest.rs:29 — the digest/normalization pair (the file
  # IS the mechanism the manifest line names)
  "crates/gauntlet-ports/src/observation_digest.rs|29:80|"
  # G7 posture.rs:124 — review_lattice_movement. EXCLUDED: exactly the
  # two `<` -> `<=` replacement mutants at 136:42 and 139:50 —
  # EQUIVALENT, because each arm evaluates only when its own axis MOVED
  # (`!=`), so `to != from` on every evaluation makes `<=` coincide with
  # `<`; the no-movement side is unwrap_or(false). The other four
  # replacements at those positions (`==` and `>` twice) are NOT
  # equivalent and are examined: AK-9505 / AK-9504 / AK-9506 of the g7
  # suite test catch them (justified in the manifest).
  "crates/gauntlet-app/src/govern/posture.rs|130:150|posture.rs:136:42: replace < with <=|posture.rs:139:50: replace < with <="
  # G8 aggregate.rs:383 — the verdict rule (the blocking-forces-fail
  # line). NARROWED to the manifest locus: the dispute/incomplete-gate
  # bookkeeping at 381/385 is outside the G8 claim (justified in the
  # manifest; covered by the compare module's own aggregate unit tests).
  "crates/gauntlet-app/src/compare/aggregate.rs|383:390|compare/aggregate.rs:38[15]:"
  # G11 comparator.rs:1486 — assert_blind (+ its visit fn)
  "crates/gauntlet-ports/src/comparator.rs|1487:1510|"
  # G13 delta.rs:159 — settlement_access_verdict
  "crates/gauntlet-app/src/promote/delta.rs|160:180|"
  # G14 developmental.rs:304 — record_recovery's earned-credit guards
  "crates/gauntlet-app/src/memory/developmental.rs|304:306|"
  # G15 report/mod.rs:485 — classify_unit's proven-pair computation.
  # EXCLUDED: `report/mod.rs:493:57` (`&&` -> `||` in the proven
  # predicate) is only distinguishable in a store state the settlement
  # authority cannot produce (COR-3 moors every committed row's
  # verification ref to a durable PASS record), so no honest fixture can
  # turn it RED. Recorded per the manifest-recorded-justification rule.
  "crates/gauntlet-app/src/report/mod.rs|491:495|mod.rs:493:57"
  # G16 gate.rs:263 — the re-ask floor: the sweep bound in adjudicate +
  # earlier_record_failures (was 842:912; cor2's E-2 discharge shifted
  # lines +18 — re-pinned). The second span is the FULL fn body
  # (widened by cor1, F-4: includes the margin-chain and payment-floor
  # interior mutants, each caught by a g16 leg).
  "crates/gauntlet-app/src/promote/gate.rs|280:292|"
  "crates/gauntlet-app/src/promote/gate.rs|860:930|"
)

# ---------------------------------------------------------------------------
# Take the repository compile lock ONCE for the whole run (the task24
# operational addendum: one compile at a time; flock is this script's own
# top-level scope; sccache is stopped inside the same scope at exit).
exec 9>"$LOCK"
flock 9
trap 'sccache --stop-server >/dev/null 2>&1 || true' EXIT

command -v cargo-mutants >/dev/null 2>&1 || { echo "cargo-mutants is not installed" >&2; exit 2; }
[[ -f "$MANIFEST" ]] || { echo "manifest missing: $MANIFEST" >&2; exit 2; }

MANIFEST_SHA256="$(sha256sum "$MANIFEST" | cut -d' ' -f1)"
RUN_INVOCATION="scripts/run-mutation.sh (cargo-mutants $(cargo mutants --version 2>/dev/null | awk '{print $2}'), --no-config --baseline skip -j ${JOBS} --test-tool nextest --no-default-features -f <defended file> -F <scope span> [-E <justified exclusion>] -- -p gauntlet-cli --test antigaming)"

rm -rf "$SCRATCH_DIR"
mkdir -p "$SCRATCH_DIR"
# mutants.out is cargo-mutants' fixed output dir; it is scratch.
rm -rf mutants.out

SHARD_INDEX=0
: > "$SCRATCH_DIR/shards.jsonl"

for row in "${SCOPE_TABLE[@]}"; do
  file="${row%%|*}"; rest="${row#*|}"
  span="${rest%%|*}"; exclude="${rest#*|}"
  span_start="${span%%:*}"; span_end="${span##*:}"
  [[ -f "$file" ]] || { echo "scope file missing: $file" >&2; exit 2; }

  # The examine regex: exact list-name lines inside the span.
  pattern=":($(seq "$span_start" "$span_end" | paste -sd'|')):"

  shard_args=(-f "$file" -F "$pattern")
  if [[ -n "$exclude" ]]; then
    shard_args+=(-E "$exclude")
  fi

  echo "== shard $((SHARD_INDEX + 1)): $file [$span] ${exclude:+(excluding: $exclude)} =="
  # A shard's cargo-mutants exits non-zero when mutants are missed; the
  # per-shard exit is RECORDED and the run continues — the merge at the
  # end is the single authority for the gate (missed == 0).
  shard_exit=0
  cargo mutants \
    --no-config \
    --baseline skip \
    -j "$JOBS" \
    --test-tool nextest \
    --no-default-features \
    "${shard_args[@]}" \
    -- -p gauntlet-cli --test antigaming || shard_exit=$?
  echo "shard $((SHARD_INDEX + 1)) cargo-mutants exit: $shard_exit"

  # Collect the shard's outcomes, then clear the scratch.
  python3 - "$file" "$span" "$exclude" "$shard_exit" "$SCRATCH_DIR/shards.jsonl" <<'PYEOF'
import json, pathlib, sys
file, span, exclude, shard_exit, sink = sys.argv[1:6]
outcomes = json.loads(pathlib.Path("mutants.out/outcomes.json").read_text())
record = {"file": file, "span": span, "exclude": exclude, "shard_exit": int(shard_exit), "mutants": outcomes}
with open(sink, "a") as handle:
    handle.write(json.dumps(record) + "\n")
PYEOF
  rm -rf mutants.out
  SHARD_INDEX=$((SHARD_INDEX + 1))
done

# ---------------------------------------------------------------------------
# Merge: .agents/reports/mutation.json — the deliverable AK-070 cites.
# ---------------------------------------------------------------------------
GENERATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

python3 - "$OUT_JSON" "$MANIFEST_SHA256" "$GENERATED_AT" "$RUN_INVOCATION" "$SCRATCH_DIR/shards.jsonl" <<'PYEOF'
import json, pathlib, sys

out_path, manifest_sha256, generated_at, invocation, shards_path = sys.argv[1:6]

# cargo-mutants 27.1.0 outcomes.json: {"outcomes": [{"summary":
# "CaughtMutant"|"MissedMutant"|"UnviableMutant"|"TimeoutMutant"|...,
# "scenario": {"Mutant": {"name": "path:line:col: ...", ...}}, ...}]}
def normalize(summary):
    state = summary.strip().lower()
    if state.endswith("mutant"):
        state = state[: -len("mutant")]
    return {"caught": "caught", "missed": "missed", "unviable": "unviable",
            "timeout": "timeout", "skipped": "skipped",
            "notcovered": "not covered"}.get(state, "unknown")

counts = {"missed": 0, "caught": 0, "unviable": 0, "timeout": 0, "skipped": 0, "not covered": 0}
shards = []
missed_names = []
total = 0
for line in pathlib.Path(shards_path).read_text().splitlines():
    if not line.strip():
        continue
    record = json.loads(line)
    # cargo-mutants 27.1.0 writes outcomes.json as a DICT whose
    # "outcomes" member is the per-mutant list (the shard record stores
    # that whole document).
    document = record.pop("mutants")
    outcomes = document["outcomes"] if isinstance(document, dict) else document
    shard = dict(record)
    shard["examined"] = len(outcomes)
    shard["statuses"] = {}
    for outcome in outcomes:
        state = normalize(outcome.get("summary", "unknown"))
        counts[state] = counts.get(state, 0) + 1
        shard["statuses"][state] = shard["statuses"].get(state, 0) + 1
        if state == "missed":
            missed_names.append(outcome.get("scenario", {}).get("Mutant", {}).get("name", "unknown mutant"))
    total += len(outcomes)
    shards.append(shard)

document = {
    "total": total,
    "missed": counts.get("missed", 0),
    "caught": counts.get("caught", 0),
    "unviable": counts.get("unviable", 0),
    "timeout": counts.get("timeout", 0),
    "skipped": counts.get("skipped", 0),
    "not_covered": counts.get("not covered", 0),
    "shards": shards,
    "missed_mutants": missed_names,
    "manifest_sha256": manifest_sha256,
    "generated_at": generated_at,
    "invocation": invocation,
    "scope_note": (
        "Examined mutants are the defended loci from "
        ".agents/reports/defended-mechanisms.txt (span table in "
        "scripts/run-mutation.sh); advisory entries (G9) have no mutation "
        "scope by the honest-boundary rule. Justified exclusions are "
        "recorded next to their entries in the manifest and applied as "
        "-E regexes."
    ),
}
pathlib.Path(out_path).write_text(json.dumps(document, indent=2) + "\n")

print(
    f"mutation run: total={document['total']} caught={document['caught']} "
    f"missed={document['missed']} unviable={document['unviable']} "
    f"timeout={document['timeout']}"
)
for name in missed_names:
    print(f"MISSED: {name}")
sys.exit(0 if document["missed"] == 0 else 1)
PYEOF
