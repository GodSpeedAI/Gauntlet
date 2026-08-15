# Environment fingerprint — Task 0

Machine-readable version: `.agents/reports/environment.json`. This is the human summary.

## Host

Linux (WSL2, kernel `6.18.35.2-microsoft-standard-WSL2`), x86_64, 6 cores, 7.8 GiB RAM.
**Memory is tight**: only ~1.6 GiB was available at fingerprint time with a bare shell —
cargo compiles under load and multiple concurrent builder invocations each triggering a
Rust compile are a real OOM risk on this host. This is a live constraint, not
theoretical: keep `max_parallel` conservative (spec default of 2 is appropriate here, not
a number to raise casually) and prefer serial or lightly-parallel builder dispatch for
Rust-compiling slices until incremental caches are warm. 761 GiB free disk — not a
constraint.

## Toolchain — present and working

`rustc`/`cargo` 1.92.0, `rustup` 1.29.0 (stable toolchain active and default),
`rust-analyzer` 1.92.0, `just` 1.58.0, `git` 2.53.0, `sqlite3` CLI 3.46.1, `python3`
3.14.4, `node` v24.19.0, `bun` 1.3.14, `uv` 0.12.1, `gh` 2.46.0.

## Toolchain — missing, installation in progress

`cargo-nextest`, `cargo-deny`, `cargo-mutants` were absent. All three are required by the
plan's gates (`just test`, `just deny`, Task 24 mutation testing / AK-070). None have a
sound long-term fallback — `cargo test` can stand in for `cargo-nextest` temporarily but
the plan's `just test` target and AGENTS.md both specify nextest explicitly, so a
fallback would be a recorded deviation, not a silent substitution. Installed via
`cargo install <name> --locked` in the background during Task 0; final pass/fail status
recorded in `environment.json` once resolved (network reachability to crates.io is
already proven by these installs progressing).

## Sandbox backends

`bubblewrap` (bwrap 0.11.1) is present and smoke-tested successfully:
`bwrap --ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent -- echo
bwrap-ok` → `bwrap-ok`. No AppArmor profile registration was found at the usual sysfs
path (unsurprising under WSL2), but bwrap's own user-namespace isolation does not depend
on that and worked directly. No container runtime (`docker`, `podman`) and no
hardware-isolated microVM backend (`qemu`, `firecracker`) are installed.

**Consequence, recorded honestly per spec Annex F and the Task 14 redesign trigger:**
`bwrap` is a real backend, so `contained` and `local` tiers get genuine containment. For
`cross_cutting`, spec Annex F's "typical" backend is a microVM or the strongest available
container tier — neither exists here, so `bwrap` (the strongest *available* real backend)
is what Task 14 will wrap `cross_cutting` invocations with, and this weakening will be
stated plainly in `docs/CONFORMANCE.md` at Task 27, not hidden behind an "it's sandboxed"
claim. This is not a redesign trigger under Task 14's own text ("if no real sandbox
backend exists on this host, do not fake containment... refuse dispatch") because a real
backend *does* exist — it is simply not container/VM-grade. `irreversible` gets no policy
file, as required, regardless.

## Reference profile

Rust, per spec §24.2's default profile — confirmed rather than asked, because (a) the
toolchain is fully present and working, so the spec's own redesign trigger ("no Rust
toolchain and no way to install one") does not fire, and (b) this run is explicitly
unattended (operator instruction: proceed without waiting for approval between tasks).
No deviation to record.

## Delegation mechanism (Task 0, step "delegation spike")

Empirically verified this session — see `.agents/reports/delegation-spike.md` for the
full method and evidence. Finding: this harness's delegation (`Agent` tool, and by its
own documented contract the `Workflow` tool) is **admission-based**, matching the
Prime-Agent behavior the spec's D1 correction and Annex C were written against. No
adaptation to the `AgentRunner` port design is required before Task 4. No blocker.

## Directory skeleton and `.gitignore`

Created per spec §25 in this same Task 0 pass (see commit). `.jolli/`, `.claude/`,
`.gemini/`, `.serena/`, `.mcp.json`, `.tmp/` are pre-existing session/tooling artifacts
unrelated to the Gauntlet build (Jolli memory plugin, IDE/agent tooling config, Serena
project files) and are left untouched; `.gitignore` accounts for build output and local
state without disturbing them.

## Operator confirmation

No interactive operator is available this run (explicit unattended instruction). Nothing
above required escalation: every choice traces to a spec default, a redesign-trigger
check that did not fire, or an honestly-recorded environment limitation.
