//! Offline shape checks for the 24 built-in definitions' provenance. No donor checkout needed.

use gauntlet_adapter_harness_catalog::provenance::{
    check_probe_provenance, claimed_fields, claimed_probe_fields, is_probe_ref, parse_probe_ref,
    parse_ref,
};
use gauntlet_adapter_harness_catalog::{
    HERDR_LICENSE, HERDR_PINNED_COMMIT, HERDR_SOURCE, builtin, builtin_with_evidence,
};

#[test]
fn every_builtin_has_pinned_provenance_and_well_formed_refs() {
    let defs = builtin().expect("builtin catalog loads");
    assert_eq!(defs.len(), 24, "catalog should hold exactly 24 definitions");

    for def in &defs {
        let id = &def.id;
        let prov = def
            .provenance
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: missing provenance"));
        assert_eq!(prov.source, HERDR_SOURCE, "{id}: source");
        assert_eq!(
            prov.commit, HERDR_PINNED_COMMIT,
            "{id}: commit must equal pinned"
        );
        assert_eq!(prov.license, HERDR_LICENSE, "{id}: license");

        let herdr_refs: Vec<_> = prov
            .refs
            .iter()
            .filter(|r| !is_probe_ref(r))
            .map(|r| parse_ref(r).unwrap_or_else(|e| panic!("{id}: {e}")))
            .collect();
        let mut fields: Vec<_> = herdr_refs.iter().map(|r| r.field).collect();
        fields.sort();
        let mut want = claimed_fields(def);
        want.sort();
        assert_eq!(
            fields, want,
            "{id}: herdr refs must be exactly one per claimed herdr field"
        );

        let mut probe_fields: Vec<_> = prov
            .refs
            .iter()
            .filter_map(|r| parse_probe_ref(r))
            .map(|r| r.unwrap_or_else(|e| panic!("{id}: {e}")))
            .collect();
        probe_fields.sort();
        let mut probe_want = claimed_probe_fields(def);
        probe_want.sort();
        assert_eq!(
            probe_fields, probe_want,
            "{id}: probe refs must be exactly one per probe-backed claim"
        );
    }
}

#[test]
fn builtin_probe_refs_are_backed_by_verification_records() {
    let (defs, records) = builtin_with_evidence().expect("builtin evidence loads");
    let findings = check_probe_provenance(&defs, &records);
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn probe_refs_are_separate_from_donor_refs() {
    assert!(is_probe_ref("launch.prompt_mode=verification"));
    assert!(!is_probe_ref("executables=src/detect/mod.rs:202"));
    assert!(
        parse_ref("launch.prompt_mode=verification").is_err(),
        "a probe ref is not a donor ref"
    );
    assert!(
        parse_probe_ref("launch.shell=verification")
            .expect("probe shape")
            .is_err()
    );
}

#[test]
fn ref_grammar_rejects_unlabelled_and_out_of_shape_refs() {
    assert!(
        parse_ref("src/detect/mod.rs:127").is_err(),
        "unlabelled ref"
    );
    assert!(
        parse_ref("executables=src/detect/mod.rs").is_err(),
        "no line range"
    );
    assert!(
        parse_ref("executables=src/detect/mod.rs:9-3").is_err(),
        "inverted range"
    );
    assert!(
        parse_ref("executables=src/detect/mod.rs:0").is_err(),
        "line zero"
    );
    assert!(
        parse_ref("executables=src/agent_resume.rs:204").is_err(),
        "wrong donor file"
    );
    assert!(
        parse_ref("bogus=src/detect/mod.rs:1").is_err(),
        "unknown field"
    );
    assert!(
        parse_ref("executables=/etc/passwd:1").is_err(),
        "absolute path"
    );
    assert!(
        parse_ref("executables=../x/src/detect/mod.rs:1").is_err(),
        "parent path"
    );
    let ok = parse_ref("session.resume_argv=src/agent_resume.rs:204-210").expect("valid");
    assert_eq!((ok.start, ok.end), (204, 210));
}
