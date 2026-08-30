//! The shared answer-key validator (plan Task 18 step 4): a thin wrapper
//! over the Task 7 strict parser ([`super::parse::parse_all_or_fail`]), used
//! by both `gauntlet reference validate` (CLI, formation-time — the
//! answer-key-authoring skill's own last step, spec §9.2) and anywhere else
//! that needs to check an answer key's shape without standing up a full
//! `ReferenceProvider` pipeline (resolve → normalize → digest → freeze).
//!
//! No new parsing logic lives here: every rule this function enforces is
//! the Task 7 parser's rule, reused rather than reimplemented.

use gauntlet_domain::Unit;

use super::parse::{BlockFailure, parse_all_or_fail};

/// Validates an answer key's text against the frozen `## AK-NNN` block
/// shape (spec §5.3.2), all-or-nothing: every block well-formed, or the
/// first block failure as the error.
pub fn validate_answer_key(text: &str) -> Result<Vec<Unit>, BlockFailure> {
    parse_all_or_fail(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_key_validates_into_units() {
        let text = "## AK-001\nclaim: A checkable claim.\nverify_by: test\n\
                     blast_radius: local\nauthority: critic\nevidence_required: \
                     {kind: test_report, locator: suite-check-0001, producer: instrument}\n";
        let units = validate_answer_key(text).expect("a well-formed key validates");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].id().as_str(), "AK-001");
    }

    #[test]
    fn a_malformed_key_is_the_first_block_failure() {
        let text = "## AK-001\nverify_by: vibes\n";
        let failure = validate_answer_key(text).expect_err("an unknown enum value is refused");
        assert_eq!(failure.block.as_ref().map(|id| id.as_str()), Some("AK-001"));
    }
}
