//! The content-addressed filesystem store behind [`crate`].
//!
//! Port methods carry a `Deadline` (spec §6 preamble); this adapter performs
//! single filesystem operations with no retry loop and holds no clock of its
//! own (the `Clock` port is the runtime's only "now"), so the bound is noted
//! and not consulted — there is no waiting code here to bound. An adapter
//! that adds retries must consult it.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gauntlet_domain::{EvidenceKind, EvidenceRef, RunId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
use sha2::{Digest as _, Sha256};

/// The digest algorithm this store addresses content by. The ref is
/// `<algo>:<64 lowercase hex>` — the domain `Digest` shape, and the
/// `EvidenceRef` grammar accepts it (non-empty, whitespace-free, relative).
const ALGO: &str = "sha256";

/// The prefix every ref this store assigns carries.
const REF_PREFIX: &str = "sha256:";

/// How many hex characters a SHA-256 digest carries.
const HEX_LEN: usize = 64;

/// Distinguishes unique temporary writers within one process.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A write-once, content-addressed filesystem evidence store.
#[derive(Debug, Clone)]
pub struct FsEvidenceStore {
    root: PathBuf,
}

impl FsEvidenceStore {
    /// Opens (and creates, if needed) a store rooted at `root`.
    pub fn open(root: &Path) -> Result<Self, EvidenceStoreError> {
        let store = Self {
            root: root.to_path_buf(),
        };
        for dir in [store.objects_root(), store.pins_root()] {
            create_dir_all(&dir)?;
        }
        Ok(store)
    }

    fn objects_root(&self) -> PathBuf {
        self.root.join("objects")
    }

    fn pins_root(&self) -> PathBuf {
        self.root.join("pins")
    }

    /// `objects/<b0-2>/<b2-4>/<hex>` — a two-level fanout keeps any one
    /// directory small; the file name is the full digest so the path is
    /// recoverable from the ref alone.
    fn object_path(&self, hex: &str) -> PathBuf {
        self.objects_root()
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex)
    }

    /// The store path for a ref's pin marker.
    fn pin_path(&self, run_id: &RunId, reference: &EvidenceRef) -> PathBuf {
        self.pins_root()
            .join(run_id.to_string())
            .join(reference.to_string())
    }

    /// Writes `bytes` at the path `hex` names, write-once: an existing object
    /// is compared byte-for-byte — equal is the content-addressed no-op, and
    /// ANY byte mismatch is a typed error, never an overwrite. The bytes are
    /// fsynced under a temporary name and then renamed into place, so a
    /// crash leaves either the old object or the new one, never a torn one.
    fn write_bytes_at_digest(&self, hex: &str, bytes: &[u8]) -> Result<(), EvidenceStoreError> {
        let target = self.object_path(hex);
        if target.exists() {
            return verify_untouched(&target, hex, bytes);
        }
        let parent = target
            .parent()
            .ok_or_else(|| write_failed(hex, "the object path has no parent directory"))?;
        create_dir_all(parent)?;
        let temp = parent.join(format!(
            ".incoming-{}-{}",
            std::process::id(),
            TMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        write_durable(&temp, bytes)?;
        match std::fs::rename(&temp, &target) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                // Another writer won the race; the object exists now.
                let _ = std::fs::remove_file(&temp);
                return verify_untouched(&target, hex, bytes);
            }
            Err(err) => {
                let _ = std::fs::remove_file(&temp);
                return Err(write_failed(
                    hex,
                    &format!("renaming the written object into place failed: {err}"),
                ));
            }
        }
        sync_directory(parent)
    }

    /// Records the kind beside the object, write-once: the first writer's
    /// kind stands, and a later writer asserting a DIFFERENT kind is a
    /// typed refusal — a contradicting provenance assertion is loud, never
    /// silently discarded. Kind is provenance, not identity — the ref is a
    /// function of the bytes alone.
    fn record_kind(&self, hex: &str, kind: EvidenceKind) -> Result<(), EvidenceStoreError> {
        let sidecar = self.object_path(hex).with_extension("kind");
        if sidecar.exists() {
            return verify_kind_sidecar(&sidecar, hex, kind);
        }
        let parent = sidecar
            .parent()
            .ok_or_else(|| write_failed(hex, "the kind sidecar path has no parent directory"))?;
        // pid/seq-unique temp name, like the objects: two writers in one
        // process never share a marker.
        let marker = parent.join(format!(
            ".incoming-kind-{}-{}",
            std::process::id(),
            TMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        write_durable(&marker, kind.wire().as_bytes())?;
        match std::fs::rename(&marker, &sidecar) {
            Ok(()) => sync_directory(parent),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                // Another writer won the race; what it published must agree.
                let _ = std::fs::remove_file(&marker);
                verify_kind_sidecar(&sidecar, hex, kind)
            }
            Err(err) => {
                let _ = std::fs::remove_file(&marker);
                Err(write_failed(
                    hex,
                    &format!("renaming the kind sidecar into place failed: {err}"),
                ))
            }
        }
    }

    /// The hex payload of a ref this store could have assigned; `None` for
    /// any other shape, which is the same as "never put here".
    fn hex_of(&self, reference: &EvidenceRef) -> Option<String> {
        let hex = reference.as_str().strip_prefix(REF_PREFIX)?;
        let shaped = hex.len() == HEX_LEN
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        shaped.then(|| hex.to_owned())
    }
}

impl EvidenceStore for FsEvidenceStore {
    fn put(
        &self,
        bytes: &[u8],
        kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        let hashed = Sha256::digest(bytes);
        let hex: String = hashed.iter().map(|byte| format!("{byte:02x}")).collect();
        self.write_bytes_at_digest(&hex, bytes)?;
        self.record_kind(&hex, kind)?;
        EvidenceRef::new(&format!("{ALGO}:{hex}")).map_err(|err| {
            write_failed(
                &hex,
                &format!("the content-derived ref is not a valid EvidenceRef: {err}"),
            )
        })
    }

    fn get(
        &self,
        reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        let Some(hex) = self.hex_of(reference) else {
            return Ok(None);
        };
        let path = self.object_path(&hex);
        // The object must be a regular file this store wrote: a symlink, a
        // directory, or anything else at an object path is aliasing, never
        // evidence. `symlink_metadata` does not follow, so a link is seen
        // as a link.
        match std::fs::symlink_metadata(&path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(EvidenceStoreError::ReadFailed {
                    detail: format!("inspecting evidence {ALGO}:{hex} failed: {err}"),
                });
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(EvidenceStoreError::ReadFailed {
                    detail: format!(
                        "evidence {ALGO}:{hex} exists but is {} — an object must be a regular \
                         file this store wrote, never an alias",
                        describe_path_kind(&metadata)
                    ),
                });
            }
            Ok(_) => {}
        }
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(EvidenceStoreError::ReadFailed {
                    detail: format!("reading evidence {ALGO}:{hex} failed: {err}"),
                });
            }
        };
        // Content addressing is enforced at READ time too: the bytes are
        // re-hashed and compared to the address, so an out-of-band swap
        // (tamper, bit rot) is a typed integrity error rather than content
        // silently served under an address it is not. Cost: one SHA-256
        // pass per read — evidence reads are not a hot path.
        let hashed = Sha256::digest(&bytes);
        let actual_hex: String = hashed.iter().map(|byte| format!("{byte:02x}")).collect();
        if actual_hex != hex {
            return Err(EvidenceStoreError::ReadFailed {
                detail: format!(
                    "content at {ALGO}:{hex} does not hash to its address — tampered or \
                     corrupted (it hashes to {ALGO}:{actual_hex})"
                ),
            });
        }
        Ok(Some(bytes))
    }

    fn pin(
        &self,
        run_id: &RunId,
        reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<(), EvidenceStoreError> {
        let Some(hex) = self.hex_of(reference) else {
            return Err(EvidenceStoreError::NotFound {
                reference: reference.clone(),
            });
        };
        if !self.object_path(&hex).exists() {
            return Err(EvidenceStoreError::NotFound {
                reference: reference.clone(),
            });
        }
        let marker = self.pin_path(run_id, reference);
        let parent = marker
            .parent()
            .ok_or_else(|| pin_failed(reference, "the pin path has no parent directory"))?;
        create_dir_all(parent)?;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
        {
            Ok(file) => {
                if let Err(err) = file.sync_all() {
                    return Err(pin_failed(
                        reference,
                        &format!("fsyncing the pin failed: {err}"),
                    ));
                }
                sync_directory(parent)
                    .map_err(|_| pin_failed(reference, "fsyncing the pin directory failed"))
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(err) => Err(pin_failed(
                reference,
                &format!("creating the pin failed: {err}"),
            )),
        }
    }
}

/// An existing object must hold exactly the offered bytes; anything else is a
/// collision on a content-addressed path — reported, never overwritten. The
/// object must also be a regular file: an existing symlink or directory at
/// an object path is aliasing, never evidence.
fn verify_untouched(target: &Path, hex: &str, bytes: &[u8]) -> Result<(), EvidenceStoreError> {
    match std::fs::symlink_metadata(target) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(EvidenceStoreError::WriteFailed {
                detail: format!(
                    "{ALGO}:{hex}: the object path exists but is {} — an object must be a \
                     regular file this store wrote, never an alias",
                    describe_path_kind(&metadata)
                ),
            });
        }
        Ok(_) => {}
        Err(err) => {
            return Err(write_failed(
                hex,
                &format!("inspecting the existing object failed: {err}"),
            ));
        }
    }
    match std::fs::read(target) {
        Ok(existing) if existing.as_slice() == bytes => Ok(()),
        Ok(existing) => Err(EvidenceStoreError::WriteFailed {
            detail: format!(
                "object {ALGO}:{hex} already holds {} bytes but {} were offered — a \
                 content-addressed path is never rewritten",
                existing.len(),
                bytes.len()
            ),
        }),
        Err(err) => Err(write_failed(
            hex,
            &format!("re-reading the object failed: {err}"),
        )),
    }
}

/// An existing kind sidecar must agree with the asserted kind: the same
/// kind is the idempotent no-op; a DIFFERENT kind is a typed refusal naming
/// both — a contradicting provenance assertion is loud, never silently
/// discarded.
fn verify_kind_sidecar(
    sidecar: &Path,
    hex: &str,
    kind: EvidenceKind,
) -> Result<(), EvidenceStoreError> {
    match std::fs::read(sidecar) {
        Ok(existing) if existing == kind.wire().as_bytes() => Ok(()),
        Ok(existing) => Err(EvidenceStoreError::WriteFailed {
            detail: format!(
                "{ALGO}:{hex}: the kind sidecar already records kind {:?} but {:?} was asserted \
                 — a contradicting provenance assertion is never silently discarded",
                String::from_utf8_lossy(&existing),
                kind.wire()
            ),
        }),
        Err(err) => Err(write_failed(
            hex,
            &format!("reading the existing kind sidecar failed: {err}"),
        )),
    }
}

/// The filesystem kind of an existing path, for refusals that name it.
fn describe_path_kind(metadata: &std::fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        "a symbolic link"
    } else if file_type.is_dir() {
        "a directory"
    } else {
        "not a regular file"
    }
}

fn write_failed(hex: &str, detail: &str) -> EvidenceStoreError {
    EvidenceStoreError::WriteFailed {
        detail: format!("{ALGO}:{hex}: {detail}"),
    }
}

fn pin_failed(reference: &EvidenceRef, detail: &str) -> EvidenceStoreError {
    EvidenceStoreError::PinFailed {
        reference: reference.clone(),
        detail: detail.to_owned(),
    }
}

fn create_dir_all(path: &Path) -> Result<(), EvidenceStoreError> {
    std::fs::create_dir_all(path).map_err(|err| EvidenceStoreError::WriteFailed {
        detail: format!("creating directory {:?} failed: {err}", path.display()),
    })
}

/// Writes `bytes` to `path` and fsyncs the file before returning, so the
/// rename that follows publishes durable content.
fn write_durable(path: &Path, bytes: &[u8]) -> Result<(), EvidenceStoreError> {
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|err| EvidenceStoreError::WriteFailed {
            detail: format!("opening {:?} for writing failed: {err}", path.display()),
        })?;
    file.write_all(bytes)
        .map_err(|err| EvidenceStoreError::WriteFailed {
            detail: format!("writing {:?} failed: {err}", path.display()),
        })?;
    file.sync_all()
        .map_err(|err| EvidenceStoreError::WriteFailed {
            detail: format!("fsyncing {:?} failed: {err}", path.display()),
        })
}

/// Renames are durable only once the directory entry is on disk; syncing the
/// parent directory is what makes the write-once object survive a crash.
fn sync_directory(dir: &Path) -> Result<(), EvidenceStoreError> {
    std::fs::File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(|err| EvidenceStoreError::WriteFailed {
            detail: format!("fsyncing directory {:?} failed: {err}", dir.display()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    /// Isolated scratch directory per test; removed on drop. `tempfile` is
    /// not on any allowlist, and a test helper does not justify an entry.
    struct Scratch {
        path: PathBuf,
    }

    static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "gauntlet-evidence-fs-{label}-{}-{}",
                std::process::id(),
                SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::remove_dir_all(&path).ok();
            std::fs::create_dir_all(&path).expect("scratch directory is creatable");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path).ok();
        }
    }

    fn bound() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(1_700_000_060))
    }

    /// A file path BESIDE the scratch store — for symlink tests that need a
    /// target outside the store root. The caller removes it when done.
    fn scratch_sibling(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gauntlet-evidence-fs-{label}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// A digest the store could have assigned (the right shape), used to
    /// drive the internal write path at a chosen address.
    fn fixed_hex(seed: u8) -> String {
        let hashed = Sha256::digest([seed]);
        hashed.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Tooth (b), permanent half: a colliding digest with different bytes is
    /// a typed refusal, and the stored object is untouched. The address is
    /// a REAL one (the digest of the stored bytes, assigned by `put`) —
    /// since get() began re-verifying content against its address, bytes
    /// written at an address that is not their hash are refused on read, so
    /// a hand-chosen address can no longer round-trip.
    #[test]
    fn a_colliding_digest_with_different_bytes_is_a_typed_refusal() {
        let scratch = Scratch::new("collision");
        let store = FsEvidenceStore::open(&scratch.path).expect("store opens");
        let reference = store
            .put(b"original bytes", EvidenceKind::Log, bound())
            .expect("the original lands at its own address");
        let hex = store
            .hex_of(&reference)
            .expect("the ref is this store's shape");
        let error = store
            .write_bytes_at_digest(&hex, b"different bytes entirely")
            .expect_err("a collision must refuse");
        assert!(
            matches!(error, EvidenceStoreError::WriteFailed { ref detail }
                if detail.contains(&format!("{ALGO}:{hex}"))),
            "the refusal must name the digest, got: {error}"
        );
        // The original bytes are what the address resolves to afterwards.
        assert_eq!(
            store.get(&reference, bound()).expect("get works"),
            Some(b"original bytes".to_vec())
        );
    }

    /// Tooth (b), structural half: no public API method names an overwrite or
    /// removal — the API shape IS the immutability invariant (spec §6.2).
    /// Reads this crate's own sources, exactly like the domain's
    /// `no_public_surface_can_supply_a_reference_version_after_construction`.
    #[test]
    fn no_public_api_exposes_an_overwrite_or_delete_path() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for source in ["src/lib.rs", "src/store.rs"] {
            let text =
                std::fs::read_to_string(manifest_dir.join(source)).expect("source is readable");
            let production = text
                .split("#[cfg(test)]")
                .next()
                .expect("split always yields a first element")
                .to_owned();
            for line in production.lines() {
                if !line.contains("pub fn") && !line.contains("pub unsafe fn") {
                    continue;
                }
                let lowered = line.to_lowercase();
                assert!(
                    !lowered.contains("delete")
                        && !lowered.contains("remove")
                        && !lowered.contains("overwrite")
                        && !lowered.contains("update")
                        && !lowered.contains("unlink")
                        && !lowered.contains("truncate_all"),
                    "a public method names a destructive path: {line}"
                );
            }
        }
    }

    #[test]
    fn refs_outside_this_stores_grammar_resolve_to_none() {
        let scratch = Scratch::new("grammar");
        let store = FsEvidenceStore::open(&scratch.path).expect("store opens");
        let foreign = EvidenceRef::new("evd-000042").expect("a foreign ref shape");
        assert_eq!(store.get(&foreign, bound()).expect("get works"), None);
    }

    /// A tampered object is refused on `get`: the store re-hashes the bytes
    /// it read and compares them to the address — content addressing is
    /// enforced at READ time too, not only at write time, so an out-of-band
    /// swap (bit rot, tamper) is a typed integrity error, never silently
    /// served as if it were the preimage.
    #[test]
    fn a_tampered_object_is_a_typed_integrity_error_on_get() {
        let scratch = Scratch::new("tampered-get");
        let store = FsEvidenceStore::open(&scratch.path).expect("store opens");
        let reference = store
            .put(b"the original evidence bytes", EvidenceKind::Log, bound())
            .expect("the original lands");
        let hex = store
            .hex_of(&reference)
            .expect("the ref is this store's shape");
        // Out-of-band swap: different bytes at the same content address.
        std::fs::write(store.object_path(&hex), b"tampered bytes").expect("the swap lands");
        match store.get(&reference, bound()) {
            Err(EvidenceStoreError::ReadFailed { detail }) => {
                assert!(
                    detail.contains("does not hash to its address"),
                    "the error names the integrity breach: {detail}"
                );
                assert!(
                    detail.contains(&format!("{ALGO}:{hex}")),
                    "the error names the address: {detail}"
                );
            }
            other => panic!("tampered content must be a typed error, got {other:?}"),
        }
    }

    /// An object path that exists but is not a regular file — a symlink —
    /// is refused on BOTH `put` (the write-verify entry) and `get`, with a
    /// typed error naming the path kind. The symlink deliberately points at
    /// a file holding the SAME bytes: without the check, both paths would
    /// follow the link and serve bytes the store does not own.
    #[cfg(unix)]
    #[test]
    fn a_symlink_at_an_object_path_is_refused_by_put_and_get() {
        let outside = scratch_sibling("symlink-outside");
        std::fs::write(&outside, b"the aliased bytes").expect("the target file writes");
        let scratch = Scratch::new("symlink-object");
        let store = FsEvidenceStore::open(&scratch.path).expect("store opens");
        let reference = store
            .put(b"the aliased bytes", EvidenceKind::Log, bound())
            .expect("the original lands");
        let hex = store
            .hex_of(&reference)
            .expect("the ref is this store's shape");
        let target = store.object_path(&hex);
        std::fs::remove_file(&target).expect("the object file is removed for the swap");
        std::os::unix::fs::symlink(&outside, &target).expect("the symlink lands");
        match store.put(b"the aliased bytes", EvidenceKind::Log, bound()) {
            Err(EvidenceStoreError::WriteFailed { detail }) => {
                assert!(
                    detail.contains("symbolic link"),
                    "the refusal names the path kind: {detail}"
                );
            }
            other => panic!("a symlinked object must be refused on put, got {other:?}"),
        }
        match store.get(&reference, bound()) {
            Err(EvidenceStoreError::ReadFailed { detail }) => {
                assert!(
                    detail.contains("symbolic link"),
                    "the refusal names the path kind: {detail}"
                );
            }
            other => panic!("a symlinked object must be refused on get, got {other:?}"),
        }
        std::fs::remove_file(&outside).ok();
    }

    /// A directory at an object path is refused by `get` with the path kind
    /// named — not a bare EISDIR.
    #[test]
    fn a_directory_at_an_object_path_is_refused_with_its_kind_named() {
        let scratch = Scratch::new("directory-object");
        let store = FsEvidenceStore::open(&scratch.path).expect("store opens");
        let hex = fixed_hex(7);
        let target = store.object_path(&hex);
        std::fs::create_dir_all(&target).expect("the directory lands");
        let reference = EvidenceRef::new(&format!("{ALGO}:{hex}")).expect("the ref shape is valid");
        match store.get(&reference, bound()) {
            Err(EvidenceStoreError::ReadFailed { detail }) => {
                assert!(
                    detail.contains("directory"),
                    "the refusal names the path kind: {detail}"
                );
            }
            other => panic!("a directory object must be refused on get, got {other:?}"),
        }
    }

    /// A kind sidecar that already records a DIFFERENT kind is a typed
    /// refusal naming both: a contradicting provenance assertion is loud,
    /// never silently discarded. Same kind again is the no-op it always was.
    #[test]
    fn a_contradicting_kind_sidecar_is_a_typed_refusal() {
        let scratch = Scratch::new("kind-conflict");
        let store = FsEvidenceStore::open(&scratch.path).expect("store opens");
        store
            .put(b"shared bytes", EvidenceKind::Log, bound())
            .expect("the first kind stands");
        // Same bytes, same ref — but a different provenance assertion.
        match store.put(b"shared bytes", EvidenceKind::Diff, bound()) {
            Err(EvidenceStoreError::WriteFailed { detail }) => {
                assert!(
                    detail.contains("log") && detail.contains("diff"),
                    "the refusal names both kinds: {detail}"
                );
                assert!(
                    detail.contains("kind"),
                    "the refusal names what disagrees: {detail}"
                );
            }
            other => panic!("a contradicting kind must be a typed refusal, got {other:?}"),
        }
        // Re-asserting the SAME kind is the idempotent no-op.
        store
            .put(b"shared bytes", EvidenceKind::Log, bound())
            .expect("the same kind again is a no-op");
    }
}
