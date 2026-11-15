#!/usr/bin/env python3
"""Release-qualification record tooling (WP-62). Standard library only.

  qualify.py record    <steps.tsv> <out.json> key=value ...
      Build one per-platform qualification record from the step ledger that
      packaging/qualify-local.sh writes (name<TAB>status<TAB>reason lines).
  qualify.py notes --json qualification.json [--attested true]
      Print the release-notes readiness block (between readiness markers).
  qualify.py aggregate --platforms <platforms.json> --records <dir>
                       [--dist-sums SHA256SUMS] [--attested true] --version V --commit SHA
                       --out-json qualification.json --out-md QUALIFICATION.md
                       [--signing-note TEXT] [--provenance-note TEXT]
      Collect every record under <dir> into qualification.json and a
      QUALIFICATION.md readiness table. Exit 0 always; the verdict is the
      "release_ready" field (every claimed platform is QUALIFIED).

Statuses of a platform: QUALIFIED, QUALIFIED (tarball), PARTIAL, NOT QUALIFIED.
  QUALIFIED (tarball): a no-sudo run (--mode tarball); the package install and
  upgrade steps are NOT_RUN. Never counts as a full QUALIFIED for release_ready.
  QUALIFIED      no step FAILed and every REQUIRED step PASSed.
  PARTIAL        no step FAILed, but a REQUIRED step was NOT_RUN.
  NOT QUALIFIED  a step FAILed, the record is missing, or (Jetson) there is
                 no valid real-hardware evidence.
Provenance and upgrade may be NOT_RUN without blocking QUALIFIED; both are
always listed with their reason.
"""
import glob
import json
import os
import sys

REQUIRED = ["checksums", "install", "tarball_install", "smoke", "governed_run", "uninstall"]
SCHEMA = "gauntlet.qualification/1"


TARBALL_REQUIRED = ["checksums", "tarball_install", "smoke", "governed_run", "uninstall"]


def verdict(steps, platform_steps=None, mode="package"):
    required = platform_steps or (TARBALL_REQUIRED if mode == "tarball" else REQUIRED)
    by = {s["name"]: s["status"] for s in steps}
    if any(s["status"] == "FAIL" for s in steps):
        return "NOT QUALIFIED"
    for name in required:
        st = by.get(name)
        if st is None:
            return "PARTIAL"
        if st == "NOT_RUN":
            return "PARTIAL"
    return "QUALIFIED (tarball)" if mode == "tarball" else "QUALIFIED"


def cmd_record(argv):
    tsv, out = argv[0], argv[1]
    meta = dict(a.split("=", 1) for a in argv[2:])
    steps = []
    with open(tsv, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line:
                continue
            name, status, reason = (line.split("\t") + ["", ""])[:3]
            steps.append({"name": name, "status": status, "reason": reason})
    artifacts = []
    for item in filter(None, meta.get("artifacts", "").split(";")):
        name, _, sha = item.partition("=")
        artifacts.append({"name": name, "sha256": sha})
    mode = meta.get("mode", "package")
    required = [r for r in meta.get("required", "").split(",") if r] or (TARBALL_REQUIRED if mode == "tarball" else REQUIRED)
    rec = {
        "schema": SCHEMA,
        "platform": meta["platform"],
        "evidence": meta.get("evidence", "hosted-runner"),
        "version": meta["version"],
        "commit": meta["commit"],
        "runner_image": meta.get("runner_image", ""),
        "host": {"os": meta.get("os", ""), "arch": meta.get("arch", ""), "detail": meta.get("detail", "")},
        "artifacts": artifacts,
        "steps": steps,
        "required_steps": required,
        "mode": mode,
        "status": verdict(steps, required, mode),
    }
    with open(out, "w", encoding="utf-8") as fh:
        json.dump(rec, fh, indent=2, sort_keys=True)
        fh.write("\n")
    print("qualification record: %s -> %s" % (rec["platform"], rec["status"]))


def load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def record_problem(rec, platform_id, version, sums):
    """Return None when the record may count, else the reason it is rejected.

    Every record is checked against the release's own SHA256SUMS (when given):
    evidence applies to the exact artifacts it tested, never to a rebuild.
    The Jetson row additionally needs real-hardware evidence.
    """
    if rec.get("schema") != SCHEMA:
        return "record schema is not " + SCHEMA
    if rec.get("platform") != platform_id:
        return "record platform is %s, expected %s" % (rec.get("platform"), platform_id)
    if rec.get("version") != version:
        return "record is for version %s, release is %s" % (rec.get("version"), version)
    if platform_id == "linux-arm64-jetson" and rec.get("evidence") != "real-hardware":
        return "Jetson evidence must be real-hardware, record says %s" % rec.get("evidence")
    if sums:
        recorded = rec.get("artifacts", [])
        if not recorded:
            return "record names no artifacts"
        for a in recorded:
            if sums.get(a["name"]) != a["sha256"]:
                return "stale evidence: %s sha256 differs from this release's SHA256SUMS" % a["name"]
    return None


def table_lines(rows):
    md = ["| Platform | Status | Evidence | Runner image | Notes |", "|---|---|---|---|---|"]
    for r in rows:
        notes = "<br>".join(n.replace("|", "/") for n in r["notes"]) or "all steps PASS"
        md.append("| %s | **%s** | %s | %s | %s |" % (
            r["label"], r["status"], r["evidence"] or "none", r.get("runner_image") or "n/a", notes))
    return md


def cmd_notes(argv):
    """Print the readiness block for the release notes (between markers)."""
    opts = {}
    for i in range(0, len(argv), 2):
        opts[argv[i].lstrip("-")] = argv[i + 1]
    q = load(opts["json"])
    rows = q["platforms"]
    ready = q["release_ready"]
    attested = opts.get("attested", "false") == "true"
    out = ["<!-- readiness:begin -->", "## Platform readiness", ""]
    if ready and attested:
        out.append("Every platform claimed as supported is QUALIFIED (record: `qualification.json`).")
    else:
        out.append("**NOT production-ready: this is a PRERELEASE.** At least one claimed platform is not QUALIFIED"
                   + ("" if attested else ", or this run did not attest the tagged commit")
                   + "; nothing below is presented as production-ready without its record.")
    out.append("")
    jet = [r for r in rows if r["id"] == "linux-arm64-jetson"][0]
    if jet["evidence"] != "real-hardware":
        out.append("**Jetson: NOT QUALIFIED (no real-hardware evidence)** - hosted ubuntu-22.04-arm results "
                   "qualify linux-arm64 (generic) only. See docs/how-to/qualify-a-release.md to add a record.")
        out.append("")
    elif jet["status"] != "QUALIFIED":
        out.append("**Jetson: %s** (real-hardware record present; not a full QUALIFIED)." % jet["status"])
        out.append("")
    out.extend(table_lines(rows))
    out.append("")
    out.append("Signing: **unsigned** (no Apple Developer ID, no Authenticode).")
    if attested:
        out.append("")
        out.append("<!-- provenance:attested -->")
    out.append("<!-- readiness:end -->")
    print("\n".join(out))


def cmd_aggregate(argv):
    opts = {}
    i = 0
    while i < len(argv):
        opts[argv[i].lstrip("-")] = argv[i + 1]
        i += 2
    platforms = load(opts["platforms"])["platforms"]
    records = {}
    for path in sorted(glob.glob(os.path.join(opts["records"], "**", "qualification-*.json"), recursive=True)):
        rec = load(path)
        records[rec["platform"]] = rec
    dist_shas = {}
    if "dist-sums" in opts and os.path.isfile(opts["dist-sums"]):
        with open(opts["dist-sums"], encoding="utf-8") as fh:
            for line in fh:
                parts = line.strip().split(None, 1)
                if len(parts) == 2:
                    dist_shas[parts[1].lstrip("*")] = parts[0]
    rows = []
    for p in platforms:
        pid = p["id"]
        rec = records.get(pid)
        row = {"id": pid, "label": p["label"], "claimed": p.get("claimed", True),
               "evidence": None, "status": "NOT QUALIFIED", "notes": []}
        if rec is not None:
            problem = record_problem(rec, pid, opts["version"], dist_shas)
            if problem:
                row["notes"].append("record rejected: " + problem)
                rec = None
        if pid == "linux-arm64-jetson" and rec is None:
            row["notes"].append("NOT QUALIFIED (no real-hardware evidence)")
        if rec is not None:
            row["evidence"] = rec.get("evidence")
            row["status"] = rec["status"]
            row["runner_image"] = rec.get("runner_image", "")
            row["artifacts"] = rec.get("artifacts", [])
            row["steps"] = rec.get("steps", [])
            for s in rec.get("steps", []):
                if s["status"] != "PASS":
                    row["notes"].append("%s: %s (%s)" % (s["name"], s["status"], s.get("reason", "")))
        elif pid != "linux-arm64-jetson":
            row["notes"].append("no qualification record was produced (job failed before recording, or the runner label was rejected)")
            if p.get("unavailable_reason"):
                row["notes"].append(p["unavailable_reason"])
        rows.append(row)
    claimed = [r for r in rows if r["claimed"]]
    ready = bool(claimed) and all(r["status"] == "QUALIFIED" for r in claimed)
    out = {
        "schema": "gauntlet.qualification-set/1",
        "version": opts["version"],
        "commit": opts["commit"],
        "release_ready": ready,
        "attested": opts.get("attested", "false") == "true",
        "signing": opts.get("signing-note", "unsigned (no signing configured)"),
        "provenance": opts.get("provenance-note", ""),
        "platforms": rows,
    }
    with open(opts["out-json"], "w", encoding="utf-8") as fh:
        json.dump(out, fh, indent=2, sort_keys=True)
        fh.write("\n")
    md = []
    md.append("# Release qualification: Gauntlet %s" % opts["version"])
    md.append("")
    md.append("Commit `%s`. Signing: %s." % (opts["commit"], out["signing"]))
    if out["provenance"]:
        md.append(out["provenance"])
    md.append("")
    md.extend(table_lines(rows))
    md.append("")
    md.append("Overall: **%s**" % ("every claimed platform is QUALIFIED" if ready else
              "NOT production-ready: at least one claimed platform is not QUALIFIED"))
    md.append("")
    md.append("## Step matrix")
    md.append("")
    names = []
    for r in rows:
        for s in r.get("steps", []):
            if s["name"] not in names:
                names.append(s["name"])
    md.append("| Platform | " + " | ".join(names) + " |")
    md.append("|---|" + "---|" * len(names))
    for r in rows:
        by = {s["name"]: s["status"] for s in r.get("steps", [])}
        md.append("| %s | %s |" % (r["label"], " | ".join(by.get(n, "-") for n in names)))
    with open(opts["out-md"], "w", encoding="utf-8") as fh:
        fh.write("\n".join(md) + "\n")
    print("\n".join(md))


if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in ("record", "aggregate", "notes"):
        sys.stderr.write(__doc__)
        sys.exit(2)
    {"record": cmd_record, "aggregate": cmd_aggregate, "notes": cmd_notes}[sys.argv[1]](sys.argv[2:])
