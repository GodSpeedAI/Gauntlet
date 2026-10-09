//! Offline checks of the committed evidence store, `definitions/VERIFICATION.toml`.

use gauntlet_adapter_harness_catalog::builtin_with_evidence;
use gauntlet_adapter_harness_catalog::matrix::PRIME_ROW_ID;
use gauntlet_adapter_harness_catalog::verification::{EvidenceKind, LEGAL_FIELDS};

#[test]
fn every_record_names_an_existing_harness_a_legal_field_and_a_legal_kind() {
    let (defs, records) = builtin_with_evidence().expect("catalog and evidence load");
    assert!(
        !records.is_empty(),
        "the evidence store should not be empty"
    );

    let mut known: Vec<&str> = defs.iter().map(|d| d.id.as_str()).collect();
    known.push(PRIME_ROW_ID);

    for record in &records {
        assert!(
            known.contains(&record.harness.as_str()),
            "record names unknown harness {}",
            record.harness
        );
        assert!(
            LEGAL_FIELDS.contains(&record.field.as_str()),
            "record names illegal field {}",
            record.field
        );
        if record.kind == EvidenceKind::LiveVerified {
            assert!(
                !record.probe_argv.is_empty() && record.harness_version != "n/a",
                "live-verified {}.{} needs probe_argv and a harness_version",
                record.harness,
                record.field
            );
        }
    }
}

#[test]
fn live_verified_records_only_cover_the_installed_binaries_and_only_version_or_help() {
    let (_, records) = builtin_with_evidence().expect("catalog and evidence load");
    for record in records
        .iter()
        .filter(|r| r.kind == EvidenceKind::LiveVerified)
    {
        assert_eq!(
            record.field, "executables",
            "this slice live-verifies only discovery plus version, not {}.{}",
            record.harness, record.field
        );
        assert_eq!(
            record.probe_argv.get(1).map(String::as_str),
            Some("--version"),
            "live-verified {} must come from --version",
            record.harness
        );
    }
}

#[test]
fn every_definition_has_at_least_a_fixture_record() {
    let (defs, records) = builtin_with_evidence().expect("catalog and evidence load");
    for def in &defs {
        assert!(
            records
                .iter()
                .any(|r| r.harness == def.id && r.kind == EvidenceKind::Fixture),
            "{} has no fixture record",
            def.id
        );
    }
}
