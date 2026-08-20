//! Filesystem observer adapter (spec §4.1, §12; plan Task 9): the
//! filesystem facet — a bounded, deterministic manifest walk of the
//! observed workspace, recorded as instrument-produced evidence with full
//! provenance. Builder claims never enter an observation bundle through
//! this path.
//!
//! The wrapper IS the instrument (plan Task 9 redesign trigger): the facet
//! names `gauntlet-observe-filesystem`, not the filesystem itself, and the
//! exact walk contract (root key, caps, symlink posture) is the recorded
//! command string. Raw output is ONE `facet_evidence@v1` document
//! (`gauntlet_ports::observer_evidence`): the manifest walk renders as
//! JSONL lines in the document's `stdout`, one object per entry, paths
//! relative with forward slashes.
//!
//! Exit-status semantics (P3 core): a walk that COMPLETES is exit `0` —
//! including a walk whose manifest records unreadable entries, because the
//! observation is of reality and reality includes the error. Only an
//! instrument that cannot run at all (the workspace root unreadable, the
//! evidence store refusing the write) is an [`ObserverError`].
//!
//! Bounded by named caps (Task-7 runaway-memory lesson): walk depth,
//! manifest entries, and per-file hashed bytes. Every cap is stated in the
//! recorded command string, so the evidence document is self-describing
//! about what was NOT walked.

use std::collections::VecDeque;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_domain::{EvidenceKind, InstrumentName, WorkspaceRef};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::json::{JsonNumber, JsonValue};
use gauntlet_ports::observer::{
    FacetKind, InstrumentIdentity, ObservationFacet, ObservationRequest, Observer, ObserverError,
};
use gauntlet_ports::observer_evidence::{FacetEvidenceDoc, ObservedRun};
use sha2::{Digest as _, Sha256};

/// The wrapper instrument's name — the capability is `filesystem`
/// (`FacetKind::Filesystem`); this names the adapter that observed.
pub const INSTRUMENT_NAME: &str = "gauntlet-observe-filesystem";

/// Maximum directory depth the walk descends (the workspace's own entries
/// are depth 1). A deeper directory is LISTED with a reason and not
/// descended.
pub const MAX_WALK_DEPTH: usize = 64;

/// Maximum entries the manifest records. A larger workspace is walked up to
/// the cap and the manifest ends with an explicit truncation line — never
/// unbounded growth from filesystem content.
pub const MAX_MANIFEST_ENTRIES: usize = 10_000;

/// Maximum bytes hashed per file. A larger file records its size with
/// `"digest": null` and a reason.
pub const MAX_HASHED_BYTES: u64 = 8 * 1024 * 1024;

/// The environment fingerprint of an explicit pair list: the sorted
/// `KEY=VALUE` lines, SHA-256, rendered `sha256:<hex>`. Deterministic by
/// construction under injected pairs; every observe adapter uses this same
/// rule (duplicated per adapter — adapter-to-adapter dependencies are
/// forbidden, so the function travels with each crate).
pub fn env_fingerprint(pairs: &[(String, String)]) -> String {
    let mut lines: Vec<String> = pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    lines.sort();
    let mut hasher = Sha256::new();
    for line in &lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    render_digest(&hasher.finalize())
}

fn render_digest(bytes: &[u8]) -> String {
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

/// The filesystem observer: bound to a workspace ROOT at construction
/// (canonicalized then, C1); a request resolves to a path under that
/// canonical root, refused if it would escape it.
pub struct FilesystemObserver {
    root: PathBuf,
    clock: Arc<dyn Clock>,
    store: Arc<dyn EvidenceStore>,
    env_fingerprint: String,
}

impl FilesystemObserver {
    /// Binds the observer to a workspace root, fingerprinting the observing
    /// process's environment.
    pub fn new(root: PathBuf, clock: Arc<dyn Clock>, store: Arc<dyn EvidenceStore>) -> Self {
        let pairs: Vec<(String, String)> = std::env::vars().collect();
        Self {
            root: canonical_root(root),
            clock,
            store,
            env_fingerprint: env_fingerprint(&pairs),
        }
    }

    /// Test/composition-root constructor over an explicit environment, so
    /// the recorded fingerprint (and everything derived from it) is a
    /// function of the declared pairs alone.
    pub fn from_parts(
        root: PathBuf,
        clock: Arc<dyn Clock>,
        store: Arc<dyn EvidenceStore>,
        env_pairs: &[(String, String)],
    ) -> Self {
        Self {
            root: canonical_root(root),
            clock,
            store,
            env_fingerprint: env_fingerprint(env_pairs),
        }
    }
}

impl Observer for FilesystemObserver {
    fn observe(
        &self,
        request: &ObservationRequest,
        deadline: Deadline,
    ) -> Result<ObservationFacet, ObserverError> {
        let kind = FacetKind::Filesystem;
        let workspace = resolve_workspace(&self.root, request, kind)?;
        let query = format!(
            "walk(root={}, max_depth={}, max_entries={}, max_file_bytes={}, \
             follow_symlinks=false)",
            request.workspace.as_str(),
            MAX_WALK_DEPTH,
            MAX_MANIFEST_ENTRIES,
            MAX_HASHED_BYTES,
        );
        let started = self.clock.now();
        let walk = walk_workspace(&workspace)
            .map_err(|detail| ObserverError::InstrumentFailed { kind, detail })?;
        let ended = self.clock.now();
        let summary = format!(
            "walked {} entries: {} files ({} hashed), {} dirs, {} symlinks, {} recorded errors{}",
            walk.files + walk.dirs + walk.symlinks,
            walk.files,
            walk.hashed,
            walk.dirs,
            walk.symlinks,
            walk.errors,
            if walk.truncated {
                format!(", truncated at the {}-entry cap", MAX_MANIFEST_ENTRIES)
            } else {
                String::new()
            },
        );
        let doc = FacetEvidenceDoc::new(
            INSTRUMENT_NAME.to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            ObservedRun {
                command: query.clone(),
                env_fingerprint: self.env_fingerprint.clone(),
                exit_status: 0,
                started_at: started.get(),
                ended_at: ended.get(),
                stdout: walk.manifest,
                stderr: String::new(),
            },
        )
        .map_err(|err| ObserverError::InstrumentFailed {
            kind,
            detail: format!("building the facet evidence document failed: {err}"),
        })?;
        let evidence_ref = self
            .store
            .put(doc.encode().as_bytes(), EvidenceKind::Log, deadline)
            .map_err(|err| ObserverError::Unavailable {
                kind,
                detail: format!("storing the facet evidence document failed: {err}"),
            })?;
        let instrument = instrument_identity(kind)?;
        Ok(ObservationFacet::new(kind, instrument, evidence_ref)
            .with_invocation(Some(query), Some(0))
            .with_window(Some(started), Some(ended))
            .with_summary(Some(summary)))
    }

    fn facet_kind(&self) -> FacetKind {
        FacetKind::Filesystem
    }
}

/// One adapter per instrument class (spec §6.2): this adapter fronts the
/// `filesystem` class.
fn instrument_identity(kind: FacetKind) -> Result<InstrumentIdentity, ObserverError> {
    let name = InstrumentName::new(INSTRUMENT_NAME).map_err(|err| ObserverError::Unavailable {
        kind,
        detail: format!("the instrument name is not a valid InstrumentName: {err}"),
    })?;
    Ok(InstrumentIdentity::new(
        name,
        env!("CARGO_PKG_VERSION").to_owned(),
    ))
}

/// The canonical workspace root, computed once at construction (C1): the
/// root itself may legitimately be a symlink (operator configuration), so
/// canonicalizing THROUGH it is correct — what containment requires is
/// that no WORKSPACE resolves outside the canonical root. A root that
/// does not exist yet is stored unresolved; any observe of a workspace
/// under it then fails the containment prefix check fail-closed.
fn canonical_root(root: PathBuf) -> PathBuf {
    root.canonicalize().unwrap_or(root)
}

/// Resolves a request's workspace under the observer's CANONICAL root,
/// refusing every ref that could resolve outside it (correction C1):
/// `.`/`..` components are refused outright, and the resolved path must
/// canonicalize back under the root — a symlinked workspace pointing
/// outside the root is refused, never followed. Existence on disk is the
/// truth (no membership list): a ref whose path does not exist is
/// `Unavailable`.
fn resolve_workspace(
    root: &Path,
    request: &ObservationRequest,
    kind: FacetKind,
) -> Result<PathBuf, ObserverError> {
    let reference = request.workspace.as_str();
    if let Some(part) = reference
        .split('/')
        .find(|part| *part == ".." || *part == ".")
    {
        return Err(ObserverError::Unavailable {
            kind,
            detail: format!(
                "workspace {reference} is refused: the '{part}' path component is a containment \
                 violation — a workspace must resolve inside the observer root"
            ),
        });
    }
    let resolved = match root.join(reference).canonicalize() {
        Ok(resolved) => resolved,
        Err(_) => {
            return Err(ObserverError::Unavailable {
                kind,
                detail: format!("workspace {reference} does not exist under the observer root"),
            });
        }
    };
    if !resolved.starts_with(root) {
        return Err(ObserverError::Unavailable {
            kind,
            detail: format!(
                "workspace {reference} resolves to {}, outside the canonical observer root {} — \
                 a containment violation (a symlinked workspace pointing outside the root is \
                 refused, never followed)",
                resolved.display(),
                root.display()
            ),
        });
    }
    Ok(resolved)
}

/// What the walk produced: the JSONL manifest plus its counts.
struct WalkOutcome {
    manifest: String,
    files: usize,
    hashed: usize,
    dirs: usize,
    symlinks: usize,
    errors: usize,
    truncated: bool,
}

/// Walks the workspace breadth-first, deterministically (entries sorted
/// by their [`component_spelling`] spelling per directory), never
/// following symlinks. Every cap is enforced
/// here, and the manifest records its own limits: a depth-capped directory
/// says so on its line, and a capped manifest ends with a truncation line.
fn walk_workspace(root: &Path) -> Result<WalkOutcome, String> {
    // A root that cannot even be probed is an instrument failure, not a
    // recorded reality (the walk contract needs a readable root).
    fs::read_dir(root).map_err(|err| {
        format!(
            "reading the workspace root {} failed: {err}",
            root.display()
        )
    })?;

    let mut lines: Vec<String> = Vec::new();
    let mut outcome = WalkOutcome {
        manifest: String::new(),
        files: 0,
        hashed: 0,
        dirs: 0,
        symlinks: 0,
        errors: 0,
        truncated: false,
    };
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) => {
                // C5: an unreadable-directory error line counts against the
                // manifest entry cap like any other line — a tree of
                // unlistable directories can no more push the manifest past
                // the cap than a tree of files can.
                if lines.len() >= MAX_MANIFEST_ENTRIES {
                    outcome.truncated = true;
                    break;
                }
                outcome.errors += 1;
                lines.push(error_line(
                    &relative(root, &dir),
                    &format!("read_dir failed: {err}"),
                ));
                continue;
            }
        };
        let mut named: Vec<(String, PathBuf)> = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => named.push((component_spelling(&entry.file_name()), entry.path())),
                Err(err) => {
                    // C5: iteration-error lines count against the cap too.
                    if lines.len() >= MAX_MANIFEST_ENTRIES {
                        outcome.truncated = true;
                        break;
                    }
                    outcome.errors += 1;
                    lines.push(error_line(
                        &relative(root, &dir),
                        &format!("iterating the directory failed: {err}"),
                    ));
                }
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, path) in named {
            if lines.len() >= MAX_MANIFEST_ENTRIES {
                outcome.truncated = true;
                break;
            }
            debug_assert!(
                path.file_name()
                    .is_some_and(|file_name| component_spelling(file_name) == name),
                "the entry path names its own file name under the C6 spelling"
            );
            let rel = relative(root, &path);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    outcome.errors += 1;
                    lines.push(error_line(&rel, &format!("stat failed: {err}")));
                    continue;
                }
            };
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                outcome.symlinks += 1;
                lines.push(entry_line(
                    &rel,
                    "symlink",
                    Some(metadata.len()),
                    None,
                    Some("symlink: listed, never followed".to_owned()),
                    None,
                ));
            } else if file_type.is_dir() {
                outcome.dirs += 1;
                if depth + 1 < MAX_WALK_DEPTH {
                    lines.push(entry_line(&rel, "dir", None, None, None, None));
                    queue.push_back((path, depth + 1));
                } else {
                    lines.push(entry_line(
                        &rel,
                        "dir",
                        None,
                        None,
                        Some(format!("depth cap {MAX_WALK_DEPTH}: listed, not descended")),
                        None,
                    ));
                }
            } else {
                outcome.files += 1;
                let size = metadata.len();
                let (digest, reason, error) = if size > MAX_HASHED_BYTES {
                    (
                        None,
                        Some(format!(
                            "file exceeds the per-file hash budget of {MAX_HASHED_BYTES} bytes"
                        )),
                        None,
                    )
                } else {
                    match hash_file(&path, size) {
                        Ok(hex) => (Some(hex), None, None),
                        Err(err) => {
                            outcome.errors += 1;
                            (None, None, Some(err))
                        }
                    }
                };
                if digest.is_some() {
                    outcome.hashed += 1;
                }
                lines.push(entry_line(&rel, "file", Some(size), digest, reason, error));
            }
        }
        if outcome.truncated {
            break;
        }
    }
    if outcome.truncated {
        lines.push(truncation_line());
    }
    outcome.manifest = lines.join("\n");
    if outcome.manifest.is_empty() {
        // An empty workspace is still a successful observation; one line
        // saying so keeps stdout a non-empty, self-describing record.
        outcome.manifest =
            "{\"path\":\".\",\"kind\":\"dir\",\"note\":\"empty workspace: no entries\"}".to_owned();
    }
    Ok(outcome)
}

/// The path relative to the walk root, forward slashes, each component
/// spelled by the C6 rule ([`component_spelling`]). For pure-ASCII trees
/// the bytes are identical to the historical lossy spelling (manifest
/// pins do not move).
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component_spelling(component.as_os_str()))
        .collect::<Vec<_>>()
        .join("/")
}

/// The manifest spelling of one file name — the C6 determinism rule:
///
/// - a name whose bytes are all ASCII renders exactly as it always has
///   (backslashes become forward slashes, as the historical lossy
///   spelling did), so pure-ASCII manifest bytes are unchanged;
/// - a name containing any non-ASCII byte renders every such byte as
///   `\xNN` (lowercase hex) and doubles its own backslashes, an
///   injective spelling on that class: two distinct non-UTF-8 names
///   whose LOSSY renderings collide (`a\xffb` and `a\xfdb` both lossy to
///   `a\u{FFFD}b`) become distinct, totally ordered spellings, and the
///   per-directory sort (by this spelling) is a total order.
///
/// No crossing between the classes: a PURE-ASCII name renders verbatim
/// with `\` mapped to `/`, so it can never contain a backslash, while
/// every escaped spelling carries at least one `\xNN` — an escaped
/// spelling cannot collide with an ASCII one (confirmation critic
/// MINOR-1 corrected the earlier over-stated residual note here).
fn component_spelling(name: &std::ffi::OsStr) -> String {
    let bytes = name.as_encoded_bytes();
    if bytes.iter().all(|byte| *byte < 0x80) {
        String::from_utf8_lossy(bytes).replace('\\', "/")
    } else {
        let mut spelling = String::new();
        for &byte in bytes {
            if byte == b'\\' {
                spelling.push_str("\\\\");
            } else if byte < 0x80 {
                spelling.push(byte as char);
            } else {
                spelling.push_str(&format!("\\x{byte:02x}"));
            }
        }
        spelling
    }
}

/// Reads at most `declared + 1` bytes and hashes them when the read stayed
/// within the declared size (a file that grew past the cap mid-walk is
/// recorded un-hashed, honestly).
fn hash_file(path: &Path, declared: u64) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|err| format!("open for hashing failed: {err}"))?;
    let mut bytes: Vec<u8> = Vec::new();
    file.take(MAX_HASHED_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("read for hashing failed: {err}"))?;
    if bytes.len() as u64 > MAX_HASHED_BYTES {
        return Err(format!(
            "file grew past the per-file hash budget of {MAX_HASHED_BYTES} bytes during the walk \
             (declared {declared})"
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(render_digest(&hasher.finalize()))
}

fn entry_line(
    path: &str,
    kind: &str,
    size: Option<u64>,
    digest: Option<String>,
    reason: Option<String>,
    error: Option<String>,
) -> String {
    let mut fields: Vec<(String, JsonValue)> = vec![
        ("path".to_owned(), JsonValue::Str(path.to_owned())),
        ("kind".to_owned(), JsonValue::Str(kind.to_owned())),
    ];
    if let Some(size) = size {
        fields.push(("size".to_owned(), JsonValue::Number(JsonNumber::Uint(size))));
    }
    fields.push((
        "digest".to_owned(),
        match digest {
            Some(hex) => JsonValue::Str(hex),
            None => JsonValue::Null,
        },
    ));
    if let Some(reason) = reason {
        fields.push(("reason".to_owned(), JsonValue::Str(reason)));
    }
    if let Some(error) = error {
        fields.push(("error".to_owned(), JsonValue::Str(error)));
    }
    JsonValue::Object(fields).to_json_text()
}

fn error_line(path: &str, error: &str) -> String {
    entry_line(path, "unreadable", None, None, None, Some(error.to_owned()))
}

fn truncation_line() -> String {
    JsonValue::Object(vec![
        ("truncated".to_owned(), JsonValue::Bool(true)),
        (
            "reason".to_owned(),
            JsonValue::Str(format!(
                "manifest entry cap of {MAX_MANIFEST_ENTRIES} reached"
            )),
        ),
    ])
    .to_json_text()
}

/// The deterministic environment the suite factories fingerprint — a fixed
/// pair list, so the recorded fingerprint is a function of the declaration.
fn suite_env() -> Vec<(String, String)> {
    vec![(
        "GAUNTLET_OBSERVER_SUITE".to_owned(),
        "filesystem".to_owned(),
    )]
}

/// Suite scaffolding for the shared `Observer` contract suite (spec §7.3):
/// `build` materializes each declared live workspace under a per-build
/// scratch root (the hand-rolled Scratch temp-dir idiom — `tempfile` is not
/// allowlisted) and returns an observer bound to that root with a
/// deterministic environment. Production wiring composes this adapter
/// through the plan Task 12/15 controller, not through this factory. The
/// factory removes its scratch roots on drop.
pub struct FilesystemObserverFactory {
    clock: Arc<dyn Clock>,
    store: Arc<dyn EvidenceStore>,
    roots: std::cell::RefCell<Vec<PathBuf>>,
    next: std::cell::Cell<u32>,
}

impl FilesystemObserverFactory {
    pub fn new(clock: Arc<dyn Clock>, store: Arc<dyn EvidenceStore>) -> Self {
        Self {
            clock,
            store,
            roots: std::cell::RefCell::new(Vec::new()),
            next: std::cell::Cell::new(0),
        }
    }
}

impl Drop for FilesystemObserverFactory {
    fn drop(&mut self) {
        for root in self.roots.borrow().iter() {
            let _ = fs::remove_dir_all(root);
        }
    }
}

impl gauntlet_ports::contract::observer::ObserverFactory for FilesystemObserverFactory {
    fn label(&self) -> &'static str {
        "gauntlet-observe-filesystem"
    }

    fn kind(&self) -> FacetKind {
        FacetKind::Filesystem
    }

    fn build(&self, live: Vec<WorkspaceRef>) -> Result<Box<dyn Observer>, String> {
        let seq = self.next.get();
        self.next.set(seq.wrapping_add(1));
        let root = std::env::temp_dir().join(format!(
            "gauntlet-observe-filesystem-{}-{seq}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root)
            .map_err(|err| format!("creating the suite scratch root failed: {err}"))?;
        for workspace in &live {
            let dir = root.join(workspace.as_str());
            fs::create_dir_all(&dir)
                .map_err(|err| format!("materializing a live workspace failed: {err}"))?;
            fs::write(
                dir.join("suite-entry.txt"),
                b"filesystem observer suite materialization\n",
            )
            .map_err(|err| format!("seeding a live workspace failed: {err}"))?;
        }
        self.roots.borrow_mut().push(root.clone());
        Ok(Box::new(FilesystemObserver::from_parts(
            root,
            self.clock.clone(),
            self.store.clone(),
            &suite_env(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{IterationNo, RunId};

    /// Per-test scratch tree; removed on drop.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "gauntlet-observe-filesystem-containment-{tag}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("the containment scratch parent is creatable");
            Self { path }
        }

        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.path.join(name);
            fs::create_dir_all(&dir).expect("the containment scratch dir is creatable");
            dir
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn containment_request(workspace: &str) -> ObservationRequest {
        ObservationRequest {
            run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            workspace: WorkspaceRef::new(workspace).expect("valid workspace ref"),
        }
    }

    /// Containment (correction C1): a workspace ref can never resolve the
    /// walked tree outside the observer's canonical root — `.`/`..`
    /// components are refused outright, a symlinked workspace pointing
    /// outside the root is refused (never followed), while a plain ref and
    /// a workspace under a symlinked ROOT (operator config, canonicalized
    /// through at construction) both still resolve.
    #[test]
    fn workspace_resolution_is_contained_inside_the_observer_root() {
        let scratch = Scratch::new("root");
        let root = scratch.dir("root");
        fs::create_dir_all(root.join("ws")).expect("the live workspace is creatable");
        // A sibling the `..` ref would reach, so the refusal below is about
        // containment, not existence.
        scratch.dir("sibling");
        let outside = scratch.dir("outside");
        std::os::unix::fs::symlink(&outside, root.join("ws-out"))
            .expect("the outside-pointing symlink is creatable");
        let canonical_root = root.canonicalize().expect("the scratch root canonicalizes");

        for escape in ["../sibling", "./ws"] {
            match resolve_workspace(
                &canonical_root,
                &containment_request(escape),
                FacetKind::Filesystem,
            ) {
                Err(ObserverError::Unavailable { detail, .. }) => {
                    assert!(
                        detail.contains("containment"),
                        "the refusal names containment for {escape}: {detail}"
                    );
                }
                other => panic!("ref {escape} must be refused, got {other:?}"),
            }
        }
        match resolve_workspace(
            &canonical_root,
            &containment_request("ws-out"),
            FacetKind::Filesystem,
        ) {
            Err(ObserverError::Unavailable { detail, .. }) => {
                assert!(
                    detail.contains("containment") && detail.contains("symlink"),
                    "the refusal names the symlinked-workspace containment violation: {detail}"
                );
            }
            other => panic!("a symlink outside the root must be refused, got {other:?}"),
        }

        // The honest refs still resolve: a plain workspace, and a workspace
        // under a symlinked ROOT (the root itself may be a symlink — that is
        // operator configuration, canonicalized through at construction).
        assert!(
            resolve_workspace(
                &canonical_root,
                &containment_request("ws"),
                FacetKind::Filesystem
            )
            .is_ok()
        );
        let linked_root = scratch.path.join("root-link");
        std::os::unix::fs::symlink(&root, &linked_root).expect("the root symlink is creatable");
        let through_link = linked_root.canonicalize().expect("the link canonicalizes");
        assert_eq!(through_link, canonical_root);
        assert!(
            resolve_workspace(
                &through_link,
                &containment_request("ws"),
                FacetKind::Filesystem
            )
            .is_ok()
        );
    }

    /// C5: unreadable-directory error lines count against
    /// [`MAX_MANIFEST_ENTRIES`] exactly like entry lines — a tree of
    /// unlistable directories can no more push the manifest past the cap
    /// than a tree of files can. The invariant: total manifest lines ≤
    /// cap + 1 (the truncation line included).
    #[test]
    fn manifest_error_lines_respect_the_entry_cap() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("cap");
        let root = scratch.dir("root");
        // Two unreadable directories queued while the root listing fills
        // the manifest to exactly the cap, so their read_dir error lines
        // are the pushes that would cross it.
        let unreadable_one = root.join("d1");
        let unreadable_two = root.join("d2");
        fs::create_dir_all(&unreadable_one).expect("the first unreadable dir is creatable");
        fs::create_dir_all(&unreadable_two).expect("the second unreadable dir is creatable");
        for index in 0..(MAX_MANIFEST_ENTRIES - 2) {
            fs::File::create(root.join(format!("f{index:05}")))
                .expect("the cap-filling file is creatable");
        }
        for dir in [&unreadable_one, &unreadable_two] {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o000))
                .expect("permissions are settable");
        }
        // Precondition: this host actually refuses to list the directories
        // (a root-running suite cannot make them unreadable, and the test
        // would be vacuous).
        for dir in [&unreadable_one, &unreadable_two] {
            assert!(
                fs::read_dir(dir).is_err(),
                "the host must refuse to list {} for the cap test to discriminate",
                dir.display()
            );
        }

        let outcome = walk_workspace(&root).expect("the walk completes");
        for dir in [&unreadable_one, &unreadable_two] {
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
        }
        let line_count = outcome.manifest.lines().count();
        assert!(
            line_count <= MAX_MANIFEST_ENTRIES + 1,
            "error lines must count against the cap: {line_count} lines for a cap of \
             {MAX_MANIFEST_ENTRIES}"
        );
        assert!(
            outcome.truncated,
            "a manifest refused mid-error is marked truncated"
        );
        assert!(
            outcome
                .manifest
                .lines()
                .next_back()
                .is_some_and(|last| last.contains("truncated")),
            "the final line is the truncation marker: {}",
            outcome.manifest.lines().next_back().unwrap_or("")
        );
    }

    /// C6: two distinct non-UTF-8 names whose LOSSY renderings collide get
    /// two distinct manifest lines, and the manifest bytes are stable
    /// across creation orders (sorted by the escaped spelling).
    #[test]
    fn lossy_colliding_names_get_distinct_deterministic_manifest_lines() {
        use std::os::unix::ffi::OsStringExt;

        let name_a = std::ffi::OsString::from_vec(b"a\xffb".to_vec());
        let name_b = std::ffi::OsString::from_vec(b"a\xfdb".to_vec());
        // The fixture is honest: distinct bytes, one lossy rendering.
        assert_ne!(name_a, name_b);
        assert_eq!(name_a.to_string_lossy(), name_b.to_string_lossy());

        // Two fresh workspaces with the two names created in opposite
        // orders must produce byte-identical manifests.
        let mut manifests: Vec<String> = Vec::new();
        for (tag, names) in [
            ("one", [name_a.clone(), name_b.clone()]),
            ("two", [name_b, name_a]),
        ] {
            let scratch = Scratch::new(tag);
            let root = scratch.dir("root");
            for name in names {
                fs::File::create(root.join(&name))
                    .expect("the non-UTF-8 file is creatable on this host");
            }
            let outcome = walk_workspace(&root).expect("the walk completes");
            manifests.push(outcome.manifest);
        }
        let lines: Vec<&str> = manifests[0].lines().collect();
        assert_eq!(lines.len(), 2, "both files appear: {}", manifests[0]);
        assert_ne!(
            lines[0], lines[1],
            "under two DISTINCT spellings: {lines:?}"
        );
        // The escape rule is visible: `\xff` and `\xfd` as path spellings
        // (JSON doubles the backslash inside the manifest line).
        assert!(
            manifests[0].contains("a\\\\xffb") && manifests[0].contains("a\\\\xfdb"),
            "the escaped spellings appear: {}",
            manifests[0]
        );
        assert_eq!(
            manifests[0], manifests[1],
            "creation order cannot move the manifest bytes"
        );
    }

    #[test]
    fn the_env_fingerprint_is_sorted_pairs_over_sha256() {
        let pairs = vec![
            ("B".to_owned(), "2".to_owned()),
            ("A".to_owned(), "x=y".to_owned()),
        ];
        let mut hasher = Sha256::new();
        hasher.update(b"A=x=y\n");
        hasher.update(b"B=2\n");
        let expected = render_digest(&hasher.finalize());
        assert_eq!(env_fingerprint(&pairs), expected);
        // Order of the input list is irrelevant; the sorted lines are the
        // fingerprint's input.
        let mut reversed = pairs.clone();
        reversed.reverse();
        assert_eq!(env_fingerprint(&reversed), expected);
        // A different environment is a different fingerprint.
        let other = vec![
            ("B".to_owned(), "2".to_owned()),
            ("A".to_owned(), "z".to_owned()),
        ];
        assert_ne!(env_fingerprint(&other), expected);
    }
}
