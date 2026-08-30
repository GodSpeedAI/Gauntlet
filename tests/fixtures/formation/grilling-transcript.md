# Fixture: grilling transcript

This is a fictional, self-contained fixture transcript for a small formation
decision. It stands in for a real grilling session and is used by the
formation conformance tests, not by any live Gauntlet run.

---

**Agent:** What's the destination for this decision?

**Human:** Should the `gauntlet` CLI support a `--dry-run` flag on its
`run` subcommand?

**Agent:** What should `--dry-run` do, precisely — skip dispatch entirely,
or dispatch into a sandbox that discards its output?

**Human:** Skip dispatch entirely. It should just print the plan: which
Units would be attempted, in what order, under what budget.

**Agent:** Should `--dry-run` also validate the answer key first, or assume
it's already valid?

**Human:** Validate first — a dry run against a broken key isn't useful.

**Agent:** Should `--dry-run` be allowed to run against `irreversible`
Units, since nothing actually dispatches?

**Human:** Not sure yet — I'd want to see how irreversible-tier scheduling
is represented before deciding whether the dry-run output should even show
those Units differently.

**Agent:** Understood — that's not sharp enough to answer yet, so it stays
open rather than being guessed.

**Human:** Also, don't worry about a `--dry-run` for the `roles install`
subcommand — that's a different effort entirely.
