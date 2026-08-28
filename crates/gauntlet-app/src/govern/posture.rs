//! The §17.3 tier posture and the §17.1 lattice-movement rule (plan Task 14;
//! AK-046, AK-050's tier half).
//!
//! Two pure functions, no state:
//!
//! - [`required_isolation`]: the containment rung a tier's dispatch
//!   requires. `contained` is the only tier where the no-backend posture is
//!   permitted (§8.6); `local` and `cross_cutting` require a real backend
//!   and a runner reporting `isolation ≥ process` (§17.3's own sentence);
//!   `irreversible` is never dispatched at all (§17.1 line 1) — its entry
//!   keeps the function total and monotone with `blast_rank`, because the
//!   tier where an unsandboxed dispatch is most dangerous is not the one to
//!   leave unrepresented.
//! - [`review_lattice_movement`]: whether a proposed change to a Unit's
//!   recorded blast radius/authority may be honored by the RUNTIME (AK-046:
//!   "An agent role cannot lower a Unit's blast radius or authority; only an
//!   operator can raise it"). Through the runtime NO decrease is ever
//!   honored, whoever asks: an operator's path to moving a tier is editing
//!   the ruler they own, not a runtime API. An agent-originated movement is
//!   refused in every direction — an agent cannot raise a tier either, only
//!   an operator can (§17.1: "A tier may be raised by an operator; it is
//!   never lowered by an agent").

use gauntlet_domain::{Authority, BlastRadius, RoleId, authority_rank, blast_rank};
use gauntlet_ports::agent_runner::Isolation;

/// The containment rung a tier's dispatch requires (§17.3, §8.6).
///
/// Monotone with [`blast_rank`]: as blast radius rises, the required rung
/// never falls.
pub fn required_isolation(tier: BlastRadius) -> Isolation {
    match tier {
        BlastRadius::Contained => Isolation::None,
        BlastRadius::Local | BlastRadius::CrossCutting => Isolation::Process,
        // Never dispatched (§17.1 line 1); the top rung keeps the function
        // total and monotone — moot by construction, not by accident.
        BlastRadius::Irreversible => Isolation::Vm,
    }
}

/// Who is proposing a lattice movement on a Unit's recorded facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeOrigin {
    /// The operator, through whatever surface proposes the change. The
    /// runtime still refuses decreases (see the module doc); raises and
    /// no-ops are the operator's to make.
    Operator,
    /// A runtime role (builder, critic, verifier, …) — any movement is
    /// refused: roles execute the ruler, they do not edit it (AK-046).
    AgentRole(RoleId),
}

/// The verdict on a proposed lattice movement, with the structured refusal
/// the caller records as a governance event (AK-046: rejected movements are
/// RECORDED, never silently dropped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LatticeMovement {
    /// The movement may be honored (an operator raise, or no movement at
    /// all).
    Allowed,
    /// The movement is refused. The payload names every refused direction
    /// with the recorded and proposed values, so the governance event is
    /// reconstructable from the refusal alone.
    Refused {
        unit: gauntlet_domain::UnitId,
        blast_movement: Option<(BlastRadius, BlastRadius)>,
        authority_movement: Option<(Authority, Authority)>,
        rule: &'static str,
    },
}

impl LatticeMovement {
    /// Whether this verdict admits the movement.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Reviews a proposed movement on a Unit's recorded blast radius and
/// authority (§17.1, AK-046).
///
/// `recorded` and `proposed` are the (blast radius, authority) pairs. An
/// agent-originated proposal is refused whenever EITHER value moved, in ANY
/// direction; an operator proposal is refused on any DECREASE of either
/// rank (the runtime never lowers — the operator's path to a lower tier is
/// editing the ruler, which is outside this review entirely) and allowed on
/// raises and no-ops.
pub fn review_lattice_movement(
    unit: gauntlet_domain::UnitId,
    recorded: (BlastRadius, Authority),
    proposed: (BlastRadius, Authority),
    origin: &ChangeOrigin,
) -> LatticeMovement {
    let blast_movement = (recorded.0 != proposed.0).then_some((recorded.0, proposed.0));
    let authority_movement = (recorded.1 != proposed.1).then_some((recorded.1, proposed.1));
    if blast_movement.is_none() && authority_movement.is_none() {
        return LatticeMovement::Allowed;
    }
    let decreased = blast_movement
        .map(|(from, to)| blast_rank(to) < blast_rank(from))
        .unwrap_or(false)
        || authority_movement
            .map(|(from, to)| authority_rank(to) < authority_rank(from))
            .unwrap_or(false);
    let rule = match origin {
        ChangeOrigin::AgentRole(_) => {
            "an agent role cannot move a Unit's blast radius or authority in any direction — \
             only an operator can raise it (spec 17.1, AK-046)"
        }
        ChangeOrigin::Operator if decreased => {
            "the runtime never lowers a Unit's blast radius or authority — an operator's path \
             to a lower tier is editing the ruler, not a runtime movement (spec 17.1)"
        }
        ChangeOrigin::Operator => {
            // A raise: the one movement §17.1 admits.
            return LatticeMovement::Allowed;
        }
    };
    LatticeMovement::Refused {
        unit,
        blast_movement,
        authority_movement,
        rule,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::UnitId;

    fn unit(n: u16) -> UnitId {
        UnitId::new(&format!("AK-{n:03}")).expect("valid unit id")
    }

    #[test]
    fn the_rung_table_is_the_spec_posture_and_monotone_with_blast_rank() {
        use gauntlet_domain::BlastRadius;
        // §8.6: none-backend is permitted only at contained.
        assert_eq!(required_isolation(BlastRadius::Contained), Isolation::None);
        // §17.3: local and cross_cutting need a real backend; the runner
        // must report isolation ≥ process.
        assert_eq!(required_isolation(BlastRadius::Local), Isolation::Process);
        assert_eq!(
            required_isolation(BlastRadius::CrossCutting),
            Isolation::Process
        );
        // Total and monotone: as blast_rank rises the required rung never
        // falls.
        let ladder = [
            BlastRadius::Contained,
            BlastRadius::Local,
            BlastRadius::CrossCutting,
            BlastRadius::Irreversible,
        ];
        for pair in ladder.windows(2) {
            let (lower, higher) = (pair[0], pair[1]);
            assert!(blast_rank(lower) < blast_rank(higher));
            assert!(required_isolation(lower) <= required_isolation(higher));
        }
    }

    #[test]
    fn an_agent_cannot_move_a_unit_in_any_direction() {
        // AK-046 verbatim: "An agent role cannot lower a Unit's blast
        // radius or authority; only an operator can raise it." The runtime
        // reads the second clause strictly: raises are operator-only too.
        let builder = ChangeOrigin::AgentRole(RoleId::Builder);
        let critic = ChangeOrigin::AgentRole(RoleId::Critic);
        let recorded = (BlastRadius::Local, Authority::Critic);
        // Decrease each axis.
        let refused = review_lattice_movement(
            unit(1),
            recorded,
            (BlastRadius::Contained, Authority::Critic),
            &builder,
        );
        assert!(
            matches!(&refused, LatticeMovement::Refused { rule, .. } if rule.contains("agent role"))
        );
        let refused = review_lattice_movement(
            unit(2),
            recorded,
            (BlastRadius::Local, Authority::Critic),
            &critic,
        );
        assert!(refused.is_allowed(), "no movement is always allowed");
        // Raise attempts by an agent are refused as well.
        let refused = review_lattice_movement(
            unit(3),
            recorded,
            (BlastRadius::CrossCutting, Authority::LeadAgent),
            &builder,
        );
        assert!(
            !refused.is_allowed(),
            "an agent cannot raise a tier either — only an operator can"
        );
    }

    #[test]
    fn an_operator_may_raise_and_may_not_lower() {
        let operator = ChangeOrigin::Operator;
        let recorded = (BlastRadius::Local, Authority::Critic);
        // Raise either axis.
        assert!(
            review_lattice_movement(
                unit(1),
                recorded,
                (BlastRadius::CrossCutting, Authority::Critic),
                &operator
            )
            .is_allowed()
        );
        assert!(
            review_lattice_movement(
                unit(2),
                recorded,
                (BlastRadius::Local, Authority::LeadAgent),
                &operator
            )
            .is_allowed()
        );
        // Lower either axis: refused, with the refused values carried.
        let refused = review_lattice_movement(
            unit(3),
            recorded,
            (BlastRadius::Contained, Authority::Critic),
            &operator,
        );
        match refused {
            LatticeMovement::Refused {
                blast_movement,
                authority_movement,
                ..
            } => {
                assert_eq!(
                    blast_movement,
                    Some((BlastRadius::Local, BlastRadius::Contained))
                );
                assert_eq!(authority_movement, None);
            }
            other => panic!("an operator decrease must be refused, got {other:?}"),
        }
        assert!(
            !review_lattice_movement(
                unit(4),
                recorded,
                (BlastRadius::Contained, Authority::Critic),
                &operator
            )
            .is_allowed()
        );
        // A no-op movement is allowed for an operator too.
        assert!(
            review_lattice_movement(
                unit(5),
                recorded,
                (BlastRadius::Local, Authority::Critic),
                &operator
            )
            .is_allowed()
        );
    }
}
