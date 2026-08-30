## Decisions so far
- [D1] The `run` subcommand gains a `--dry-run` flag that validates the answer key, then skips dispatch entirely and prints the planned Unit order and budget → AK-101, AK-102
- [D2] `--dry-run` output framing follows the same plan-summary style as normal `run` logging → context-only

## Not yet specified
- [F1] Whether `--dry-run` should render `irreversible`-tier Units differently, pending how irreversible-tier scheduling is represented

## Out of scope
- [S1] `--dry-run` support for the `roles install` subcommand
