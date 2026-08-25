//! The production [`ReferenceProvider`] (plan Task 7 D4): a file-backed
//! implementation over the frozen `.wayfinder/answer-key.md` and
//! `.wayfinder/reference.yaml`, binding them into the §6.2 port pipeline:
//! resolve → normalize → digest → freeze → `verify_unchanged`.
//!
//! The ruler is never written from here. `freeze` writes the NORMALIZED
//! answer-key bytes into the caller's evidence store (content-addressed,
//! write-once, then pinned to the run); it digests the bytes it actually
//! reads and fires the I1 tripwire if they moved since `normalize`; it
//! returns the run's one [`ReferenceVersion`]. `verify_unchanged` re-reads
//! the answer key, re-normalizes, re-digests, and compares to the frozen
//! version's digest — the same I1 tripwire (spec §6.2, §11.5).
//!
//! ## The artifact-source seam
//!
//! The answer-key and manifest content can arrive from a filesystem path
//! (production) or from a shared in-memory string (the shared contract
//! suite drives the artifact through the latter — spec §13.2's "fresh
//! context" is structural, and the suite rewrites the standard mid-run to
//! trip the I1 wire). The seam is exactly [`AnswerKeySource`] and
//! [`ManifestSource`]; nothing else branches on provenance.
//!
//! ## Recorded decision: structural strictness lives at admission, not in
//! the port's `normalize`
//!
//! The frozen shared contract suite (Task 4) drives the provider with an
//! artifact that is a plain phrase, not a well-formed answer key
//! (`"the standard text"`), and expects `normalize` to succeed on it — the
//! suite is testing pipeline semantics (round-trip, freeze-then-verify,
//! tripwire), not answer-key structure. A port `normalize` that refused any
//! parse failure would fail the suite by construction. So:
//!
//! - the port `normalize` enforces the DOMAIN invariants via
//!   [`Reference::new`] (id/version/digest present, narrative non-empty,
//!   unique ids, hard-units present, unknown-naming rules);
//! - strict per-block answer-key validation is enforced where structure is
//!   admitted. The strict-admission surface today: `parse_all_or_fail` in
//!   `reference/parse.rs` (an all-or-nothing API; its production caller is
//!   the Task-12 controller, not yet built), doctor check 2's "unit schema"
//!   deep sub-result in `doctor/checks.rs` (parses the configured answer
//!   key; per-block failures are check evidence), and the P1 proof's
//!   clean-parse assertion over its seeded copy in
//!   `gauntlet-cli/tests/p1_reference_integrity.rs`.
//!
//! This is a recorded decision, not a silent weakening: the units that reach
//! [`ReferenceDefinition`] are the well-formed Units from the per-block
//! parser, so a malformed block yields that block's Unit missing at the
//! port, and the strict admission callsites report the actual block
//! failures with their loci.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gauntlet_domain::{Digest, EvidenceKind, Reference, ReferenceVersion, RunId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::reference_provider::{
    ReferenceDraft, ReferenceProvider, ReferenceProviderError,
};

use crate::reference::manifest::{ManifestDefinition, build_definition, parse_manifest};
use crate::reference::normalize::{digest_normalized, digest_of, normalize_bytes, text_from_bytes};
use crate::reference::parse::parse_answer_key;

/// How the answer-key content is reached: a filesystem path (production) or
/// a shared in-memory string (the contract suite drives the artifact this
/// way, rewriting it mid-run to trip the I1 wire).
#[derive(Clone)]
pub enum AnswerKeySource {
    Path(PathBuf),
    Shared(Rc<RefCell<String>>),
}

impl AnswerKeySource {
    fn read(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Path(path) => std::fs::read(path)
                .map_err(|err| format!("reading the answer key at {:?} failed: {err}", path)),
            Self::Shared(text) => Ok(text.borrow().as_bytes().to_vec()),
        }
    }

    /// Reads the current answer-key TEXT — the per-tick re-read path the
    /// Task-12 controller uses (§11.4 step 3: "Re-read the answer key …
    /// The in-memory snapshot is never authoritative over the file").
    /// Unavailable reads surface as the caller's typed Unavailable class;
    /// this returns the raw read failure verbatim.
    pub fn read_text(&self) -> Result<String, String> {
        let bytes = self.read()?;
        String::from_utf8(bytes).map_err(|err| format!("the answer key is not valid UTF-8: {err}"))
    }

    /// The source's own description, for typed refusals that must name
    /// where the answer key was sought.
    pub fn describe(&self) -> String {
        match self {
            Self::Path(path) => format!("{:?}", path),
            Self::Shared(_) => "<shared artifact>".to_owned(),
        }
    }
}

/// How the manifest content is reached: a filesystem path (production) or
/// an in-memory string (the contract suite supplies a minimal manifest).
#[derive(Clone)]
pub enum ManifestSource {
    Path(PathBuf),
    Inline(String),
}

impl ManifestSource {
    fn read(&self) -> Result<String, String> {
        match self {
            Self::Path(path) => std::fs::read_to_string(path)
                .map_err(|err| format!("reading the manifest at {:?} failed: {err}", path)),
            Self::Inline(text) => Ok(text.clone()),
        }
    }
}

/// The production, file-backed [`ReferenceProvider`].
///
/// Constructed over an [answer-key source], a [manifest source], and the
/// evidence store that `freeze` writes the normalized key into. Holds no
/// clock and performs single reads/puts; the `Deadline` is noted and not
/// consulted (no waiting code exists here to bound) — the same convention
/// as the fs evidence store.
pub struct FileReferenceProvider {
    answer_key: AnswerKeySource,
    manifest: ManifestSource,
    evidence: Rc<dyn EvidenceStore>,
}

impl FileReferenceProvider {
    /// A production provider over file paths.
    pub fn from_paths(
        answer_key: PathBuf,
        manifest: PathBuf,
        evidence: Rc<dyn EvidenceStore>,
    ) -> Self {
        Self {
            answer_key: AnswerKeySource::Path(answer_key),
            manifest: ManifestSource::Path(manifest),
            evidence,
        }
    }

    /// A provider over a shared answer-key string and an inline manifest —
    /// the shape the shared contract suite builds. Production code should
    /// prefer [`Self::from_paths`].
    pub fn from_sources(
        answer_key: AnswerKeySource,
        manifest: ManifestSource,
        evidence: Rc<dyn EvidenceStore>,
    ) -> Self {
        Self {
            answer_key,
            manifest,
            evidence,
        }
    }

    fn sources(&self) -> Vec<String> {
        vec![
            self.answer_key.describe(),
            match &self.manifest {
                ManifestSource::Path(path) => format!("{:?}", path),
                ManifestSource::Inline(_) => "<inline manifest>".to_owned(),
            },
        ]
    }

    /// Reads and strictly parses the manifest half of the ruler into the
    /// fields the definition carries (spec §5.3.1); a malformed manifest is
    /// a typed `ResolveFailed` naming the parser's locus.
    fn read_manifest(&self) -> Result<ManifestDefinition, ReferenceProviderError> {
        let text = self
            .manifest
            .read()
            .map_err(|detail| ReferenceProviderError::ResolveFailed { detail })?;
        parse_manifest(&text).map_err(|err| ReferenceProviderError::ResolveFailed {
            detail: format!("parsing the manifest failed: {err}"),
        })
    }
}

impl ReferenceProvider for FileReferenceProvider {
    fn resolve(
        &self,
        _intent: &str,
        _deadline: Deadline,
    ) -> Result<ReferenceDraft, ReferenceProviderError> {
        let key_bytes =
            self.answer_key
                .read()
                .map_err(|detail| ReferenceProviderError::ResolveFailed {
                    detail: format!("{}: {detail}", self.answer_key.describe()),
                })?;
        let key_text =
            text_from_bytes(&key_bytes).map_err(|err| ReferenceProviderError::ResolveFailed {
                detail: format!("the answer key is not valid UTF-8: {err}"),
            })?;
        // Per-block isolation: well-formed Units are admitted; structural
        // strictness is the admission callsite's job (see module header).
        let parsed = parse_answer_key(&key_text);
        let manifest = self.read_manifest()?;
        let digest =
            digest_of(&key_bytes).map_err(|err| ReferenceProviderError::ResolveFailed {
                detail: format!("normalizing the answer key failed: {err}"),
            })?;
        let definition = build_definition(&manifest, parsed.units, digest);
        Ok(ReferenceDraft::new(self.sources(), definition))
    }

    fn normalize(
        &self,
        draft: ReferenceDraft,
        _deadline: Deadline,
    ) -> Result<Reference, ReferenceProviderError> {
        // Domain invariants only (see module header decision). Structural
        // key validation happens where structure is admitted.
        Reference::new(draft.into_definition())
            .map_err(|cause| ReferenceProviderError::NormalizeFailed { cause })
    }

    fn digest(
        &self,
        reference: &Reference,
        _deadline: Deadline,
    ) -> Result<Digest, ReferenceProviderError> {
        // Identity, not re-derivation: `normalize` computed the digest over
        // the normalized answer-key bytes and carried it into the
        // `ReferenceDefinition`; the digest is the reference's own.
        Ok(reference.digest().clone())
    }

    fn freeze(
        &self,
        run_id: &RunId,
        reference: &Reference,
        deadline: Deadline,
    ) -> Result<ReferenceVersion, ReferenceProviderError> {
        // Freeze writes the NORMALIZED answer-key bytes into the evidence
        // store, pins them to the run, and returns the reference's one
        // version. Content addressing makes a repeat freeze for the same run
        // a no-op on the object (same bytes → same ref); the pin is
        // idempotent by construction.
        let key_bytes =
            self.answer_key
                .read()
                .map_err(|detail| ReferenceProviderError::FreezeFailed {
                    detail: format!("re-reading the answer key for freeze failed: {detail}"),
                })?;
        let normalized =
            normalize_bytes(&key_bytes).map_err(|err| ReferenceProviderError::FreezeFailed {
                detail: format!("normalizing the answer key for freeze failed: {err}"),
            })?;
        // Guard against a mutation landing between `normalize` and `freeze`
        // (d4-verify F4): digest the bytes freeze actually read and compare
        // with the digest the reference was normalized under. A mismatch
        // fires the same I1 tripwire `verify_unchanged` fires — the stored
        // evidence and the returned version can never silently disagree.
        // Same variant, no port change: the expected/actual digests are the
        // evidence either way.
        let actual = digest_normalized(&normalized);
        if actual != *reference.digest() {
            return Err(ReferenceProviderError::ReferenceMutated {
                expected: reference.digest().clone(),
                actual,
            });
        }
        let evidence_ref = self
            .evidence
            .put(&normalized, EvidenceKind::File, deadline)
            .map_err(|err| ReferenceProviderError::FreezeFailed {
                detail: format!("storing the frozen key failed: {err}"),
            })?;
        self.evidence
            .pin(run_id, &evidence_ref, deadline)
            .map_err(|err| ReferenceProviderError::FreezeFailed {
                detail: format!("pinning the frozen key to the run failed: {err}"),
            })?;
        Ok(ReferenceVersion::with_evidence(
            reference.id().clone(),
            reference.version(),
            reference.digest().clone(),
            evidence_ref,
        ))
    }

    fn verify_unchanged(
        &self,
        version: &ReferenceVersion,
        _deadline: Deadline,
    ) -> Result<(), ReferenceProviderError> {
        // Broken, missing, or invalid-UTF-8 artifact is a typed
        // Unavailable-class failure, NOT `ReferenceMutated`: a broken
        // artifact is not proof of mutation, and the two must not be
        // conflated (spec §6.2). Only a well-formed current digest that
        // differs from the frozen one fires the I1 tripwire.
        let bytes = match self.answer_key.read() {
            Ok(bytes) => bytes,
            Err(detail) => {
                return Err(ReferenceProviderError::Unavailable {
                    detail: format!("re-reading the answer key failed: {detail}"),
                });
            }
        };
        let actual = match digest_of(&bytes) {
            Ok(digest) => digest,
            Err(err) => {
                return Err(ReferenceProviderError::Unavailable {
                    detail: format!("the current answer key does not digest: {err}"),
                });
            }
        };
        if actual == *version.digest() {
            Ok(())
        } else {
            Err(ReferenceProviderError::ReferenceMutated {
                expected: version.digest().clone(),
                actual,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use gauntlet_domain::{EvidenceKind, EvidenceRef, RunId};
    use gauntlet_ports::contract::reference_provider::ReferenceProviderFactory;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::reference_provider::ReferenceProviderError;
    use sha2::{Digest as Sha2Digest, Sha256};

    use gauntlet_ports::contract::reference_provider::reference_provider_suite;
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::reference_provider::ReferenceProvider;

    use super::{AnswerKeySource, FileReferenceProvider, ManifestSource};

    /// A deadline fixed far from any clock (the pipeline under test never
    /// reads one). A named const because the core-crate literals scanner
    /// refuses a bare clock number on any line that also names the bound.
    const BOUND_AT: u64 = 1_700_000_060;

    fn bound() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(BOUND_AT))
    }
    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID")
    }

    /// Unique scratch paths, pid + atomic counter — the repository's Scratch
    /// idiom (see the fs evidence store's tests), replacing the earlier
    /// pid + nanos naming (d4-verify F5).
    static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

    fn scratch_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gauntlet-ref-provider-{label}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// A minimal manifest in the frozen shape, for the contract suite and
    /// the focused tests. Copy of `manifest::tests::minimal`'s shape (the
    /// manifest parser's own unit-test fixture), so the provider's inline
    /// manifest always parses.
    fn minimal_manifest() -> String {
        [
            "reference:",
            "  id: ref-suite-01",
            "  version: 1",
            "  purpose: >",
            "    One paragraph.",
            "  desired_state: >",
            "    The destination.",
            "  settlement:",
            "    success_criteria:",
            "      - the destination is reached",
            "    minimum_reliability: >",
            "      spec 19.2 default: >= 1 settlements before promotion.",
            "    terminal_conditions: [settled, aborted]",
            "",
        ]
        .join("\n")
    }

    /// A minimal in-memory `EvidenceStore` for the tests: content-addressed
    /// by the bytes' sha256, write-once (a different byte offering for a
    /// present digest is a typed refusal), pins tracked. The freeze path
    /// only needs `put` + `pin`; `get` returns from memory for the
    /// round-trip assertions. `RefCell` for interior mutability: the port
    /// takes `&self` (write-once is an API-shape invariant, not a `&mut`
    /// signature — the same convention as the fs store).
    #[derive(Default)]
    struct MemoryStore {
        objects: RefCell<HashMap<String, Vec<u8>>>,
        pins: RefCell<HashMap<(String, String), Vec<u8>>>,
    }

    impl MemoryStore {
        fn hex(bytes: &[u8]) -> String {
            let hashed = Sha256::digest(bytes);
            hashed.iter().map(|byte| format!("{byte:02x}")).collect()
        }

        fn fresh() -> Rc<dyn EvidenceStore> {
            Rc::new(MemoryStore::default())
        }
    }

    impl EvidenceStore for MemoryStore {
        fn put(
            &self,
            bytes: &[u8],
            _kind: EvidenceKind,
            _deadline: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            let hex = Self::hex(bytes);
            let mut objects = self.objects.borrow_mut();
            if let Some(existing) = objects.get(&hex) {
                if existing.as_slice() != bytes {
                    return Err(EvidenceStoreError::WriteFailed {
                        detail: format!(
                            "object {hex} already holds {} bytes, offered {} — content-addressed \
                             paths are never rewritten",
                            existing.len(),
                            bytes.len()
                        ),
                    });
                }
                return EvidenceRef::new(&format!("sha256:{hex}")).map_err(|err| {
                    EvidenceStoreError::WriteFailed {
                        detail: format!("the digest is not a valid ref: {err}"),
                    }
                });
            }
            objects.insert(hex.clone(), bytes.to_vec());
            EvidenceRef::new(&format!("sha256:{hex}")).map_err(|err| {
                EvidenceStoreError::WriteFailed {
                    detail: format!("the digest is not a valid ref: {err}"),
                }
            })
        }

        fn get(
            &self,
            reference: &EvidenceRef,
            _deadline: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
            let hex = reference
                .as_str()
                .strip_prefix("sha256:")
                .map(str::to_owned);
            Ok(hex.and_then(|h| self.objects.borrow().get(&h).cloned()))
        }

        fn pin(
            &self,
            run_id: &RunId,
            reference: &EvidenceRef,
            _deadline: Deadline,
        ) -> Result<(), EvidenceStoreError> {
            let key = (run_id.to_string(), reference.as_str().to_owned());
            self.pins.borrow_mut().insert(key, Vec::new());
            Ok(())
        }
    }

    /// The shared contract suite, run against the production provider: the
    /// factory builds a provider over a shared answer-key artifact and an
    /// inline manifest, so the suite can rewrite the standard mid-run and
    /// trip the I1 wire. Must produce zero findings.
    struct SuiteFactory;

    impl ReferenceProviderFactory for SuiteFactory {
        fn label(&self) -> &'static str {
            "file-reference-provider"
        }
        fn build(
            &self,
            artifact: Rc<RefCell<String>>,
        ) -> Result<Box<dyn ReferenceProvider>, String> {
            Ok(Box::new(FileReferenceProvider::from_sources(
                AnswerKeySource::Shared(artifact),
                ManifestSource::Inline(minimal_manifest()),
                MemoryStore::fresh(),
            )))
        }
    }

    #[test]
    fn the_production_provider_passes_the_shared_contract_suite() {
        let findings = reference_provider_suite(&SuiteFactory);
        assert!(
            findings.is_empty(),
            "the production ReferenceProvider must pass the contract suite: {findings:#?}"
        );
    }

    #[test]
    fn significance_of_the_digest_and_refusal_semantics() {
        let key = scratch_path("key");
        std::fs::write(&key, b"the standard text").expect("writes the key");
        let provider = FileReferenceProvider::from_sources(
            AnswerKeySource::Path(key.clone()),
            ManifestSource::Inline(minimal_manifest()),
            MemoryStore::fresh(),
        );
        let draft = provider.resolve("intent", bound()).expect("resolves");
        let reference = provider.normalize(draft, bound()).expect("normalizes");
        let version = provider
            .freeze(&run_id(), &reference, bound())
            .expect("freezes");
        // verify_unchanged right after freeze is Ok.
        provider
            .verify_unchanged(&version, bound())
            .expect("unmoved");
        // Rewriting the artifact trips the I1 wire with both digests.
        std::fs::write(&key, b"the operator edited the standard").expect("rewrites");
        match provider.verify_unchanged(&version, bound()) {
            Err(ReferenceProviderError::ReferenceMutated { expected, actual }) => {
                assert_eq!(expected, *version.digest());
                assert_ne!(actual, *version.digest());
            }
            other => panic!("a moved artifact must fire the tripwire, got {other:?}"),
        }
        // The digest is the reference's own (identity, not re-derivation).
        assert_eq!(
            provider.digest(&reference, bound()).expect("digests"),
            *reference.digest()
        );
        std::fs::remove_file(&key).ok();
    }

    /// d4-verify F3: the `verify_unchanged` Unavailable arms. A broken
    /// artifact is NOT proof of mutation: removed after freeze, and made
    /// non-UTF-8 after freeze, both report `Unavailable`, never
    /// `ReferenceMutated` (spec §6.2 — the two states must not be conflated).
    #[test]
    fn a_broken_artifact_after_freeze_is_unavailable_not_reference_mutated() {
        let key = scratch_path("broken");
        std::fs::write(&key, b"the standard text").expect("writes the key");
        let provider = FileReferenceProvider::from_sources(
            AnswerKeySource::Path(key.clone()),
            ManifestSource::Inline(minimal_manifest()),
            MemoryStore::fresh(),
        );
        let draft = provider.resolve("intent", bound()).expect("resolves");
        let reference = provider.normalize(draft, bound()).expect("normalizes");
        let version = provider
            .freeze(&run_id(), &reference, bound())
            .expect("freezes");
        // Artifact removed after freeze: Unavailable, not a mutation verdict.
        std::fs::remove_file(&key).expect("removes the key");
        match provider.verify_unchanged(&version, bound()) {
            Err(ReferenceProviderError::Unavailable { .. }) => {}
            other => panic!(
                "a missing artifact must be Unavailable, never ReferenceMutated, got {other:?}"
            ),
        }
        // Artifact made non-UTF-8 after freeze: Unavailable, naming the
        // class, still never a mutation verdict.
        std::fs::write(&key, b"the standard \xff text").expect("writes invalid UTF-8");
        match provider.verify_unchanged(&version, bound()) {
            Err(ReferenceProviderError::Unavailable { detail }) => {
                assert!(
                    detail.contains("UTF-8"),
                    "the refusal names the invalid-UTF-8 class: {detail}"
                );
            }
            other => panic!(
                "a non-UTF-8 artifact must be Unavailable, never ReferenceMutated, got {other:?}"
            ),
        }
        std::fs::remove_file(&key).ok();
    }

    /// d4-verify F4's guard: a mutation landing between `normalize` and
    /// `freeze` cannot store evidence whose digest differs from the returned
    /// version — freeze digests the bytes it actually read and fires the I1
    /// tripwire with both digests instead.
    #[test]
    fn a_mutation_between_normalize_and_freeze_fires_the_tripwire_at_freeze() {
        let shared = Rc::new(RefCell::new("the standard text".to_owned()));
        let provider = FileReferenceProvider::from_sources(
            AnswerKeySource::Shared(shared.clone()),
            ManifestSource::Inline(minimal_manifest()),
            MemoryStore::fresh(),
        );
        let draft = provider.resolve("intent", bound()).expect("resolves");
        let reference = provider.normalize(draft, bound()).expect("normalizes");
        // The artifact moves after normalize, before freeze.
        *shared.borrow_mut() = "a mid-flight edit of the standard".to_owned();
        let rederived = super::super::normalize::digest_of(b"a mid-flight edit of the standard")
            .expect("the moved artifact digests");
        match provider.freeze(&run_id(), &reference, bound()) {
            Err(ReferenceProviderError::ReferenceMutated { expected, actual }) => {
                assert_eq!(expected, *reference.digest());
                assert_eq!(actual, rederived);
            }
            other => panic!("freeze must refuse a moved artifact, got {other:?}"),
        }
    }

    #[test]
    fn freeze_is_content_addressed_and_idempotent_for_the_same_run() {
        let store = MemoryStore::fresh();
        let provider = FileReferenceProvider::from_sources(
            AnswerKeySource::Shared(Rc::new(RefCell::new("the standard text".to_owned()))),
            ManifestSource::Inline(minimal_manifest()),
            store.clone(),
        );
        let draft = provider.resolve("intent", bound()).expect("resolves");
        let reference = provider.normalize(draft, bound()).expect("normalizes");
        let first = provider
            .freeze(&run_id(), &reference, bound())
            .expect("freezes");
        let second = provider
            .freeze(&run_id(), &reference, bound())
            .expect("freezes again");
        assert_eq!(first, second, "same run, same reference → same version");
        // Content addressing: the stored object is the normalized key bytes.
        let key_digest = gauntlet_app_normalize("the standard text");
        let ref_str = format!("sha256:{key_digest}");
        let stored = store
            .get(&EvidenceRef::new(&ref_str).expect("valid ref"), bound())
            .expect("get works");
        assert_eq!(
            stored,
            Some(b"the standard text\n".to_vec()),
            "freeze stores the NORMALIZED key bytes"
        );
    }

    fn gauntlet_app_normalize(text: &str) -> String {
        let hashed = Sha256::digest(super::super::normalize::normalize(text).as_bytes());
        hashed.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn a_missing_artifact_is_unavailable_not_reference_mutated() {
        let missing = scratch_path("missing");
        let provider = FileReferenceProvider::from_sources(
            AnswerKeySource::Path(missing),
            ManifestSource::Inline(minimal_manifest()),
            MemoryStore::fresh(),
        );
        // resolve: missing answer key is a ResolveFailed-class error.
        assert!(matches!(
            provider.resolve("intent", bound()),
            Err(ReferenceProviderError::ResolveFailed { .. })
        ));
    }
}
