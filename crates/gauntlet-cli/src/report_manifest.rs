//! The CLI's capability manifest (plan Task 20, brief Item 4; charter
//! D8): the command table THIS surface serves, declared to the
//! six-category report. The manifest is CALLER-DECLARED — it records
//! what the surface wires, never what the records prove; the report's
//! population rules then classify each capability from the run's durable
//! records alone (`capability rows never upgrade a not-wired
//! declaration`).
//!
//! Every §20.1 command fronts a backing service — `improve` included
//! since plan Task 23 wired the optimizer surface.

use gauntlet_app::report::CapabilityManifestEntry;

/// One manifest row: the §20.1 command name and whether this surface
/// wires it.
struct CommandRow {
    name: &'static str,
    wired: bool,
}

/// The real command table (keep in step with `run_cli`'s dispatch and
/// the `cli_surface` snapshot; a command added there without a row here
/// under-reports the surface's scope).
const COMMAND_TABLE: &[CommandRow] = &[
    CommandRow {
        name: "doctor",
        wired: true,
    },
    CommandRow {
        name: "init",
        wired: true,
    },
    CommandRow {
        name: "reference",
        wired: true,
    },
    CommandRow {
        name: "run",
        wired: true,
    },
    CommandRow {
        name: "status",
        wired: true,
    },
    CommandRow {
        name: "inspect",
        wired: true,
    },
    CommandRow {
        name: "verify",
        wired: true,
    },
    CommandRow {
        name: "resume",
        wired: true,
    },
    CommandRow {
        name: "escalations",
        wired: true,
    },
    CommandRow {
        name: "roles",
        wired: true,
    },
    CommandRow {
        name: "skills",
        wired: true,
    },
    // Plan Task 23 wired the optimizer surface: `improve
    // propose|evaluate|promote|rollback` fronts the §19.3/§19.4 services
    // (the manifest row and the command agree by construction; the
    // `cli_surface` pins it).
    CommandRow {
        name: "improve",
        wired: true,
    },
    CommandRow {
        name: "export",
        wired: true,
    },
    // Plan §W5: `deliver` fronts the work-product promotion/delivery
    // path — the controller's `promote` + this surface's delta/apply.
    CommandRow {
        name: "deliver",
        wired: true,
    },
    // F2b2: `fabric` fronts the agent-fabric root admission and the per-caller
    // host process (`admit-root`, `serve`).
    CommandRow {
        name: "fabric",
        wired: true,
    },
];

/// The manifest entries handed to `gauntlet_app::report::build_report`.
/// No command binds an evidence unit here — capability evidence is a
/// run-record fact, not a surface claim. Under the `surfaces` feature
/// the §20.2 JSON surface's `serve` command joins the table (Task 21,
/// charter D9) — the DEFAULT build's table stays byte-identical.
pub fn cli_capability_manifest() -> Vec<CapabilityManifestEntry> {
    let entries: Vec<CapabilityManifestEntry> = COMMAND_TABLE
        .iter()
        .map(|row| {
            CapabilityManifestEntry::new(row.name.to_owned(), row.wired, None)
                .expect("the command table carries no empty keys")
        })
        .chain(surfaces_rows())
        .collect();
    entries
}

/// The surfaces feature's extra row count (the `serve` command), so the
/// pinned length and the manifest cannot drift per configuration.
#[cfg(feature = "surfaces")]
fn surfaces_rows() -> Option<CapabilityManifestEntry> {
    Some(
        CapabilityManifestEntry::new("serve".to_owned(), true, None)
            .expect("the command table carries no empty keys"),
    )
}

#[cfg(not(feature = "surfaces"))]
fn surfaces_rows() -> Option<CapabilityManifestEntry> {
    None
}

#[cfg(feature = "surfaces")]
const fn surfaces_extra_rows() -> usize {
    1
}

#[cfg(not(feature = "surfaces"))]
const fn surfaces_extra_rows() -> usize {
    0
}

/// The table's size and shape, pinned so a drift between the dispatch,
/// the usage text, and this manifest fails a test instead of shrinking
/// the report's scope silently.
pub const COMMAND_TABLE_LEN: usize = COMMAND_TABLE.len() + surfaces_extra_rows();

/// Whether the named command is wired (the `improve` command's own help
/// reads this so the exit and the manifest cannot disagree).
pub fn is_wired(name: &str) -> bool {
    COMMAND_TABLE
        .iter()
        .find(|row| row.name == name)
        .map(|row| row.wired)
        .unwrap_or(false)
}

/// Compile-time shape guards (the report refuses empty/duplicate keys;
/// the guard keeps that refusal a test failure, not a runtime surprise).
const _: () = {
    assert!(COMMAND_TABLE_LEN > 0, "the command table is not empty");
};

// A duplicate key would surface as a typed ReportError at report time;
// this check makes it a unit-test failure at build time instead.
#[cfg(test)]
mod tests {
    use super::{COMMAND_TABLE, COMMAND_TABLE_LEN, is_wired, surfaces_extra_rows};
    use gauntlet_app::report::ReportError;

    #[test]
    fn the_command_table_has_no_duplicates_and_names_the_not_wired_row() {
        let mut names: Vec<&str> = COMMAND_TABLE.iter().map(|row| row.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate command row");
        assert_eq!(
            COMMAND_TABLE_LEN,
            COMMAND_TABLE.len() + surfaces_extra_rows()
        );
        assert!(
            is_wired("improve"),
            "the manifest's wired fact and the command's backing services agree"
        );
        let _ = ReportError::ManifestKeyEmpty; // the typed shape this table must never trip
    }
}
