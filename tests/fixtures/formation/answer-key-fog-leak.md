## AK-101
claim: The `run` subcommand accepts a `--dry-run` flag that skips Unit dispatch entirely.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/fixtures/formation::fixture_check, producer: instrument}

## AK-102
claim: Under `--dry-run`, the CLI validates the answer key first, then prints the planned Unit order and budget without executing any Unit.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/fixtures/formation::fixture_check, producer: instrument}
depends_on: [AK-101]

## AK-103
claim: Under `--dry-run`, `irreversible`-tier Units are rendered with a distinct marker in the printed plan.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/fixtures/formation::fixture_check, producer: instrument}
