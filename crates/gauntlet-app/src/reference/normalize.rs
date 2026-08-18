//! Reference normalization and digest (plan Task 7 step 1) — a byte-for-byte
//! reproduction of `scripts/reference-digest.sh`, the deliberately primitive
//! bootstrap tool that existed before any Rust code did. Two independent
//! implementations of the same rules agreeing on real content is the
//! determinism proof (plan Task 1 teeth; AK-021): the task gate diffs this
//! module's output against the script's.
//!
//! The rules, exhaustive (the script's header is the authority; adding a
//! rule here without adding it there is a ruler change, not an
//! implementation choice):
//!
//! 1. Input is raw bytes; invalid UTF-8 is a typed error — the script
//!    errors, and untrusted input is never silently coerced.
//! 2. CRLF and lone CR line endings become LF.
//! 3. A run of trailing blank lines collapses so the normalized text ends
//!    with exactly one trailing LF; an empty (or all-blank-lines) input
//!    normalizes to zero bytes. Precisely: split on `\n`, drop trailing
//!    `""` elements (the final newline plus any additional blank lines),
//!    rejoin with exactly one trailing newline unless nothing remains. A
//!    final content line with a single `\n` is unchanged; a file ending
//!    with no newline gains one. Interior blank lines are preserved.
//! 4. Nothing else. Per-line leading, trailing, and internal whitespace and
//!    line order are preserved exactly — no sorting, no trimming. This is
//!    deliberate: appending one trailing space to any content line MUST
//!    change the digest (an implementation that also stripped per-line
//!    trailing whitespace would make the ruler insensitive to real edits
//!    and is a defect, not a convenience).
//!
//! The digest is SHA-256 (the algorithm token lives in the §8.6 defaults
//! table) over the normalized UTF-8 bytes, carried by the domain `Digest`
//! in lowercase hex.

use gauntlet_domain::Digest;
use sha2::{Digest as Sha2Digest, Sha256};

use crate::config::defaults::DIGEST_ALGO;

/// Normalization failures — typed, never a coerced success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizeError {
    /// The artifact is not valid UTF-8. `valid_up_to` is the byte offset
    /// where decoding stopped — the locus, for diagnosing which bytes are
    /// foreign. The script exits non-zero on the same input; the Rust side
    /// refuses identically instead of coercing.
    NotUtf8 { valid_up_to: usize },
}

impl core::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotUtf8 { valid_up_to } => write!(
                f,
                "the reference artifact is not valid UTF-8 (first invalid byte at offset \
                 {valid_up_to}); untrusted input is never coerced"
            ),
        }
    }
}

impl std::error::Error for NormalizeError {}

/// Rule 1: raw bytes become valid text, or a typed error.
pub fn text_from_bytes(raw: &[u8]) -> Result<String, NormalizeError> {
    match std::str::from_utf8(raw) {
        Ok(text) => Ok(text.to_owned()),
        Err(err) => Err(NormalizeError::NotUtf8 {
            valid_up_to: err.valid_up_to(),
        }),
    }
}

/// Rules 2-4: the normalization itself, over already-valid text. Pure.
///
/// Line-ending folding first (`\r\n` before `\r`, exactly as the script
/// orders its replaces), then the trailing-blank collapse: a line that is
/// whitespace-but-not-empty is CONTENT — it stops the collapse and, if it
/// is last, is preserved verbatim before the single trailing newline.
pub fn normalize(text: &str) -> String {
    let folded = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = folded.split('\n').collect();
    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut normalized = String::with_capacity(folded.len() + 1);
    normalized.push_str(&lines.join("\n"));
    normalized.push('\n');
    normalized
}

/// Normalizes raw artifact bytes (rule 1 included) into the normalized
/// byte string the digest covers and `freeze` stores.
pub fn normalize_bytes(raw: &[u8]) -> Result<Vec<u8>, NormalizeError> {
    Ok(normalize(&text_from_bytes(raw)?).into_bytes())
}

/// SHA-256 over the normalized bytes as the domain `Digest`. The algorithm
/// token comes from the §8.6 defaults table; the hex is lowercase by
/// construction.
///
/// The `Digest::new` refusal branch is unreachable by established
/// invariant, not hope: the algorithm token is the defaults-table `const`
/// and the hex payload is exactly the 64 lowercase hex characters SHA-256
/// produced here. The fingerprint service (`config::fingerprint`) holds the
/// same invariant for the same reason.
pub fn digest_normalized(normalized: &[u8]) -> Digest {
    let hashed = Sha256::digest(normalized);
    let hex: String = hashed.iter().map(|byte| format!("{byte:02x}")).collect();
    Digest::new(DIGEST_ALGO, &hex).unwrap_or_else(|err| {
        unreachable!(
            "the defaults-table algorithm token plus 64 lowercase hex characters is \
                      always a valid digest: {err}"
        )
    })
}

/// The whole pipeline over raw bytes: validate UTF-8, normalize, digest.
pub fn digest_of(raw: &[u8]) -> Result<Digest, NormalizeError> {
    Ok(digest_normalized(&normalize_bytes(raw)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-256 of the empty string — the pin the all-blank case must equal.
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    /// SHA-256 of the exact bytes `alpha\nbeta\n` (computed against the
    /// script's own rules, independently of this implementation).
    const ALPHA_SHA256: &str = "e49c81e2d2f84e259d40e2fb8192f3bcd198b355184845d76d8f58807d0d78ee";
    const ALPHA_TRAILING_SPACE_SHA256: &str =
        "5518ad7ad0bf17fedbf1de6fbec1220f428c9b8de5157131e14040509f40907d";
    const ALPHA_BLANK_INTERIOR_SHA256: &str =
        "0e937c34625ef864d694878f01fd96318893d29014a51536006dacd150d809ec";
    const REORDERED_SHA256: &str =
        "3588d4ce80593f91177fe39f97f96fece7050ebc8e030a2a92a7f61e67f07af9";

    /// AK-021's determinism face: editor-drift that carries no content
    /// change — CRLF vs LF vs lone-CR endings, and a final newline present,
    /// absent, or doubled — yields ONE digest. Every spelling below says
    /// exactly `alpha\nbeta\n` after normalization.
    #[test]
    fn normalization_is_deterministic_across_line_ending_and_final_newline_spellings() {
        let canonical = "alpha\nbeta\n";
        let spellings = [
            canonical.to_owned(),
            "alpha\nbeta".to_owned(),
            "alpha\nbeta\n\n".to_owned(),
            "alpha\nbeta\n\n\n\n".to_owned(),
            "alpha\r\nbeta\r\n".to_owned(),
            "alpha\r\nbeta".to_owned(),
            "alpha\rbeta\r".to_owned(),
            "alpha\rbeta".to_owned(),
            "alpha\rbeta\r\r".to_owned(),
        ];
        let expected = digest_of(canonical.as_bytes()).expect("canonical digests");
        for spelling in spellings {
            let got = digest_of(spelling.as_bytes())
                .unwrap_or_else(|err| panic!("{spelling:?} must normalize: {err}"));
            assert_eq!(
                got, expected,
                "the spelling {spelling:?} must digest identically to {canonical:?}"
            );
        }
    }

    /// The teeth face of the same rule: the digest stays maximally sensitive
    /// to content. One trailing space on a content line, one interior blank
    /// line, and reordering two lines are three different digests, all
    /// different from the plain two-line form.
    #[test]
    fn normalization_is_sensitive_to_content_edits() {
        let plain = digest_of(b"alpha\nbeta\n").expect("plain digests");
        assert_eq!(plain.hex(), ALPHA_SHA256);
        // One trailing space on a content line changes the digest.
        let spaced = digest_of(b"alpha \nbeta\n").expect("spaced digests");
        assert_eq!(spaced.hex(), ALPHA_TRAILING_SPACE_SHA256);
        assert_ne!(spaced, plain);
        // Leading and internal whitespace are preserved too.
        assert_ne!(
            digest_of(b"alpha\n beta\n").expect("leading space digests"),
            plain
        );
        assert_ne!(
            digest_of(b"al pha\nbeta\n").expect("internal space digests"),
            plain
        );
        // An interior blank line changes the digest (only trailing ones
        // collapse).
        let blanked = digest_of(b"alpha\n\nbeta\n").expect("blanked digests");
        assert_eq!(blanked.hex(), ALPHA_BLANK_INTERIOR_SHA256);
        assert_ne!(blanked, plain);
        // Reordering two lines changes the digest — nothing is sorted.
        let reordered = digest_of(b"beta\nalpha\n").expect("reordered digests");
        assert_eq!(reordered.hex(), REORDERED_SHA256);
        assert_ne!(reordered, plain);
    }

    /// A whitespace-only last line is content, not a blank line: the
    /// trailing collapse pops only `""` elements, so `alpha\n  \n` keeps
    /// the two-space line and digests differently from `alpha\n`.
    #[test]
    fn a_whitespace_only_last_line_is_content_not_a_blank_line() {
        assert_eq!(normalize("alpha\n"), "alpha\n".to_owned());
        assert_eq!(normalize("alpha\n  \n"), "alpha\n  \n".to_owned());
        assert_eq!(normalize("alpha\n  \n\n\n"), "alpha\n  \n".to_owned());
        assert_ne!(
            digest_of(b"alpha\n  \n").expect("whitespace tail digests"),
            digest_of(b"alpha\n").expect("clean tail digests")
        );
    }

    /// Empty and all-blank-lines inputs both normalize to zero bytes and
    /// the empty-string digest; a single content line with or without its
    /// final newline is the same normalized form.
    #[test]
    fn empty_and_all_blank_inputs_normalize_to_zero_bytes() {
        for empty_spelling in ["", "\n", "\n\n\n", "\r\n", "\r"] {
            let normalized = normalize_bytes(empty_spelling.as_bytes())
                .unwrap_or_else(|err| panic!("{empty_spelling:?} must parse: {err}"));
            assert!(
                normalized.is_empty(),
                "{empty_spelling:?} must normalize to zero bytes, got {normalized:?}"
            );
        }
        let digest = digest_of(b"").expect("empty digests");
        assert_eq!(digest.hex(), EMPTY_SHA256);
        assert_eq!(
            digest_of(b"\n\n\n").expect("all-blank digests"),
            digest,
            "all-blank-lines input is the empty input"
        );
        assert_eq!(normalize("alpha"), "alpha\n".to_owned());
    }

    /// Invalid UTF-8 is a typed error carrying the byte locus, never a
    /// coerced success and never a panic on untrusted input.
    #[test]
    fn invalid_utf8_is_a_typed_error_with_the_byte_locus() {
        let error = digest_of(b"alpha\n\xffbeta\n").expect_err("invalid UTF-8 is refused");
        assert_eq!(error, NormalizeError::NotUtf8 { valid_up_to: 6 }, "{error}");
        // The text-level entry agrees on the locus.
        assert_eq!(
            text_from_bytes(b"\xff"),
            Err(NormalizeError::NotUtf8 { valid_up_to: 0 })
        );
    }

    /// The normalization function pinned on short synthetic inputs with the
    /// exact expected output strings — not just digests, so a future edit
    /// to the rules cannot hide behind an unchanged collision.
    #[test]
    fn short_inputs_pin_the_exact_normalized_output() {
        let cases: &[(&str, &str)] = &[
            ("", ""),
            ("one", "one\n"),
            ("one\n", "one\n"),
            ("one\n\n", "one\n"),
            ("one\n\n\n\n", "one\n"),
            ("one\r\ntwo\r\n", "one\ntwo\n"),
            ("one\rtwo", "one\ntwo\n"),
            ("one\r\n\r\ntwo", "one\n\ntwo\n"),
            ("one\n\n\nthree", "one\n\n\nthree\n"),
            ("  kept  \n\tkept\t\n", "  kept  \n\tkept\t\n"),
            ("\n\none", "\n\none\n"),
            ("one\n  \n\n", "one\n  \n"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                &normalize(input),
                expected,
                "normalize({input:?}) must be {expected:?}"
            );
        }
        // Through the byte entry the same pins hold, byte for byte.
        for (input, expected) in cases {
            let normalized = normalize_bytes(input.as_bytes())
                .unwrap_or_else(|err| panic!("{input:?} must be UTF-8: {err}"));
            assert_eq!(normalized, expected.as_bytes());
        }
    }
}
