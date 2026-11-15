#!/usr/bin/env python3
"""Release-qualification record tooling (WP-62). Standard library only.

  qualify.py record    <steps.tsv> <out.json> key=value ...
      Build one per-platform qualification record from the step ledger that
      packaging/qualify-local.sh writes (name<TAB>status<TAB>reason lines).
  qualify.py aggregate --platforms <platforms.json> --records <dir>
                       [--jetson <record.json>] --version V --commit SHA
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


def jetson_valid(rec, version, dist_shas):
    """Return (ok, reason). Real hardware evidence for THESE artifacts only."""
    if rec.get("schema") != SCHEMA:
        return False, "record schema is not " + SCHEMA
    if rec.get("platform") != "linux-arm64-jetson":
        return False, "record platform is not linux-arm64-jetson"
    if rec.get("evidence") != "real-hardware":
        return False, "record evidence is not real-hardware"
    if rec.get("version") != version:
        return False, "record is for version %s, release is %s" % (rec.get("version"), version)
    if rec.get("status") not in ("QUALIFIED", "QUALIFIED (tarball)"):
        return False, "record status is %s" % rec.get("status")
    recorded = {a["name"]: a["sha256"] for a in rec.get("artifacts", [])}
    if not recorded:
        return False, "record names no artifacts"
    for name, sha in recorded.items():
        if dist_shas.get(name) != sha:
            return False, "stale evidence: %s sha256 differs from this release's asset" % name
    return True, "real-hardware record matches this release's arm64 artifacts"


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
        if pid == "linux-arm64-jetson":
            ext = opts.get("jetson")
            if ext and os.path.isfile(ext):
                ok, why = jetson_valid(load(ext), opts["version"], dist_shas)
                if ok:
                    rec = load(ext)
                else:
                    rec = None
                    row["notes"].append("Jetson record rejected: " + why)
            if rec is None:
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
    md.append("| Platform | Status | Evidence | Runner image | Notes |")
    md.append("|---|---|---|---|---|")
    for r in rows:
        notes = "<br>".join(n.replace("|", "/") for n in r["notes"]) or "all steps PASS"
        md.append("| %s | **%s** | %s | %s | %s |" % (
            r["label"], r["status"], r["evidence"] or "none", r.get("runner_image") or "n/a", notes))
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
    if len(sys.argv) < 2 or sys.argv[1] not in ("record", "aggregate"):
        sys.stderr.write(__doc__)
        sys.exit(2)
    {"record": cmd_record, "aggregate": cmd_aggregate}[sys.argv[1]](sys.argv[2:])
