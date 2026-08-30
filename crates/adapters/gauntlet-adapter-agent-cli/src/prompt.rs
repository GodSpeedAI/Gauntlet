//! The invocation prompt file: the adapter-defined, machine-parseable
//! document that carries what the child needs to write a MATCHING result
//! contract (plan Task 16; spec §10.3 rule 1's prompt channel).
//!
//! ## Format: `gauntlet-cli-agent-prompt@1`
//!
//! ```text
//! # gauntlet cli-agent prompt (format: gauntlet-cli-agent-prompt@1)
//! # One "KEY: VALUE" line per field, fixed order; the prompt body
//! # follows the BODY-BEGIN line verbatim to end-of-file. Parsers must
//! # read keys only BEFORE BODY-BEGIN and must refuse unknown keys.
//! INVOCATION-ID: <ULID>            — must be echoed by the contract (§10.3 rule 6)
//! RUN-ID: <ULID>
//! ITERATION-NO: <n>
//! ROLE: <role token>
//! ENVELOPE-DIGEST: <digest>        — must be echoed by the contract (§10.3 rule 6)
//! ENVELOPE-PATH: <absolute path>   — the context envelope BODY (RES-D2)
//! RESULT-PATH: <absolute path>     — write the contract EXACTLY here (rule 2)
//! WORKSPACE: <absolute path>
//! MODEL: <model identity, or empty when the dispatch carries none>
//! DEADLINE-EPOCH: <unix seconds>
//! BODY-BEGIN
//! <the role prompt, verbatim, to EOF>
//! ```
//!
//! Invariants:
//!
//! - the fields are single lines (`KEY: VALUE`); the body may contain
//!   anything, including lines that look like fields — parsers read keys
//!   only before `BODY-BEGIN`;
//! - the file lives inside the invocation's ANCHORED state directory
//!   (`<state_dir>/runs/<run_id>/inv/<invocation_id>/`), never inside the
//!   workspace source tree (prime's RES-D3 precedent), and is RETAINED
//!   after terminal states (RES-D12's forensics posture);
//! - the envelope body travels in its own sibling file
//!   (`<leaf>.envelope.json`, referenced by `ENVELOPE-PATH`) because the
//!   envelope is runtime-built, possibly multi-line context — the prompt
//!   file's field grammar must stay line-oriented (correction RES-D2);
//!   the digest is the one carried on the spec, echoed verbatim, never
//!   recomputed here;
//! - the deadline is recorded as informational context for the child;
//!   the RUNTIME's deadline verdict never derives from the child's
//!   reading of it.

use gauntlet_ports::agent_runner::RoleInvocationSpec;

use crate::PROMPT_FORMAT;

/// One field line of the prompt file, rendered `KEY: VALUE`.
fn field(key: &str, value: &str) -> String {
    format!("{key}: {value}")
}

/// Renders the prompt file for one dispatch. `envelope_path` names the
/// sibling file carrying the context envelope body (RES-D2);
/// `result_path` is the exact contract file the child must write;
/// `workspace` is the directory the child starts in.
pub fn render_prompt(
    spec: &RoleInvocationSpec,
    envelope_path: &std::path::Path,
    result_path: &std::path::Path,
    workspace: &std::path::Path,
) -> String {
    let mut out = String::new();
    out.push_str("# gauntlet cli-agent prompt (format: ");
    out.push_str(PROMPT_FORMAT);
    out.push_str(")\n");
    out.push_str("# One \"KEY: VALUE\" line per field, fixed order; the prompt body\n");
    out.push_str("# follows the BODY-BEGIN line verbatim to end-of-file. Parsers must\n");
    out.push_str("# read keys only BEFORE BODY-BEGIN and must refuse unknown keys.\n");
    for line in [
        field("INVOCATION-ID", spec.invocation_id.as_str()),
        field("RUN-ID", spec.run_id.as_str()),
        field("ITERATION-NO", &spec.iteration_no.get().to_string()),
        field("ROLE", &spec.role.to_string()),
        field(
            "ENVELOPE-DIGEST",
            &spec.context_envelope.digest().to_string(),
        ),
        field("ENVELOPE-PATH", &envelope_path.to_string_lossy()),
        field("RESULT-PATH", &result_path.to_string_lossy()),
        field("WORKSPACE", &workspace.to_string_lossy()),
        field(
            "MODEL",
            spec.model_hint.as_ref().map(|m| m.as_str()).unwrap_or(""),
        ),
        field("DEADLINE-EPOCH", &spec.deadline.instant().get().to_string()),
    ] {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("BODY-BEGIN\n");
    out.push_str(spec.prompt.as_str());
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, IterationNo, Limits,
        MicroUsd, PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
    };
    use gauntlet_ports::agent_runner::{ContextEnvelope, RoleInvocationSpec};
    use gauntlet_ports::deadline::Deadline;

    fn spec() -> RoleInvocationSpec {
        let budget = Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits usable"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget usable");
        RoleInvocationSpec {
            invocation_id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid id"),
            run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid id"),
            iteration_no: IterationNo::new(1).expect("1-based"),
            role: RoleId::Builder,
            prompt: "run the slice\nRESULT-PATH: /fake — body lines that LOOK like fields \
                     must never be read as fields"
                .to_owned(),
            context_envelope: ContextEnvelope::new(
                "the runtime-built allowlisted context".to_owned(),
                Digest::new("sha256", "e5f6a7b8c4d3e2f1a0b1c2d3").expect("digest"),
            ),
            workspace: WorkspaceRef::new("ws-runner").expect("valid ref"),
            permissions: PermissionSetRef::new("perm-readonly").expect("valid ref"),
            result_contract_path: ResultContractPath::new("result.json").expect("valid ref"),
            deadline: Deadline::at_epoch(EpochSeconds::new(1_700_000_030)),
            model_hint: None,
            sandbox_policy: None,
            budget_slice: budget,
        }
    }

    /// The prompt file parses back through the documented grammar: keys
    /// before BODY-BEGIN only, body verbatim to EOF — including a body
    /// line that looks exactly like a field (the falsifier's shape).
    #[test]
    fn the_prompt_file_parses_back_and_the_body_is_verbatim() {
        let text = render_prompt(
            &spec(),
            std::path::Path::new("/state/runs/r/inv/i/result.json.envelope.json"),
            std::path::Path::new("/state/runs/r/inv/i/result.json"),
            std::path::Path::new("/ws"),
        );
        let mut fields = std::collections::BTreeMap::new();
        let mut lines = text.lines();
        let mut body = String::new();
        let mut in_body = false;
        for line in lines.by_ref() {
            if in_body {
                body.push_str(line);
                body.push('\n');
                continue;
            }
            if line == "BODY-BEGIN" {
                in_body = true;
                continue;
            }
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once(": ") else {
                panic!("unparseable field line {line:?}");
            };
            fields.insert(key.to_owned(), value.to_owned());
        }
        assert_eq!(fields["INVOCATION-ID"], "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(fields["ROLE"], "builder");
        assert_eq!(fields["ENVELOPE-DIGEST"], "sha256:e5f6a7b8c4d3e2f1a0b1c2d3");
        assert_eq!(
            fields["ENVELOPE-PATH"],
            "/state/runs/r/inv/i/result.json.envelope.json"
        );
        assert_eq!(fields["RESULT-PATH"], "/state/runs/r/inv/i/result.json");
        assert_eq!(fields["MODEL"], "");
        assert_eq!(fields["DEADLINE-EPOCH"], "1700000030");
        // The hostile body line stays BODY data, never a field.
        assert!(
            body.contains("RESULT-PATH: /fake"),
            "the body must be verbatim: {body:?}"
        );
        assert_eq!(fields.len(), 10, "exactly the documented fields");
    }
}
