//! The `EvidenceStore` shared contract suite (spec §6.2: "content-addressed
//! · Immutable. No overwrite. Pinned evidence survives cleanup.").
//!
//! The immutability invariant is tested behaviorally through the API shape:
//! same bytes ⇒ same ref (content addressing), different bytes ⇒ different
//! refs, `get` round-trips, and pin requires an existing ref. There is no
//! update path in the trait, so "no overwrite" is unrepresentable — the
//! suite pins what IS reachable.

use gauntlet_domain::{EvidenceKind, EvidenceRef, RunId};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::evidence_store::{EvidenceStore, EvidenceStoreError};

/// The suite's name in findings.
pub const SUITE: &str = "evidence_store";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the store under test. `build` must return an ISOLATED, EMPTY
/// instance.
pub trait EvidenceStoreFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn EvidenceStore>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

fn bound() -> Deadline {
    Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
        BASE_INSTANT + BOUND_SPAN,
    ))
}

/// Case: putting is content-addressed — the same bytes return the same ref
/// twice; different bytes return a different ref.
fn put_is_content_addressed(factory: &dyn EvidenceStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "put_is_content_addressed";
    const RULE: &str =
        "spec 6.2 EvidenceStore: put(bytes, kind) -> EvidenceRef is content-addressed";
    let mut findings = Vec::new();
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let first = match store.put(b"observation bytes one", EvidenceKind::Log, bound()) {
        Ok(reference) => reference,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the first put failed: {error}"),
            )];
        }
    };
    let again = match store.put(b"observation bytes one", EvidenceKind::Log, bound()) {
        Ok(reference) => reference,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the second put failed: {error}"),
            )];
        }
    };
    if first != again {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the same bytes were assigned {first} then {again} — refs must be content \
                 addressed"
            ),
        ));
    }
    let other = match store.put(b"observation bytes two", EvidenceKind::Log, bound()) {
        Ok(reference) => reference,
        Err(error) => {
            findings.push(finding(
                CASE,
                RULE,
                format!("putting different bytes failed: {error}"),
            ));
            return findings;
        }
    };
    if other == first {
        findings.push(finding(
            CASE,
            RULE,
            "different bytes were assigned the same ref — content addressing is absent".to_owned(),
        ));
    }
    findings
}

/// Case: `get` returns exactly the stored bytes; an unknown ref is `None`.
fn get_round_trips_bytes(factory: &dyn EvidenceStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "get_round_trips_bytes";
    const RULE: &str = "spec 6.2 EvidenceStore: get returns the bytes at the ref";
    let mut findings = Vec::new();
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let payload = b"evidence payload for the suite";
    let reference = match store.put(payload, EvidenceKind::File, bound()) {
        Ok(reference) => reference,
        Err(error) => {
            return vec![finding(CASE, RULE, format!("put failed: {error}"))];
        }
    };
    match store.get(&reference, bound()) {
        Ok(Some(bytes)) if bytes.as_slice() == payload => {}
        Ok(Some(bytes)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "get returned {} bytes, not the {} that were put",
                bytes.len(),
                payload.len()
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "get returned None for a ref that was just put".to_owned(),
        )),
        Err(error) => findings.push(finding(CASE, RULE, format!("get errored: {error}"))),
    }
    let unknown = EvidenceRef::new("evd-never-put-9999").expect("well-formed ref");
    match store.get(&unknown, bound()) {
        Ok(None) => {}
        Ok(Some(_)) => findings.push(finding(
            CASE,
            RULE,
            "get returned bytes for a ref that was never put".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("get of an unknown ref errored instead of None: {error}"),
        )),
    }
    // Repeated gets return the same bytes — no drift.
    match store.get(&reference, bound()) {
        Ok(Some(bytes)) if bytes.as_slice() == payload => {}
        _ => findings.push(finding(
            CASE,
            RULE,
            "a second get of the same ref returned different bytes".to_owned(),
        )),
    }
    findings
}

/// Case: pinning an existing ref succeeds and is idempotent; pinning an
/// unknown ref is a typed `NotFound` refusal.
fn pin_binds_evidence_to_a_run(factory: &dyn EvidenceStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "pin_binds_evidence_to_a_run";
    const RULE: &str = "spec 6.2 EvidenceStore: pin(run_id, ref); pinned evidence survives cleanup";
    let mut findings = Vec::new();
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let reference = match store.put(b"pinned payload", EvidenceKind::Attestation, bound()) {
        Ok(reference) => reference,
        Err(error) => {
            return vec![finding(CASE, RULE, format!("put failed: {error}"))];
        }
    };
    match store.pin(&run, &reference, bound()) {
        Ok(()) => {}
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("pinning an existing ref failed: {error}"),
            )];
        }
    }
    match store.pin(&run, &reference, bound()) {
        Ok(()) => {}
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("pinning the same ref twice failed — pin is idempotent: {error}"),
        )),
    }
    let unknown = EvidenceRef::new("evd-never-put-9999").expect("well-formed ref");
    match store.pin(&run, &unknown, bound()) {
        Err(EvidenceStoreError::NotFound { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("pinning an unknown ref must be NotFound, got {other}"),
        )),
        Ok(()) => findings.push(finding(
            CASE,
            RULE,
            "pinning a ref that was never put succeeded — pins bind existing evidence".to_owned(),
        )),
    }
    findings
}

/// Runs the full `EvidenceStore` contract suite against one factory.
pub fn evidence_store_suite(factory: &dyn EvidenceStoreFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(put_is_content_addressed(factory));
    findings.extend(get_round_trips_bytes(factory));
    findings.extend(pin_binds_evidence_to_a_run(factory));
    findings
}
