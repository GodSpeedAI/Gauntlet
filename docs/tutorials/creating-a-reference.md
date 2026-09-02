# Tutorial: Creating a Reference and Answer Key

This tutorial guides you through specifying, validating, and freezing a Reference and Answer Key for a software project.

---

## Prerequisites

- Access to the `gauntlet` CLI binary.
- A project directory where you want to govern transformations.

---

## Step 1: Initialize the Reference Structure

Run `gauntlet init` in your target repository to create the standard directory layout:

```bash
gauntlet init
```

This creates:
- `.wayfinder/reference.yaml` (The high-level manifest)
- `.wayfinder/answer-key.md` (The checkable Units)
- `.gauntlet/policies/` (Containment policies)
- `GAUNTLET.md` (Runtime configuration)

---

## Step 2: Author the Reference Manifest (`reference.yaml`)

Edit `.wayfinder/reference.yaml` to describe the destination, constraints, and invariants:

```yaml
reference:
  id: auth-service-v1
  version: 1
  digest: null

  purpose: >
    Implement secure user authentication with multi-factor support,
    brute-force lockout, and audit logging.

  desired_state: >
    All user passwords are encrypted with Argon2id. Repeated failed logins
    trigger account lockout after 5 attempts. Admin actions emit audit events.

  invariants:
    - id: INV-01
      statement: Passwords are never stored or logged in plaintext.
    - id: INV-02
      statement: Rate limiting must enforce 5 attempts per IP per minute.

  constraints:
    - Must pass `cargo test` and `cargo clippy`.
    - No external network calls permitted during password hashing.

  anti_goals:
    - Storing passwords in MD5, SHA1, or unsalted SHA256.
    - Bypassing rate limits for local development environments.
```

---

## Step 3: Author Checkable Units in `answer-key.md`

Edit `.wayfinder/answer-key.md`. Each Unit is a falsifiable assertion with a declared verification method and evidence requirement:

```markdown
## AK-001

claim: Passwords are hashed using argon2id with salt length >= 16 bytes
verify_by: test
blast_radius: local
authority: lead_agent
evidence_required:
  kind: test_report
  locator: tests/auth.rs::test_argon2id_hashing
  producer: instrument
status: open

---

## AK-002

claim: Account locks after 5 consecutive failed login attempts within 5 minutes
verify_by: test
blast_radius: local
authority: lead_agent
depends_on: [AK-001]
evidence_required:
  kind: test_report
  locator: tests/auth.rs::test_account_lockout
  producer: instrument
status: open

---

## AK-003

claim: Production migration script is reviewed and approved before applying
verify_by: human_signoff
blast_radius: irreversible
authority: human
depends_on: [AK-002]
evidence_required:
  kind: attestation
  locator: security-team-approval
  producer: human
status: open
```

> [!IMPORTANT]
> **Tier and Authority Rules**:
> - If `blast_radius: irreversible`, `authority` **must be `human`**.
> - If `verify_by` is an instrument check (`test`, `build`, `schema_check`), `producer` **must be `instrument`**.
> - Claims that can only be judged subjectively belong in `quality_dimensions` in `reference.yaml`, not as blocking Units.

---

## Step 4: Validate the Reference

Validate your reference files against the spec §5.3.2 schema:

```bash
gauntlet reference validate --path .wayfinder/answer-key.md
```

### Expected Output
```text
Validating .wayfinder/answer-key.md:
  - 3 units found
  - Unit AK-001: valid (test, local, lead_agent)
  - Unit AK-002: valid (test, local, lead_agent, depends_on: [AK-001])
  - Unit AK-003: valid (human_signoff, irreversible, human)
Validation PASSED: All 3 units conform to spec §5.3.2.
```

---

## Step 5: Compute Digest and Freeze

Calculate the normalized cryptographic SHA-256 digest:

```bash
gauntlet reference digest --path .wayfinder/answer-key.md
```

Now freeze the reference for your project:

```bash
gauntlet reference freeze
```

This writes the immutable freeze record to `.gauntlet/reference.lock`:
```yaml
answer_key_digest: sha256:d82e4a91b...
manifest_digest: sha256:1f3b5c77a...
frozen_at: 2026-09-02T18:30:00Z
```

---

## Step 6: Verify Ruler Immutability

Run `gauntlet reference verify` to prove the locked ruler matches disk:

```bash
gauntlet reference verify
```

### Expected Output
```text
Reference verification:
  - Manifest digest sha256:1f3b5c77a... MATCHES .gauntlet/reference.lock
  - Answer key digest sha256:d82e4a91b... MATCHES .gauntlet/reference.lock
Reference is locked and intact. Ready for execution.
```

---

## Next Steps

- Execute a run against your new reference: [First Gauntlet Run Tutorial](first-gauntlet-run.md).
- Understand domain types and constraints: [Domain Types Reference](../reference/domain-types.md).
- Learn why references are frozen: [Mental Model](../mental-model.md).
