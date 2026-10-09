//! Offline shape checks for the 24 built-in definitions' provenance. No donor checkout needed.

use gauntlet_adapter_harness_catalog::provenance::{claimed_fields, parse_ref};
use gauntlet_adapter_harness_catalog::{HERDR_LICENSE, HERDR_PINNED_COMMIT, HERDR_SOURCE, builtin};

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

        let parsed: Vec<_> = prov
            .refs
            .iter()
            .map(|r| parse_ref(r).unwrap_or_else(|e| panic!("{id}: {e}")))
            .collect();

        let expected = claimed_fields(def);
        let mut fields: Vec<_> = parsed.iter().map(|r| r.field).collect();
        fields.sort();
        let mut want = expected.clone();
        want.sort();
        assert_eq!(
            fields, want,
            "{id}: refs must be exactly one per claimed field"
        );
    }
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
