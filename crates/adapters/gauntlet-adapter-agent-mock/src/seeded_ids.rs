//! Seeded `IdGenerator` adapter (spec §6.2: "`run_id()` · `invocation_id()`
//! — Injectable; tests use a seeded generator"; Annex E test hook
//! `GAUNTLET_ID_SEED`).
//!
//! ## Derivation (documented, deterministic, not cryptographic)
//!
//! A test identity needs three properties — same seed replays the same
//! sequence, different seeds diverge, and every minted id passes the
//! domain's ULID grammar. The derivation:
//!
//! 1. `seed_hash = FNV-1a 64` over the seed's UTF-8 bytes (public-domain
//!    hash; not cryptographic, and test identity needs no adversary
//!    resistance — the spec wants determinism, not unpredictability).
//! 2. Draw `n` (one `run_id` or `invocation_id` mint, `n` from a single
//!    shared counter so the two kinds never collide inside one generator)
//!    produces two 64-bit words `w0 = mix(seed_hash, 2n)` and
//!    `w1 = mix(seed_hash, 2n + 1)`, where `mix` is the splitmix64
//!    finalizer (public domain) applied to `h ^ (k · 0x9E3779B97F4A7C15)`
//!    — an avalanche step, so consecutive `n` yield independent-looking
//!    words.
//! 3. The ULID's 26 characters read consecutive 5-bit windows of the
//!    128-bit stream `w0 ‖ w1` through the Crockford Base32 alphabet
//!    (`0-9 A-Z` minus `I L U O`, the §5.2 grammar). Character 25 reads
//!    the final 3 bits zero-padded — the last character is drawn from the
//!    first 8 alphabet entries only; a documented, harmless bias for test
//!    identity.
//!
//! The domain remains the validator: every minted id is constructed through
//! `RunId::new`/`InvocationId::new`, so a derivation bug is a typed
//! failure at the boundary, never a silent invalid id.

use core::cell::Cell;
use core::fmt;

use gauntlet_domain::{InvocationId, RunId};
use gauntlet_ports::id_generator::IdGenerator;

/// The Crockford Base32 alphabet a ULID is written in (spec §5.2).
const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// A ULID is 26 characters (spec §5.2).
const ULID_LEN: usize = 26;
/// FNV-1a 64 offset basis and prime (public domain).
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
/// The golden-ratio constant feeding the splitmix64 avalanche.
const SKEW: u64 = 0x9E37_79B9_7F4A_7C15;

/// A seed must be present to be a seed. Typed, not a silent default — an
/// empty `GAUNTLET_ID_SEED` selecting a fixed sequence by accident is
/// exactly the bug this error exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptySeed;

impl fmt::Display for EmptySeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a seeded id generator needs a non-empty seed (GAUNTLET_ID_SEED)")
    }
}

impl std::error::Error for EmptySeed {}

/// The splitmix64 finalizer (public domain): one avalanche round.
fn mix(h: u64, k: u64) -> u64 {
    let mut z = h ^ k.wrapping_mul(SKEW);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a 64 over the seed bytes.
fn fnv1a(seed: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Character `i` of the ULID: 5 consecutive bits of the 128-bit stream
/// `w0 ‖ w1`, little-endian by index. The last window (bits 125..130) is
/// zero-padded past bit 128.
fn window(w0: u64, w1: u64, index: usize) -> usize {
    let start = index * 5;
    if start >= 64 {
        ((w1 >> (start - 64)) & 0x1F) as usize
    } else if start + 5 <= 64 {
        ((w0 >> start) & 0x1F) as usize
    } else {
        // The window straddles the word boundary.
        let low_bits = 64 - start;
        let low = (w0 >> start) & ((1u64 << low_bits) - 1);
        let high = w1 & ((1u64 << (5 - low_bits)) - 1);
        (low | (high << low_bits)) as usize
    }
}

/// Mints the ULID text for draw `n` under `seed_hash`.
fn ulid_of(seed_hash: u64, n: u64) -> String {
    let w0 = mix(seed_hash, n.wrapping_mul(2));
    let w1 = mix(seed_hash, n.wrapping_mul(2).wrapping_add(1));
    let mut out = String::with_capacity(ULID_LEN);
    for index in 0..ULID_LEN {
        out.push(CROCKFORD[window(w0, w1, index)] as char);
    }
    out
}

/// A deterministic id source: same seed, same sequence; every id passes the
/// domain ULID grammar by construction.
///
/// Single-threaded by design (a `Cell` counter): test doubles share one
/// generator by reference, and the spec's injectability need is
/// determinism, not concurrency.
#[derive(Debug, Clone)]
pub struct SeededIdGenerator {
    seed_hash: u64,
    counter: Cell<u64>,
}

impl SeededIdGenerator {
    /// Builds the generator from a non-empty seed.
    pub fn new(seed: &str) -> Result<Self, EmptySeed> {
        if seed.is_empty() {
            return Err(EmptySeed);
        }
        Ok(Self {
            seed_hash: fnv1a(seed),
            counter: Cell::new(0),
        })
    }

    /// The next id text in the sequence, consuming one draw.
    fn next_ulid(&self) -> String {
        let n = self.counter.get();
        self.counter.set(n.wrapping_add(1));
        ulid_of(self.seed_hash, n)
    }

    /// How many ids this generator has minted — lets a test assert the
    /// shared-counter behavior without reaching into the derivation.
    pub fn draws(&self) -> u64 {
        self.counter.get()
    }
}

impl IdGenerator for SeededIdGenerator {
    fn run_id(&self) -> RunId {
        RunId::new(&self.next_ulid()).expect(
            "26 Crockford characters are a valid ULID by \
             construction (spec 5.2) — a failure here is a derivation bug, not input",
        )
    }

    fn invocation_id(&self) -> InvocationId {
        InvocationId::new(&self.next_ulid()).expect(
            "26 Crockford characters are a valid ULID \
             by construction (spec 5.2) — a failure here is a derivation bug, not input",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::SeededIdGenerator;
    use gauntlet_ports::id_generator::IdGenerator;

    /// The ports crate's own id-collision sampling precedent draws eight
    /// ids; this crate matches that bound and exceeds it.
    const DRAWS: usize = 16;

    #[test]
    fn an_empty_seed_is_a_typed_error() {
        assert!(matches!(SeededIdGenerator::new(""), Err(super::EmptySeed)));
    }

    #[test]
    fn the_same_seed_replays_the_same_sequence() {
        let first = SeededIdGenerator::new("seed-alpha").expect("non-empty seed");
        let second = SeededIdGenerator::new("seed-alpha").expect("non-empty seed");
        for _ in 0..DRAWS {
            assert_eq!(first.run_id(), second.run_id());
            assert_eq!(first.invocation_id(), second.invocation_id());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        // A small battery: every pair of distinct seeds must part ways on
        // their first draws (the collision space per draw is 128 bits, so
        // any agreement is a derivation defect, not bad luck).
        let seeds = [
            "seed-alpha",
            "seed-beta",
            "a",
            "gauntlet",
            "0",
            "seed-alpha ",
        ];
        for (index, left) in seeds.iter().enumerate() {
            for right in seeds.iter().skip(index + 1) {
                let one = SeededIdGenerator::new(left).expect("non-empty seed");
                let two = SeededIdGenerator::new(right).expect("non-empty seed");
                assert_ne!(
                    one.run_id(),
                    two.run_id(),
                    "seeds {left:?} and {right:?} produced the same first run id"
                );
                assert_ne!(
                    one.invocation_id(),
                    two.invocation_id(),
                    "seeds {left:?} and {right:?} produced the same first invocation id"
                );
            }
        }
    }

    #[test]
    fn minted_ids_are_distinct_across_kinds_and_draws() {
        let generator = SeededIdGenerator::new("seed-collision").expect("non-empty seed");
        let mut seen: Vec<String> = Vec::new();
        for _ in 0..DRAWS {
            seen.push(generator.run_id().to_string());
            seen.push(generator.invocation_id().to_string());
        }
        // O(n^2) distinctness over a 32-entry battery: the point is the
        // invariant, not the algorithm.
        for (index, id) in seen.iter().enumerate() {
            assert!(
                !seen.iter().skip(index + 1).any(|later| later == id),
                "the id {id} was minted twice from one generator"
            );
        }
        assert_eq!(generator.draws(), (DRAWS * 2) as u64);
    }

    #[test]
    fn every_minted_id_passes_the_domain_grammar() {
        // run_id/invocation_id constructors already validate; this pins the
        // alphabet explicitly so a future derivation change that silently
        // emitted, say, lowercase cannot pass by touching only the tests.
        let generator = SeededIdGenerator::new("seed-grammar").expect("non-empty seed");
        for _ in 0..DRAWS {
            for id in [
                generator.run_id().to_string(),
                generator.invocation_id().to_string(),
            ] {
                assert_eq!(id.len(), 26);
                assert!(
                    id.bytes().all(|b| super::CROCKFORD.contains(&b)),
                    "id {id} left the Crockford alphabet"
                );
            }
        }
    }
}
