//! Governance: authority decides WHETHER, containment decides HOW
//! CONTAINED (spec §17; plan Task 14; defect D2's correction).
//!
//! Both checks precede builder process start, in that order, and neither
//! substitutes for the other (I9): a perfectly validated sandbox policy
//! does not admit a unit whose authority the run cannot exercise, and
//! critic authority does not admit a local-tier unit without containment.
//! [`posture`] fixes the tier→isolation-rung table and the lattice-change
//! review (AK-046); [`gate`] is the §17.1 authority gate plus the
//! capability/policy/validation conjuncts every dispatch passes through
//! BEFORE the runner is invoked (AK-044/AK-045/AK-047/AK-048/AK-050).

pub mod gate;
pub mod posture;

pub use gate::{
    AuthorityGate, BuilderCommand, GateAdmission, GateFacts, GateRefusal, GovernDeps, GovernError,
    WrapRefusal,
};
pub use posture::{ChangeOrigin, LatticeMovement, required_isolation, review_lattice_movement};
