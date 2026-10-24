# Cognitive Representations & Interactive Explainers

Gauntlet provides interactive, visual, and executable models of its runtime architecture and mechanisms.
These explainers are self-contained, offline-capable single-page applications located in this directory.

## Interactive HTML Explainers

| Explainer | Core Question Answered | Media & Cognitive Affordances | Implementation Evidence |
|---|---|---|---|
| [**index.html**](index.html) | How do all Gauntlet concepts, journeys, subsystems, and explainers connect? | Central portal with cognitive matrix, reader-intent router, and source evidence links | All crates |
| [**run-execution-loop.html**](run-execution-loop.html) | How does a run transition through 9 stages without letting the executor redefine success? | Synchronized 9-stage stepper, 5 scenarios (clean pass, test fix, inconclusive report, grader tampering, irreversible refusal), Separation 1 claim segregation probe, Verifier 3-state matrix | `crates/gauntlet-app/src/control/`, `crates/gauntlet-ports/src/verifier.rs` |
| [**architecture-and-invariants.html**](architecture-and-invariants.html) | How do 33 crates enforce strict inward layering and the Four Great Separations? | Interactive concentric layer inspector, 17 ports & 24 adapters directory with contract test suites, Four Separations lab, 6-scenario invariant stress-tester | `crates/gauntlet-arch-tests`, `crates/gauntlet-ports/src/` |
| [**governance-and-containment.html**](governance-and-containment.html) | Why are Authority and Containment orthogonal, and how does Bubblewrap isolate builders? | 4x4 interactive Authority vs Containment matrix, dynamic `bwrap` command generator with sealed credential paths, real-time 15% recovery reserve simulator | `crates/adapters/gauntlet-adapter-sandbox-bwrap/`, `crates/gauntlet-domain/src/budget.rs` |

---

## Governing Cognitive Principles

1. **Representation is an engineering discipline (Bret Victor):** Good representations make previously difficult thinking easier. The form in which an idea is represented determines which cognitive capabilities can be brought to bear on understanding it.
2. **Recognition over recall:** Essential system invariants, reference lock digests, and observation segregation are visible on-screen simultaneously rather than reconstructed from memory.
3. **Visible causality over unexplained sequence:** When a unit fails a test, the stepper visibly shows the observer recording the failure exit code, the critic producing a residual, the correction selector choosing a minimal fix, and the verifier confirming the subsequent pass before settlement.
4. **Offline portability & zero runtime dependencies:** Every HTML explainer in this directory is 100% self-contained and offline-capable, requiring no CDN, no local web server, and no build pipeline.

---

## Technical Verification & Traceability

All three interactive applications reflect verified repository invariants:

1. **Ruler Immutability:** Verified by `scripts/verify-ruler.sh` (`just ruler`). The answer key digest cannot change during execution.
2. **Three-State Verification:** Implemented in `crates/gauntlet-domain/src/values.rs` (`Outcome::Pass`, `Outcome::Fail`, `Outcome::Inconclusive`). No boolean coercion exists.
3. **Inward Layering:** Enforced by `crates/gauntlet-arch-tests` (`just boundaries`). Adapters depend on ports and domain; domain depends on nothing.
4. **Authority & Containment Separation:** Enforced by `crates/gauntlet-app/src/govern/` and `crates/adapters/gauntlet-adapter-sandbox-bwrap/`. Irreversible units have no sandbox policy and refuse automatic dispatch.
5. **15% Recovery Reserve:** Enforced by `crates/gauntlet-domain/src/budget.rs` and `crates/gauntlet-ports/src/budget_meter.rs`.

---

## Related Canonical Documentation

- [../index.md](../index.md) — Documentation root and system thesis.
- [../mental-model.md](../mental-model.md) — The Cleanroom Inspection Line and Four Great Separations.
- [../architecture.md](../architecture.md) — Concentric Hexagonal architecture and dependency rules.
- [../workflows/run-lifecycle.md](../workflows/run-lifecycle.md) — Complete trace of `gauntlet run`.
- [../SECURITY.md](../SECURITY.md) — Security and operational safety invariants.
- [../CONFORMANCE.md](../CONFORMANCE.md) — Trust posture and sandbox tier mapping.
