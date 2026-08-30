---
name: wayfinder-gauntlet
description: Form a Gauntlet reference from a loose intent by grilling it into a decision map and a typed answer key, leaving unresolved questions as fog rather than inventing answers.
disable-model-invocation: true
---

Formation runs when no sufficient reference exists yet for the work at hand. A
loose idea is too big, and too fogged, to hand straight to a builder. This
skill charts the way from that idea to a **destination** — the finished
reference — through a **grilling** process: a loop of sharp questions, worked
one at a time, that turns a vague ask into (a) a decision map and (b) a typed,
schema-valid answer key. Nothing gets guessed into place; what isn't sharp yet
stays visibly unresolved.

This is a derivation of the upstream `wayfinder` skill
(`skills/engineering/wayfinder` in `mattpocock/skills`), adapted for the
Gauntlet's writer-authority model. See `PROVENANCE.md` in this directory for
exact upstream commit, licence, and the full delta. The load-bearing
semantics below are preserved from upstream; the mechanics of where decisions
get written are not.

## Destination first

Naming what reaching the end looks like is the first act of formation, before
any question gets asked. The destination might be a spec to hand off, a
decision to lock before building starts, or an in-place change. State it in
one or two lines and write it at the top of `.wayfinder/map.md`. Every later
question is measured against this scope — it fixes what counts as in scope,
what's merely not-yet-specified, and what's out of scope entirely.

## The grilling loop

Formation proceeds by grilling: asking pointed questions that convert
ambiguity into either a resolved decision or an explicitly fogged one. Grill
breadth-first first (fan out across the whole space to find what's open and
what's immediately takeable), then work decisions one at a time. Never
resolve more than one decision per session — the one exception is a
research-style question that can run unattended in parallel with nothing to
decide. Rushing to resolve several decisions in one pass is how a skill
quietly starts inventing answers instead of asking for them.

## Fog of war

The map is deliberately incomplete. Anything you can sense is coming but
can't yet phrase precisely stays out of the answer key and out of any
decision entry — it goes in `.wayfinder/map.md`'s **Not yet specified**
section instead. This is the fog of war: the dim, in-scope view of what's
ahead, not yet sharp enough to act on.

**The test for fog versus a resolvable question is whether you can state the
question precisely right now — not whether you can currently answer it.** A
sharp, blocked, unanswered question is still a question you can work (or
queue); a question you can't yet phrase precisely is fog. Don't pre-slice fog
into question-sized pieces before it's ready — one foggy patch may resolve
into several decisions later, or none.

Fog clears as decisions resolve: answering one question often sharpens what
was previously too dim to state. Move a fog entry out of **Not yet
specified** and into a real decision only once it can be stated precisely,
never before.

## Out of scope

Out of scope is a distinct section from fog, and it never graduates. Fog is
in-scope-but-not-yet-sharp; out of scope is *ruled outside the destination
entirely*. Scope, not sharpness, is what puts something here. When a
prospective decision turns out to sit beyond the destination — whether it was
mis-scoped in originally or only became clear later — record it under **Out
of scope** in `.wayfinder/map.md` with a one-line gist and the reason, and
leave it there. It returns only if the destination itself is redrawn, and
then as a fresh formation effort, not a resumption of this one.

## HITL versus AFK — never self-answer

Every grilling question is either resolved with a real human in the loop
(HITL) or, for questions that are genuinely AFK-appropriate (research that
only needs external facts, not a judgment call), resolved by an agent working
alone. **An agent that answers its own grilling questions has broken this
skill.** A model producing both the question and its own answer is not
formation, it's fabrication wearing formation's shape.

In an unattended (no human available) run, any question that would normally
require HITL resolution MUST be recorded as fog in **Not yet specified**
rather than answered by the agent's own judgment. A confidently-filled
answer key produced this way is worse than an incomplete one: it looks
settled while encoding a guess. Record the unresolved question and stop
there; do not invent.

## One decision at a time

Work the map one decision at a time (with the research exception above).
Resolving a decision can graduate previously-fogged questions into
resolvable ones and can reveal that other open items are actually out of
scope — re-evaluate the map after each resolution rather than batching
several resolutions blind to each other's effects.

## The two files (replaces upstream's tracker wiring)

Upstream charts the map as an issue-tracker issue with child-issue tickets,
using tracker labels for type and native blocking edges for ordering. The
Gauntlet's writer-authority model (Core-spec §16.4) needs a small,
single-writer structured file instead of a tracker API, so this skill
produces exactly two local files:

- **`.wayfinder/map.md`** — the low-resolution index: Destination, Decisions
  so far (each entry linking the Unit id(s) it produced, or marked
  `context-only` when the decision produced no checkable Unit), Not yet
  specified (fog), Out of scope. This replaces the tracker map issue and its
  Decisions-so-far ticket links.
- **`.wayfinder/answer-key.md`** — the typed, checkable output. Each
  resolved decision that yields a checkable claim about the finished system
  becomes one or more Unit entries here. This replaces the tracker's child
  tickets; where a ticket body used to hold a bare question with its answer
  recorded on resolution, an answer-key entry holds a typed `Unit`
  (Core-spec §5.3.2) — a claim bound to how it gets verified and what
  evidence would prove it, not a free-text assertion graded by eyeball.

### `map.md` shape

```markdown
## Destination

<what reaching the end of this formation effort looks like, one or two lines>

## Notes

<domain context, companion skills to consult, standing preferences>

## Decisions so far

- [D1] <one-line decision text> → AK-101, AK-102
- [D2] <one-line decision text> → context-only

## Not yet specified

- [F1] <fog question — not yet precisely statable>

## Out of scope

- [S1] <ruled-out item and why>
```

Every entry in **Decisions so far** MUST either point at ≥1 Unit id in the
answer key or be marked `context-only`. Nothing in **Not yet specified**
and nothing in **Out of scope** may carry an answer-key reference — either
is a fog or scope leak into the ruler, and formation is not done while one
exists.

### The Unit schema (replaces tracker labels and ticket bodies)

Where upstream used tracker labels for ticket type, this skill uses the
typed fields of Core-spec §5.3.2 directly, because execution-safety fields
must be enums checked by a machine, not free-text labels read by a human:

```markdown
## AK-101
claim: <assertion that is true or false of the finished system>
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: <where the evidence lives>, producer: instrument}
depends_on: [AK-100]
```

- `id` — the heading (`AK-NNN`), the Unit's identity within this answer key.
- `claim` — the assertion, stated so it is checkably true or false.
- `verify_by` — how it gets checked (e.g. `test`, `static_check`,
  `human_signoff`).
- `blast_radius` — see the default rule below.
- `authority` — who may settle this Unit; `blast_radius` and `authority`
  are coupled per Core-spec §5.3.2 (an `irreversible` Unit requires
  `authority: human`; `cross_cutting`/`irreversible` Units MUST NOT carry
  `authority: critic`).
- `evidence_required` — what would prove the claim: `kind`, `locator`,
  and `producer` (`instrument`, never a bare model assertion, unless
  `verify_by: human_signoff`).
- `depends_on` — replaces the tracker's native blocking edges. It is
  **ordering only**; it never confers authority, and it never substitutes
  for a Unit's own `authority` field.

### `blast_radius` default

Default every new Unit to `blast_radius: local`. Only raise it when the
decision touches shared state, money, authentication, security, or is hard
to reverse — those get `cross_cutting` or `irreversible` instead. Do not
guess a wider radius "to be safe"; a default that is too wide routes trivial
decisions through authority they don't need, and a default that is too
narrow is the more dangerous failure this rule exists to prevent, so treat
the touches-shared-state/money/auth/security/hard-to-reverse test as the
actual gate, not a formality.

This skill does not decide settlement, authority elevation, or budget: it
only assigns the *initial* typed fields on a Unit at formation time. What
happens with those fields at run time — dispatch, verification, settlement —
is runtime control logic, not something this skill (or any skill) implements
(Core-spec Annex D.4: skills are instructions, not control logic).

## Formation procedure

### Chart the map

1. **Name the destination.** Grill for what reaching the end looks like;
   write it into `.wayfinder/map.md`'s Destination section first, since it
   fixes scope for everything after.
2. **Map the frontier, breadth-first.** Fan out across the whole space
   rather than going deep on one thread. Sketch what's resolvable now,
   what's blocked-but-sharp, and what's still fog into **Not yet
   specified**. If nothing is fogged and the whole effort fits one session,
   stop here and say so rather than forcing a map into existence.
3. **Create the map file** with Destination and Notes filled in, Decisions
   so far empty, and the fog sketched in.
4. **Work decisions one at a time** (per "One decision at a time" above).
   For each: grill it to resolution (HITL for anything requiring human
   judgment; AFK only for genuinely fact-finding questions; never
   self-answer), then:
   - record the resolution as one or more `## AK-NNN` Units in
     `.wayfinder/answer-key.md` (or mark the map entry `context-only` if
     the decision produced no checkable claim),
   - add the corresponding `[Dn] ... → AK-...` line to `map.md`'s Decisions
     so far,
   - move any newly-sharpened fog out of **Not yet specified** into a fresh
     decision, or move a decision found to sit beyond the destination into
     **Out of scope** instead of resolving it on the route.
5. Repeat until the way to the destination is clear — not until every fog
   item is resolved. Remaining fog is a legitimate, first-class output of
   formation, not a failure to fix before stopping.
6. **Validate before declaring formation done.** As the last step, run:

   ```bash
   gauntlet reference validate --path .wayfinder/answer-key.md
   ```

   This invokes the reference validator so a non-schema-conforming answer
   key is rejected at formation time, before anything downstream builds
   against it — not discovered later at run time. Formation is not complete
   until this command exits clean.

## Companion skills

Per Core-spec Annex D.4, this skill leans on companions for parts of the
grilling loop it does not itself implement: `grilling` (extracting sharp
questions from loose intent), `domain-modeling` (producing the vocabulary
the reference is written in), `research` (AFK resolution of a fact a
decision waits on), and `prototype` (a cheap concrete artifact to raise
discussion fidelity, used for HITL decisions where "how should it look or
behave" is the live question). Call the Skill tool for whichever of these a
given decision needs; when in doubt for a HITL decision, call both
`grilling` and `domain-modeling`.
