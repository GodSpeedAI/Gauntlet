//! Production id adapter (spec §4.1 `IdGenerator`, §6.2: "Production
//! adapters mint ULIDs"; plan Task 20): the composition root's id
//! source for real runs.
//!
//! The `IdGenerator` port is injectable, and until this adapter existed
//! only the mock crate's seeded double did — a generator whose ids
//! REPLAY per seed, unusable for production (two runs under one seed
//! would mint the same run id, and `record_run`'s idempotence would
//! silently fold them into one identity). This adapter mints ids whose
//! 128-bit stream is seeded once from the operating system's random
//! device and advanced by a monotonic per-process counter: unique
//! within the process by the counter, unique across restarts by the
//! entropy, and grammar-valid by construction (the seeded sibling's
//! proven encoding).
//!
//! Construction reads the entropy ONCE and fails typed; `run_id()` /
//! `invocation_id()` are the port's infallible shape — after
//! construction, minting cannot fail (the counter never repeats under
//! wrapping by construction of the advance).

use std::cell::Cell;
use std::io::Read;

use gauntlet_domain::{InvocationId, RunId};
use gauntlet_ports::id_generator::IdGenerator;

/// The random device this adapter reads its seed entropy from.
const RANDOM_DEVICE: &str = "/dev/urandom";

/// The entropy draw, in bytes (128 bits — one full ULID stream).
const ENTROPY_BYTES: usize = 16;

/// The Crockford Base32 alphabet a ULID is written in (spec §5.2) — the
/// seeded sibling's own table.
const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A ULID is 26 characters (spec §5.2).
const ULID_LEN: usize = 26;

/// The golden-ratio constant feeding the splitmix64 avalanche.
const SKEW: u64 = 0x9E37_79B9_7F4A_7C15;

/// Why the generator could not be constructed: the entropy source was
/// unreadable or short. Typed — a composition that cannot seed its ids
/// refuses to start, never falls back to a guessable stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdEntropyError {
    /// The random device could not be opened.
    Unavailable { detail: String },
    /// The random device delivered fewer bytes than the draw.
    ShortRead,
}

impl core::fmt::Display for IdEntropyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unavailable { detail } => {
                write!(
                    f,
                    "the id entropy source {RANDOM_DEVICE} is unavailable: {detail}"
                )
            }
            Self::ShortRead => write!(
                f,
                "the id entropy source {RANDOM_DEVICE} delivered fewer than {ENTROPY_BYTES} bytes"
            ),
        }
    }
}

impl std::error::Error for IdEntropyError {}

/// The splitmix64 finalizer (public domain): one avalanche round.
fn mix(h: u64, k: u64) -> u64 {
    let mut z = h ^ k.wrapping_mul(SKEW);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Character `i` of the ULID: 5 consecutive bits of the 128-bit stream
/// `w0 ‖ w1`, little-endian by index (the seeded sibling's encoding —
/// grammar-valid by construction).
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

fn ulid_text(w0: u64, w1: u64) -> String {
    let mut out = String::with_capacity(ULID_LEN);
    for index in 0..ULID_LEN {
        out.push(CROCKFORD[window(w0, w1, index)] as char);
    }
    out
}

/// The production id source: OS-entropy seed, monotonic counter stream.
#[derive(Debug, Clone)]
pub struct UlidIdGenerator {
    seed_w0: u64,
    seed_w1: u64,
    counter: Cell<u64>,
}

impl UlidIdGenerator {
    /// Draws the seed entropy once. Fails typed when the operating
    /// system's random device cannot serve the draw.
    pub fn new() -> Result<Self, IdEntropyError> {
        let mut entropy = [0u8; ENTROPY_BYTES];
        let mut device =
            std::fs::File::open(RANDOM_DEVICE).map_err(|error| IdEntropyError::Unavailable {
                detail: error.to_string(),
            })?;
        device.read_exact(&mut entropy).map_err(|error| {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                IdEntropyError::ShortRead
            } else {
                IdEntropyError::Unavailable {
                    detail: error.to_string(),
                }
            }
        })?;
        let mut seed_w0 = [0u8; 8];
        let mut seed_w1 = [0u8; 8];
        seed_w0.copy_from_slice(&entropy[..8]);
        seed_w1.copy_from_slice(&entropy[8..]);
        Ok(Self {
            seed_w0: u64::from_le_bytes(seed_w0),
            seed_w1: u64::from_le_bytes(seed_w1),
            counter: Cell::new(0),
        })
    }

    /// Mints the next id: draw `n` mixes the counter into both seed
    /// words. The counter advance is monotonic, so ids never repeat
    /// within the process.
    fn mint(&self) -> String {
        let n = self.counter.get();
        self.counter.set(n.wrapping_add(1));
        let w0 = mix(self.seed_w0, n);
        let w1 = mix(self.seed_w1, n.wrapping_add(1));
        ulid_text(w0, w1)
    }
}

impl IdGenerator for UlidIdGenerator {
    fn run_id(&self) -> RunId {
        // A minted id is 26 Crockford characters by construction; the
        // domain's validator re-checks the grammar and its refusal is
        // unreachable — converted into a panic only at that
        // impossibility (the seeded sibling's convention).
        RunId::new(&self.mint()).expect("a minted ULID passes the domain grammar")
    }

    fn invocation_id(&self) -> InvocationId {
        InvocationId::new(&self.mint()).expect("a minted ULID passes the domain grammar")
    }
}

#[cfg(test)]
mod tests {
    use super::UlidIdGenerator;
    use gauntlet_ports::id_generator::IdGenerator;

    #[test]
    fn minted_ids_are_unique_within_the_process() {
        let generator = UlidIdGenerator::new().expect("entropy is available in tests");
        let first = generator.run_id();
        let second = generator.run_id();
        let third = generator.invocation_id();
        assert_ne!(first, second, "the monotonic counter never repeats");
        assert_ne!(
            first.to_string(),
            third.to_string(),
            "the run and invocation draws never collide"
        );
        assert_ne!(
            second.to_string(),
            third.to_string(),
            "the run and invocation draws never collide"
        );
    }

    #[test]
    fn minted_ids_carry_run_and_invocation_distinct_types() {
        let generator = UlidIdGenerator::new().expect("entropy is available in tests");
        let run = generator.run_id();
        let invocation = generator.invocation_id();
        // Distinct types: an invocation id cannot stand in for a run id
        // (compile-enforced); both render 26 uppercase Crockford chars.
        for id in [run.to_string(), invocation.to_string()] {
            assert_eq!(id.len(), 26, "{id}");
            assert!(
                id.bytes()
                    .all(|byte| byte.is_ascii_digit() || byte.is_ascii_uppercase()),
                "{id}"
            );
        }
        assert_ne!(
            run.to_string(),
            invocation.to_string(),
            "the shared counter never repeats"
        );
    }
}
