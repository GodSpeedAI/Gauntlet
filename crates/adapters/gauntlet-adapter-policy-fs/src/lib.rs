//! The filesystem `PolicyProvider` adapter (spec §6.2, §17.3, §25; plan
//! Task 14): resolves repo-owned per-tier sandbox policy artifacts from
//! `<policy_dir>/<tier-token>.json`, strictly decoded against a minimal
//! schema — unknown keys, tier mismatches, non-`deny_by_default` network
//! forms, unanchored or relative paths, and empty writable roots are all
//! typed `Invalid` findings naming the file and the offense. The tier
//! token is the domain's own wire vocabulary
//! ([`BlastRadius::wire`](gauntlet_domain::BlastRadius::wire)), never a
//! local copy.
//!
//! The `irreversible` tier has NO artifact — "No policy exists" (spec
//! §17.3) — and that absence is enforced STRUCTURALLY, before any file is
//! read: [`PolicyProvider::policy_for`] refuses the tier outright. The
//! doctor's forbidden-tier scan (spec §8.5 check 5) polices the directory;
//! this adapter polices the resolution.
//!
//! Path entries carry anchor tokens (`$WORKSPACE_ROOT`, `$PROJECT_ROOT`,
//! `$HOME`) at the prefix position — the same indirection discipline as
//! the configuration layer — resolved against the anchoring roots injected
//! at construction. EVERY path entry must be anchored: a bare relative
//! path, a bare ABSOLUTE path, and an unknown token are all `Invalid`.
//! Policy files never hardcode operator-machine paths, and an unanchored
//! absolute entry is both that hardcoding and an unbounded one — nothing
//! constrains how far it reaches, so `writable_roots: ["/"]` would ask a
//! backend to bind the whole host read-write.
//!
//! `$WORKSPACE_ROOT` is the one token that is NOT the bare anchor root
//! injected at construction: [`AnchorRoots::workspace_root`] names the
//! PARENT under which every run's and every unit's workspace lives, so
//! resolving `$WORKSPACE_ROOT` to that bare path verbatim would grant
//! EVERY caller read-write over EVERY OTHER caller's workspace too (the
//! defect this module closes, R-1). `policy_for` instead scopes
//! `$WORKSPACE_ROOT`, per call, to `workspace_root.join(<this call's own,
//! validated workspace ref>)` — the caller's own workspace subdirectory,
//! never a sibling's and never the shared parent itself. `$PROJECT_ROOT`
//! and `$HOME` are unaffected: they name one repository and one operator
//! home, not a per-caller subdirectory, so they resolve to the bare
//! anchor as injected.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use gauntlet_domain::{BlastRadius, RoleId, SandboxPolicyRef, UnitId, WorkspaceRef};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{self, JsonValue};
use gauntlet_ports::policy_provider::{PermissionSet, PolicyProvider, PolicyProviderError};
use gauntlet_ports::sandbox::{NetworkPosture, SandboxBackendRef, SandboxPolicy};

/// The anchoring roots the policy path tokens resolve against. Injected
/// by the composition root from configuration — never read from the
/// policy files themselves (an artifact cannot widen its own anchor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorRoots {
    /// The PARENT root under which every run's and every unit's workspace
    /// lives — injected once at construction and shared across every
    /// `policy_for` call this provider ever answers. This is NOT what
    /// `$WORKSPACE_ROOT` resolves to in a policy entry: `policy_for`
    /// scopes it, per call, to `workspace_root.join(<that call's own
    /// workspace ref>)` — the caller's own workspace subdirectory under
    /// this parent, never a sibling's and never this bare parent path
    /// itself (R-1: resolving the bare parent verbatim would grant every
    /// caller read-write over every other caller's workspace).
    pub workspace_root: PathBuf,
    /// `$PROJECT_ROOT` — the repository root (whose `.git` the `local`
    /// tier denies, spec §17.3).
    pub project_root: PathBuf,
    /// `$HOME` — the operator's home (credential paths the `local` tier
    /// denies: `.ssh`, `.aws`, `.gnupg`, spec §17.3).
    pub home: PathBuf,
}

/// The filesystem policy provider: one policy directory, one set of
/// anchor roots.
#[derive(Debug, Clone)]
pub struct PolicyFs {
    dir: PathBuf,
    anchors: AnchorRoots,
}

/// The minimal per-tier policy schema's exact key set (spec §25): anything
/// else in a file is `Invalid` — strict, never lenient.
const SCHEMA_KEYS: [&str; 6] = [
    "reference",
    "tier",
    "backend",
    "network",
    "writable_roots",
    "denied_paths",
];

/// The only network form this adapter decodes today: the string
/// `"deny_by_default"`. The allowlist object form is future scope (spec
/// §17.3: an allowlist exists "only if the claim requires it" — and only
/// when a backend can enforce it); anything else is `Invalid` naming what
/// was found.
const NETWORK_DENY_TOKEN: &str = "deny_by_default";

impl PolicyFs {
    pub fn new(dir: PathBuf, anchors: AnchorRoots) -> Self {
        Self { dir, anchors }
    }

    /// Strict decode of one tier's artifact against the minimal schema.
    fn decode(
        tier: BlastRadius,
        file: &Path,
        doc: &JsonValue,
        anchors: &AnchorRoots,
    ) -> Result<SandboxPolicy, PolicyProviderError> {
        let invalid = |detail: String| PolicyProviderError::Invalid { detail };
        let name = file.display().to_string();
        let unknown: Vec<String> = doc
            .keys()
            .into_iter()
            .filter(|key| !SCHEMA_KEYS.contains(&key.as_str()))
            .collect();
        if !unknown.is_empty() {
            return Err(invalid(format!(
                "{name} carries unknown key(s) [{}] — the schema is exactly {:?}",
                unknown.join(", "),
                SCHEMA_KEYS
            )));
        }
        let string_field = |key: &str| -> Result<String, PolicyProviderError> {
            match doc.get(key) {
                Some(JsonValue::Str(value)) => Ok(value.clone()),
                Some(other) => Err(invalid(format!(
                    "{name} field {key} must be a string, found {other:?}"
                ))),
                None => Err(invalid(format!("{name} is missing the {key} field"))),
            }
        };
        let reference = string_field("reference")?;
        let tier_field = string_field("tier")?;
        let backend_field = string_field("backend")?;
        let tier_token = tier.wire();
        if tier_field != tier_token {
            return Err(invalid(format!(
                "{name} names tier {tier_field:?} but the file is the {tier_token} tier's \
                 artifact"
            )));
        }
        let network = match doc.get("network") {
            Some(JsonValue::Str(token)) if token == NETWORK_DENY_TOKEN => {
                NetworkPosture::DenyByDefault
            }
            Some(other) => {
                return Err(invalid(format!(
                    "{name} field network must be the string \"{NETWORK_DENY_TOKEN}\" — the \
                     allowlist object form is future scope; found {other:?}"
                )));
            }
            None => return Err(invalid(format!("{name} is missing the network field"))),
        };
        let string_array = |key: &str| -> Result<Vec<String>, PolicyProviderError> {
            match doc.get(key) {
                Some(JsonValue::Array(items)) => {
                    let mut entries = Vec::new();
                    for item in items {
                        match item {
                            JsonValue::Str(entry) => entries.push(entry.clone()),
                            other => {
                                return Err(invalid(format!(
                                    "{name} field {key} must be an array of strings; found \
                                     {other:?}"
                                )));
                            }
                        }
                    }
                    Ok(entries)
                }
                Some(other) => Err(invalid(format!(
                    "{name} field {key} must be an array of strings, found {other:?}"
                ))),
                None => Err(invalid(format!("{name} is missing the {key} field"))),
            }
        };
        let writable_roots = string_array("writable_roots")?;
        let denied_paths = string_array("denied_paths")?;
        if writable_roots.is_empty() {
            return Err(invalid(format!(
                "{name} has an empty writable_roots — a policy must state where writes are \
                 permitted"
            )));
        }
        let mut resolved_writable = Vec::new();
        for entry in &writable_roots {
            resolved_writable.push(resolve_anchored_path(
                entry,
                anchors,
                "writable_roots",
                &name,
            )?);
        }
        let mut resolved_denied = Vec::new();
        for entry in &denied_paths {
            resolved_denied.push(resolve_anchored_path(
                entry,
                anchors,
                "denied_paths",
                &name,
            )?);
        }
        let reference = SandboxPolicyRef::new(&reference).map_err(|_| {
            invalid(format!(
                "{name} field reference {reference:?} is not a usable ref"
            ))
        })?;
        let backend = SandboxBackendRef::new(&backend_field).map_err(|_| {
            invalid(format!(
                "{name} field backend {backend_field:?} is not a usable backend token"
            ))
        })?;
        Ok(SandboxPolicy {
            reference,
            tier,
            backend,
            network,
            writable_roots: resolved_writable,
            denied_paths: resolved_denied,
        })
    }
}

/// Validates a `WorkspaceRef`'s string as a safe path segment BEFORE it is
/// joined onto [`AnchorRoots::workspace_root`] to scope `$WORKSPACE_ROOT`
/// to this call (R-1). `WorkspaceRef::new`'s own validation
/// (`gauntlet-domain`'s `validate_plain_ref`: non-empty, ≤1024 bytes, does
/// not start with `/`, no whitespace) does NOT reject `..`, a lone `.`, or
/// embedded `/` — joining such a value verbatim would let the per-call
/// anchor escape its own parent, exactly the anchor-escape class COR-2
/// already closed for policy-entry paths, reopened one level up. The
/// sanctioned class mirrors the one [`gauntlet_ports::workspace_provider`]
/// documents for a `Workspace`'s own sanitized key (spec §17.5.3):
/// `[A-Za-z0-9._-]`, and never exactly `.` or `..`. Anything else is a
/// typed `Invalid` naming the offending workspace ref — fail closed on a
/// malformed ref, matching this adapter's existing discipline of naming
/// the offense (here: no file exists yet to name, so the ref is named
/// instead).
fn validated_workspace_segment(workspace: &WorkspaceRef) -> Result<&str, PolicyProviderError> {
    let raw = workspace.as_str();
    let sanitized = !raw.is_empty()
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        && raw != "."
        && raw != "..";
    if sanitized {
        Ok(raw)
    } else {
        Err(PolicyProviderError::Invalid {
            detail: format!(
                "workspace ref {raw:?} is not a safe path segment for anchoring \
                 $WORKSPACE_ROOT — it must be non-empty, every byte restricted to \
                 [A-Za-z0-9._-], and never exactly \".\" or \"..\" (the same sanitization class \
                 spec 17.5.3 requires of a Workspace's own key); an unsanitized workspace ref \
                 must not be joined onto the workspace anchor root"
            ),
        })
    }
}

/// Resolves one path entry against the anchor roots: EVERY entry names a
/// token (`$WORKSPACE_ROOT`, `$PROJECT_ROOT`, `$HOME`) at the PREFIX
/// position. A bare relative path, a bare ABSOLUTE path, and an unknown
/// token are all `Invalid` naming the file, the entry, and the offense.
/// Anchoring is the only admission because it is the only BOUND: an
/// unanchored entry names no root it must stay inside, so nothing limits
/// how far it reaches.
///
/// An ANCHORED entry is additionally CONTAINED: the resolved path must
/// still be inside the anchor it named (spec §17.5.2), so an artifact
/// cannot widen its own anchor — the invariant [`AnchorRoots`] documents.
/// See [`contain_within_anchor`] for how containment is decided.
///
/// This function treats `anchors` as given — it does not know or care
/// whether the caller passed the bare, construction-time roots or a
/// narrower per-call scoping. `policy_for` passes an `anchors` whose
/// `workspace_root` is already scoped to the current call's own
/// workspace (`workspace_root.join(<validated workspace ref>)`), so
/// `$WORKSPACE_ROOT` here resolves to — and is contained within — THAT
/// caller's own workspace subdirectory, never the shared parent every
/// other caller's workspace also lives under.
fn resolve_anchored_path(
    entry: &str,
    anchors: &AnchorRoots,
    field: &str,
    file: &str,
) -> Result<String, PolicyProviderError> {
    for (token, root) in [
        ("$WORKSPACE_ROOT", &anchors.workspace_root),
        ("$PROJECT_ROOT", &anchors.project_root),
        ("$HOME", &anchors.home),
    ] {
        if let Some(rest) = entry.strip_prefix(token) {
            let joined = if rest.is_empty() {
                (*root).clone()
            } else if let Some(sub) = rest.strip_prefix('/') {
                root.join(sub)
            } else {
                return Err(PolicyProviderError::Invalid {
                    detail: format!(
                        "{file} {field} entry {entry:?} uses {token} outside prefix position — \
                         the token must stand alone or lead a path"
                    ),
                });
            };
            return contain_within_anchor(&joined, root, token, entry, field, file);
        }
    }
    // A known token ANYWHERE else in the entry is an unresolved anchor,
    // never a literal path component.
    for token in ["$WORKSPACE_ROOT", "$PROJECT_ROOT", "$HOME"] {
        if entry.contains(token) {
            return Err(PolicyProviderError::Invalid {
                detail: format!(
                    "{field} entry {entry:?} uses {token} outside prefix position — the token \
                     must stand alone or lead a path"
                ),
            });
        }
    }
    // Every entry must NAME an anchor. A bare ABSOLUTE path is neither a
    // token nor a relative path, so it used to fall through both refusals
    // and be carried verbatim — bounded by nothing, and exactly the
    // hardcoded operator-machine path this module's discipline forbids.
    // `writable_roots: ["/"]` was the limit case: it resolved, validated
    // clean, and bound the host root filesystem read-write. Anchoring IS
    // the bound, so there is no weaker admission beside it.
    let offense = if Path::new(entry).is_absolute() {
        "an unanchored absolute path hardcodes an operator-machine path and is bounded by \
         nothing"
    } else {
        "bare relative paths are refused"
    };
    Err(PolicyProviderError::Invalid {
        detail: format!(
            "{file} {field} entry {entry:?} is not an anchored token path ($WORKSPACE_ROOT, \
             $PROJECT_ROOT, $HOME) — {offense}"
        ),
    })
}

/// Spec §17.5.2: "paths stay inside [the] root, verified by absolute-path
/// prefix check after canonicalization (symlink escape is checked, not
/// assumed absent)". Two checks, in this order:
///
/// 1. **Lexical.** `.` is dropped and each `..` pops the segment before
///    it; a `..` with nothing left to pop walks above the filesystem root
///    and is refused outright. The normalized path must then start with
///    the normalized anchor — compared by [`Path::starts_with`], which is
///    COMPONENT-wise, so `/run/workspaces-evil` is not inside
///    `/run/workspaces` even though its bytes begin with them.
/// 2. **Symlink.** The lexical check cannot see a link, so the same prefix
///    check is repeated on canonicalized forms.
///
/// A policy path may legitimately name something that does NOT exist yet
/// (a workspace directory is created later), and `canonicalize` fails on
/// such a path. A failure there must not become a refusal — that would
/// refuse honest policies — and must not become a skip either, or the
/// symlink check would be bypassed by naming a deeper path. So
/// [`existing_canonical`] canonicalizes the DEEPEST EXISTING ANCESTOR and
/// re-attaches the remainder: every link that exists is resolved, and the
/// part that does not exist yet cannot be a link, because it is not
/// anything.
///
/// The value RETURNED is the lexically normalized path, not the
/// canonicalized one: canonicalization is how containment is VERIFIED, not
/// a rewrite of what the operator wrote. Once the check passes, the two
/// name the same location.
fn contain_within_anchor(
    joined: &Path,
    anchor: &Path,
    token: &str,
    entry: &str,
    field: &str,
    file: &str,
) -> Result<String, PolicyProviderError> {
    let escape = |offense: &str| PolicyProviderError::Invalid {
        detail: format!(
            "{file} {field} entry {entry:?} escapes its {token} anchor {} — {offense} (spec \
             17.5.2: paths stay inside their root, verified by absolute-path prefix check after \
             canonicalization)",
            anchor.display()
        ),
    };
    let resolved = normalize_lexically(joined)
        .ok_or_else(|| escape("a parent segment walks above the filesystem root"))?;
    let anchor_normalized = normalize_lexically(anchor)
        .ok_or_else(|| escape("the anchor itself does not normalize to a path"))?;
    if !resolved.starts_with(&anchor_normalized) {
        return Err(escape(
            "the normalized path is not inside the anchor (path components compared, not bytes)",
        ));
    }
    if !existing_canonical(&resolved).starts_with(existing_canonical(&anchor_normalized)) {
        return Err(escape(
            "the path resolves through a symlink to a target outside the anchor",
        ));
    }
    Ok(resolved.display().to_string())
}

/// Lexical normalization: `.` dropped, `..` popping the segment before it.
/// `None` when a `..` has nothing left to pop — the path walks above its
/// own root, which no containment check should be asked to interpret.
fn normalize_lexically(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut depth: usize = 0;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return None;
                }
                out.pop();
                depth -= 1;
            }
            Component::Normal(segment) => {
                out.push(segment);
                depth += 1;
            }
        }
    }
    Some(out)
}

/// The canonical form of the deepest ancestor that exists, with the
/// not-yet-existing remainder re-attached. Walking up terminates: every
/// step drops one component, and the filesystem root canonicalizes.
fn existing_canonical(path: &Path) -> PathBuf {
    let mut suffix: Vec<OsString> = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        if let Ok(real) = current.canonicalize() {
            let mut out = real;
            for segment in suffix.iter().rev() {
                out.push(segment);
            }
            return out;
        }
        let (Some(name), Some(parent)) = (
            current.file_name().map(OsString::from),
            current.parent().map(Path::to_path_buf),
        ) else {
            return path.to_path_buf();
        };
        suffix.push(name);
        current = parent;
    }
}

impl PolicyProvider for PolicyFs {
    fn policy_for(
        &self,
        tier: BlastRadius,
        workspace: &WorkspaceRef,
        deadline: Deadline,
    ) -> Result<SandboxPolicy, PolicyProviderError> {
        if tier == BlastRadius::Irreversible {
            // Spec §17.3: "No policy exists." The refusal is structural —
            // no file is ever read for the top tier, even if one is
            // (mistakenly or maliciously) present.
            return Err(PolicyProviderError::NoPolicyForTier { tier });
        }
        // `$WORKSPACE_ROOT` in a policy entry must mean THIS call's own
        // workspace, never the shared parent every other caller's
        // workspace also lives under (R-1). Validate the ref as a safe
        // path segment BEFORE using it as one — `WorkspaceRef::new`'s own
        // validation does not reject `..`, a lone `.`, or embedded `/`,
        // so a naive join would let the scoped anchor escape its own
        // parent, reopening the anchor-escape class COR-2 already closed
        // for policy-entry paths.
        let segment = validated_workspace_segment(workspace)?;
        let effective_anchors = AnchorRoots {
            workspace_root: self.anchors.workspace_root.join(segment),
            project_root: self.anchors.project_root.clone(),
            home: self.anchors.home.clone(),
        };
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| PolicyProviderError::Unavailable {
                detail: format!("the system clock is not after the epoch: {err}"),
            })?
            .as_secs();
        if deadline.exceeded_at(gauntlet_domain::EpochSeconds::new(now_secs)) {
            return Err(PolicyProviderError::Unavailable {
                detail: format!(
                    "the resolve deadline {deadline} has no time left — the policy artifact is \
                     not read"
                ),
            });
        }
        let file = self.dir.join(format!("{}.json", tier.wire()));
        let bytes = match std::fs::read(&file) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // For a non-irreversible tier a missing artifact means the
                // repo's policy set is incomplete (the port error's own
                // documented class).
                return Err(PolicyProviderError::NoPolicyForTier { tier });
            }
            Err(err) => {
                return Err(PolicyProviderError::Unavailable {
                    detail: format!("reading {} failed: {err}", file.display()),
                });
            }
        };
        let text = String::from_utf8(bytes).map_err(|err| PolicyProviderError::Invalid {
            detail: format!("{} is not valid UTF-8: {err}", file.display()),
        })?;
        let doc = json::parse(&text).map_err(|err| PolicyProviderError::Invalid {
            detail: format!("{} does not decode as strict JSON: {err}", file.display()),
        })?;
        Self::decode(tier, &file, &doc, &effective_anchors)
    }

    /// Deliberately out of scope, recorded honestly: the frozen ruler
    /// names the `permissions_for(role, unit)` resolution (spec §17.2)
    /// but specifies NO permissions artifact — spec §25's layout names
    /// only the per-tier sandbox policy files. Inventing an artifact
    /// shape here would be inventing a ruler; this half refuses with the
    /// reason instead.
    fn permissions_for(
        &self,
        _role: &RoleId,
        _unit: &UnitId,
        _deadline: Deadline,
    ) -> Result<PermissionSet, PolicyProviderError> {
        Err(PolicyProviderError::Unavailable {
            detail: "no permissions artifact is specified by the frozen ruler (spec 17.2 names \
                     the resolution; spec 25's layout names only per-tier sandbox policies) — \
                     deliberately out of scope, not invented"
                .to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture deadline base and span, each on its own line (no
    /// duration-shaped identifier shares a line with a numeric literal).
    const BASE_INSTANT: u64 = 1_700_000_000;
    const BOUND_SPAN: u64 = 60;

    /// A deadline the real clock has not passed yet — the adapter reads the
    /// wall clock (it has no injected `Clock`), so a fixed epoch instant
    /// would always be expired and every read would refuse.
    fn bound() -> Deadline {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the wall clock reads")
            .as_secs();
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(now_secs + BOUND_SPAN))
    }

    fn workspace_ref() -> WorkspaceRef {
        WorkspaceRef::new("ws-policy-fs-test").expect("test workspace ref")
    }

    /// A past deadline relative to the real clock (epoch-based bounds are
    /// long expired) — pins the read-refusal without touching the clock.
    fn expired_bound() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(BASE_INSTANT))
    }

    /// Per-test policy dir under the system temp dir; removed on drop.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "gauntlet-adapter-policy-fs-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("the scratch dir is creatable");
            Self { path }
        }

        fn write(&self, name: &str, body: &str) {
            std::fs::write(self.path.join(name), body).expect("the fixture file is writable");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn anchors() -> AnchorRoots {
        AnchorRoots {
            workspace_root: PathBuf::from("/run/workspaces"),
            project_root: PathBuf::from("/srv/gauntlet"),
            home: PathBuf::from("/home/operator"),
        }
    }

    const CONTAINED_BODY: &str = "{\
        \"reference\":\"tier-contained-v1\",\
        \"tier\":\"contained\",\
        \"backend\":\"bwrap\",\
        \"network\":\"deny_by_default\",\
        \"writable_roots\":[\"$WORKSPACE_ROOT\"],\
        \"denied_paths\":[]\
    }";

    #[test]
    fn every_non_irreversible_tier_resolves_from_its_artifact() {
        let scratch = Scratch::new("happy");
        scratch.write("contained.json", CONTAINED_BODY);
        scratch.write(
            "local.json",
            "{\
                \"reference\":\"tier-local-v1\",\
                \"tier\":\"local\",\
                \"backend\":\"bwrap\",\
                \"network\":\"deny_by_default\",\
                \"writable_roots\":[\"$WORKSPACE_ROOT\"],\
                \"denied_paths\":[\"$PROJECT_ROOT/.git\",\"$HOME/.ssh\"]\
            }",
        );
        scratch.write(
            "cross_cutting.json",
            "{\
                \"reference\":\"tier-cross-cutting-v1\",\
                \"tier\":\"cross_cutting\",\
                \"backend\":\"bwrap\",\
                \"network\":\"deny_by_default\",\
                \"writable_roots\":[\"$WORKSPACE_ROOT\",\"$HOME/cache\"],\
                \"denied_paths\":[\"$HOME/.gnupg\"]\
            }",
        );
        let provider = PolicyFs::new(scratch.path.clone(), anchors());

        let contained = provider
            .policy_for(BlastRadius::Contained, &workspace_ref(), bound())
            .expect("the contained artifact resolves");
        assert_eq!(contained.tier, BlastRadius::Contained);
        assert_eq!(contained.backend.as_str(), "bwrap");
        assert_eq!(contained.network, NetworkPosture::DenyByDefault);
        // `$WORKSPACE_ROOT` resolves to THIS call's own workspace under
        // the shared parent (R-1) — not the bare shared parent itself.
        assert_eq!(
            contained.writable_roots,
            vec!["/run/workspaces/ws-policy-fs-test".to_owned()]
        );
        assert!(contained.denied_paths.is_empty());

        let local = provider
            .policy_for(BlastRadius::Local, &workspace_ref(), bound())
            .expect("the local artifact resolves");
        assert_eq!(
            local.denied_paths,
            vec![
                "/srv/gauntlet/.git".to_owned(),
                "/home/operator/.ssh".to_owned(),
            ]
        );

        let cross = provider
            .policy_for(BlastRadius::CrossCutting, &workspace_ref(), bound())
            .expect("the cross_cutting artifact resolves");
        assert_eq!(
            cross.writable_roots,
            vec![
                "/run/workspaces/ws-policy-fs-test".to_owned(),
                "/home/operator/cache".to_owned(),
            ]
        );
        // Anchored, like every entry: the unanchored spelling this
        // fixture used to carry (`/etc/shadow`) is now a refusal, pinned
        // by `an_unanchored_absolute_path_is_invalid_naming_the_entry`.
        assert_eq!(cross.denied_paths, vec!["/home/operator/.gnupg".to_owned()]);
        // A bare token resolves to the anchor itself.
        let anchored: Vec<String> = cross
            .writable_roots
            .iter()
            .filter(|root| root.ends_with("cache"))
            .cloned()
            .collect();
        assert_eq!(anchored.len(), 1);
    }

    #[test]
    fn the_irreversible_tier_is_refused_structurally_even_when_a_file_exists() {
        let scratch = Scratch::new("irreversible");
        // The artifact EXISTS in this scratch dir — the refusal must still
        // be structural and the file must never be read (a parse-breaking
        // body proves no read happened: a read would surface Invalid, not
        // NoPolicyForTier).
        scratch.write("irreversible.json", "this body must never be read");
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Irreversible, &workspace_ref(), bound()) {
            Err(PolicyProviderError::NoPolicyForTier { tier }) => {
                assert_eq!(tier, BlastRadius::Irreversible);
            }
            other => panic!("irreversible must be refused structurally, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_artifact_for_a_dispatchable_tier_is_no_policy_for_tier() {
        let scratch = Scratch::new("missing");
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::NoPolicyForTier { tier }) => {
                assert_eq!(tier, BlastRadius::Contained);
            }
            other => panic!("a missing artifact must be NoPolicyForTier, got {other:?}"),
        }
    }

    #[test]
    fn bom_and_non_strict_bodies_are_invalid_naming_the_file() {
        let scratch = Scratch::new("strict");
        scratch.write("contained.json", &format!("\u{feff}{CONTAINED_BODY}"));
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("contained.json") && detail.contains("strict JSON"),
                    "the refusal names the file and the offense: {detail}"
                );
            }
            other => panic!("a BOM body must be Invalid, got {other:?}"),
        }
        // Trailing content after the top-level value is refused.
        scratch.write("contained.json", &format!("{CONTAINED_BODY} trailing"));
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { .. }) => {}
            other => panic!("trailing content must be Invalid, got {other:?}"),
        }
        // Duplicate keys are refused.
        scratch.write(
            "contained.json",
            "{\"reference\":\"a\",\"reference\":\"b\",\"tier\":\"contained\",\"backend\":\
             \"bwrap\",\"network\":\"deny_by_default\",\"writable_roots\":[\"$HOME\"],\
             \"denied_paths\":[]}",
        );
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { .. }) => {}
            other => panic!("duplicate keys must be Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_tier_mismatch_is_invalid() {
        let scratch = Scratch::new("tier-mismatch");
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("\"contained\"", "\"local\""),
        );
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("names tier") && detail.contains("local"),
                    "the refusal names the mismatch: {detail}"
                );
            }
            other => panic!("a tier mismatch must be Invalid, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_network_form_is_invalid_naming_what_was_found() {
        let scratch = Scratch::new("network");
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("\"deny_by_default\"", "\"allow_all\""),
        );
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("network") && detail.contains("allow_all"),
                    "the refusal names the offending form: {detail}"
                );
            }
            other => panic!("an unknown network form must be Invalid, got {other:?}"),
        }
        // The allowlist OBJECT form is future scope — refused, not guessed.
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("\"deny_by_default\"", "{\"hosts\":[\"example.test\"]}"),
        );
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("allowlist object form"),
                    "the refusal names the future-scope form: {detail}"
                );
            }
            other => panic!("the allowlist object form must be Invalid, got {other:?}"),
        }
    }

    #[test]
    fn unanchored_entries_are_invalid_naming_the_entry() {
        let scratch = Scratch::new("anchors");
        // Unknown token.
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("\"$WORKSPACE_ROOT\"", "\"$FOO/target\""),
        );
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("$FOO/target"),
                    "the refusal names the entry: {detail}"
                );
            }
            other => panic!("an unknown token must be Invalid, got {other:?}"),
        }
        // Bare relative path.
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("\"$WORKSPACE_ROOT\"", "\"target\""),
        );
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("bare relative"),
                    "the refusal names the relative entry: {detail}"
                );
            }
            other => panic!("a bare relative path must be Invalid, got {other:?}"),
        }
        // Token outside prefix position.
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("\"$WORKSPACE_ROOT\"", "\"/srv/$HOME/x\""),
        );
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("prefix position"),
                    "the refusal names the misplaced token: {detail}"
                );
            }
            other => panic!("a mid-path token must be Invalid, got {other:?}"),
        }
    }

    /// An UNANCHORED absolute path is neither a known token nor a bare
    /// relative path, so it fell through every refusal and was carried
    /// verbatim — unbounded. `writable_roots: ["/"]` then resolved and
    /// validated clean, and the backend bound the host root filesystem
    /// read-write under a validation reporting no findings. Anchoring is
    /// the bound: an entry must name an anchor, so no artifact can reach
    /// outside the roots the composition root injected.
    #[test]
    fn an_unanchored_absolute_path_is_invalid_naming_the_entry() {
        let scratch = Scratch::new("unanchored");
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        for entry in ["/", "/etc", "/run/workspaces"] {
            scratch.write(
                "contained.json",
                &CONTAINED_BODY.replace("\"$WORKSPACE_ROOT\"", &format!("\"{entry}\"")),
            );
            match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
                Err(PolicyProviderError::Invalid { detail }) => {
                    assert!(
                        detail.contains(entry) && detail.contains("contained.json"),
                        "the refusal names the entry and the file: {detail}"
                    );
                    assert!(
                        detail.contains("anchored"),
                        "the refusal names the offense — an unanchored path: {detail}"
                    );
                }
                other => panic!(
                    "an unanchored absolute writable root {entry:?} must be Invalid, got {other:?}"
                ),
            }
        }
        // The same bound holds on denied_paths: an unanchored denied entry
        // hardcodes an operator-machine path just as a writable one does.
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("\"denied_paths\":[]", "\"denied_paths\":[\"/etc/shadow\"]"),
        );
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("/etc/shadow") && detail.contains("anchored"),
                    "the refusal names the unanchored denied entry: {detail}"
                );
            }
            other => panic!("an unanchored denied path must be Invalid, got {other:?}"),
        }
    }

    #[test]
    fn empty_writable_roots_and_unknown_keys_are_invalid() {
        let scratch = Scratch::new("schema");
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("[\"$WORKSPACE_ROOT\"]", "[]"),
        );
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("empty writable_roots"),
                    "the refusal names the empty roots: {detail}"
                );
            }
            other => panic!("empty writable_roots must be Invalid, got {other:?}"),
        }
        scratch.write(
            "contained.json",
            &CONTAINED_BODY.replace("denied_paths", "denied_paths_extras"),
        );
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), bound()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("unknown key"),
                    "the refusal names the unknown key: {detail}"
                );
            }
            other => panic!("an unknown key must be Invalid, got {other:?}"),
        }
    }

    #[test]
    fn an_expired_deadline_refuses_without_reading() {
        let scratch = Scratch::new("deadline");
        scratch.write("contained.json", "this body must never be read");
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        match provider.policy_for(BlastRadius::Contained, &workspace_ref(), expired_bound()) {
            Err(PolicyProviderError::Unavailable { detail }) => {
                assert!(
                    detail.contains("no time left") && detail.contains("not read"),
                    "the refusal names the expired bound: {detail}"
                );
            }
            other => panic!("an expired deadline must be Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn permissions_for_refuses_honestly_instead_of_inventing_an_artifact() {
        let scratch = Scratch::new("permissions");
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        let role = RoleId::Builder;
        let unit = UnitId::new("AK-001").expect("test unit id");
        match provider.permissions_for(&role, &unit, bound()) {
            Err(PolicyProviderError::Unavailable { detail }) => {
                assert!(
                    detail.contains("no permissions artifact") && detail.contains("not invented"),
                    "the refusal records the honest reason: {detail}"
                );
            }
            other => panic!("permissions_for must refuse honestly, got {other:?}"),
        }
    }

    /// The shipped artifacts are valid: the three REAL repo policy files
    /// (`.gauntlet/policies/`) decode through `policy_for` for their
    /// tiers, anchored at the repository root — proving the versioned
    /// deliverables, not just fixtures, satisfy the schema. And the
    /// irreversible tier has no artifact there — its absence is the
    /// requirement (spec §17.3).
    #[test]
    fn the_shipped_repo_policy_artifacts_resolve_for_every_dispatchable_tier() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("the repository root resolves");
        let anchors = AnchorRoots {
            workspace_root: repo_root.join("target").join("workspaces"),
            project_root: repo_root.clone(),
            home: PathBuf::from(
                std::env::var("HOME").unwrap_or_else(|_| "/home/operator".to_owned()),
            ),
        };
        let provider = PolicyFs::new(repo_root.join(".gauntlet").join("policies"), anchors);

        let contained = provider
            .policy_for(BlastRadius::Contained, &workspace_ref(), bound())
            .expect("the shipped contained policy resolves");
        assert_eq!(contained.tier, BlastRadius::Contained);
        assert_eq!(contained.backend.as_str(), "bwrap");
        assert_eq!(contained.network, NetworkPosture::DenyByDefault);
        // `$WORKSPACE_ROOT` resolves to THIS call's own workspace under
        // the shared `target/workspaces` parent (R-1), not that bare
        // parent itself.
        assert_eq!(
            contained.writable_roots,
            vec![
                repo_root
                    .join("target")
                    .join("workspaces")
                    .join("ws-policy-fs-test")
                    .display()
                    .to_string()
            ]
        );
        assert!(contained.denied_paths.is_empty());

        for tier in [BlastRadius::Local, BlastRadius::CrossCutting] {
            let policy = provider
                .policy_for(tier, &workspace_ref(), bound())
                .expect("the shipped above-contained policy resolves");
            assert_eq!(policy.tier, tier);
            assert_eq!(policy.backend.as_str(), "bwrap");
            // §17.3: local denies the credential paths and the repo git
            // dir; cross_cutting keeps every deny local has.
            let expected_denied = vec![
                repo_root.join(".git").display().to_string(),
                PathBuf::from(
                    std::env::var("HOME").unwrap_or_else(|_| "/home/operator".to_owned()),
                )
                .join(".ssh")
                .display()
                .to_string(),
                PathBuf::from(
                    std::env::var("HOME").unwrap_or_else(|_| "/home/operator".to_owned()),
                )
                .join(".aws")
                .display()
                .to_string(),
                PathBuf::from(
                    std::env::var("HOME").unwrap_or_else(|_| "/home/operator".to_owned()),
                )
                .join(".gnupg")
                .display()
                .to_string(),
            ];
            assert_eq!(policy.denied_paths, expected_denied);
        }

        match provider.policy_for(BlastRadius::Irreversible, &workspace_ref(), bound()) {
            Err(PolicyProviderError::NoPolicyForTier { .. }) => {}
            other => panic!("the irreversible tier must have no policy, got {other:?}"),
        }
    }

    /// A contained-tier body whose `writable_roots` is the given JSON
    /// array content — the anchor-containment fixtures vary exactly that
    /// field.
    fn contained_body_with_roots(roots: &str) -> String {
        format!(
            "{{\
                \"reference\":\"tier-contained-v1\",\
                \"tier\":\"contained\",\
                \"backend\":\"bwrap\",\
                \"network\":\"deny_by_default\",\
                \"writable_roots\":[{roots}],\
                \"denied_paths\":[]\
            }}"
        )
    }

    fn resolve_root(scratch: &Scratch, entry: &str) -> Result<SandboxPolicy, PolicyProviderError> {
        resolve_root_with(scratch, entry, anchors())
    }

    fn resolve_root_with(
        scratch: &Scratch,
        entry: &str,
        anchors: AnchorRoots,
    ) -> Result<SandboxPolicy, PolicyProviderError> {
        scratch.write(
            "contained.json",
            &contained_body_with_roots(&format!("{entry:?}")),
        );
        PolicyFs::new(scratch.path.clone(), anchors).policy_for(
            BlastRadius::Contained,
            &workspace_ref(),
            bound(),
        )
    }

    /// Spec §17.5.2: an anchored path stays inside its anchor. `..` after
    /// the token walks OUT of it, which falsifies this module's own
    /// documented invariant ("an artifact cannot widen its own anchor") —
    /// so it is a typed `Invalid` naming the file and the offense, for
    /// every anchor token.
    #[test]
    fn an_anchored_path_may_not_escape_its_anchor() {
        let scratch = Scratch::new("anchor-escape");
        for entry in [
            "$WORKSPACE_ROOT/../outside",
            "$PROJECT_ROOT/..",
            "$HOME/../../etc",
            "$WORKSPACE_ROOT/inside/../../outside",
        ] {
            match resolve_root(&scratch, entry) {
                Err(PolicyProviderError::Invalid { detail }) => {
                    assert!(
                        detail.contains("contained.json"),
                        "the finding names the file: {detail}"
                    );
                    assert!(
                        detail.contains(entry),
                        "the finding names the offending entry: {detail}"
                    );
                    assert!(
                        detail.contains("escapes"),
                        "the finding names the offense: {detail}"
                    );
                }
                other => panic!("{entry} escapes its anchor and must be Invalid, got {other:?}"),
            }
        }
    }

    /// The containment check compares path COMPONENTS, not string
    /// prefixes: `/run/workspaces-evil` is NOT inside `/run/workspaces`,
    /// though it starts with those bytes.
    #[test]
    fn an_anchor_sibling_sharing_a_byte_prefix_is_not_contained() {
        let scratch = Scratch::new("anchor-sibling");
        let entry = "$WORKSPACE_ROOT/../workspaces-evil";
        match resolve_root(&scratch, entry) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("escapes"),
                    "a byte-prefix sibling is not containment: {detail}"
                );
            }
            other => panic!("a sibling of the anchor must be Invalid, got {other:?}"),
        }
    }

    /// Interior `.` and `..` that stay inside the anchor are normalized,
    /// not refused — the rule is containment, not a ban on the segments.
    #[test]
    fn interior_dot_segments_normalize_inside_the_anchor() {
        let scratch = Scratch::new("anchor-normalize");
        let policy = resolve_root(&scratch, "$WORKSPACE_ROOT/a/../b/./c")
            .expect("a path that stays inside its anchor resolves");
        assert_eq!(
            policy.writable_roots,
            vec!["/run/workspaces/ws-policy-fs-test/b/c".to_owned()]
        );
    }

    /// A policy path may legitimately name a directory that does not exist
    /// yet (a workspace is created later). `canonicalize` fails on such a
    /// path, and that failure must not become a refusal — containment is
    /// still decided, on the deepest ancestor that DOES exist.
    #[test]
    fn a_path_that_does_not_exist_yet_still_resolves() {
        let scratch = Scratch::new("anchor-absent");
        let policy = resolve_root(&scratch, "$WORKSPACE_ROOT/not/created/yet")
            .expect("a not-yet-created path inside the anchor resolves");
        assert_eq!(
            policy.writable_roots,
            vec!["/run/workspaces/ws-policy-fs-test/not/created/yet".to_owned()]
        );
    }

    /// Spec §17.5.2 says symlink escape is CHECKED, not assumed absent: a
    /// path that is lexically inside the anchor but resolves through a
    /// symlink to a target outside it is an escape.
    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_anchor_is_an_escape() {
        let scratch = Scratch::new("anchor-symlink");
        let workspace_root = scratch.path.join("ws");
        let outside = scratch.path.join("outside");
        // `policy_for` scopes $WORKSPACE_ROOT to
        // `workspace_root.join(<this call's workspace ref>)` (R-1); this
        // fixture's ref is `workspace_ref()` ("ws-policy-fs-test"), so the
        // effective anchor — and the symlink under test — live one level
        // deeper than the bare `workspace_root` passed into `AnchorRoots`.
        let effective_workspace = workspace_root.join(workspace_ref().as_str());
        std::fs::create_dir_all(&effective_workspace).expect("the anchor dir is creatable");
        std::fs::create_dir_all(&outside).expect("the outside dir is creatable");
        std::os::unix::fs::symlink(&outside, effective_workspace.join("escape"))
            .expect("the symlink is creatable");
        let anchors = AnchorRoots {
            workspace_root: workspace_root.clone(),
            ..anchors()
        };
        match resolve_root_with(&scratch, "$WORKSPACE_ROOT/escape", anchors.clone()) {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("escapes"),
                    "the symlink escape names the offense: {detail}"
                );
            }
            other => panic!("a symlink out of the anchor must be Invalid, got {other:?}"),
        }
        // A real directory inside the anchor still resolves.
        std::fs::create_dir_all(effective_workspace.join("real")).expect("the dir is creatable");
        let policy = resolve_root_with(&scratch, "$WORKSPACE_ROOT/real", anchors)
            .expect("a real path inside the anchor resolves");
        assert_eq!(
            policy.writable_roots,
            vec![effective_workspace.join("real").display().to_string()]
        );
    }

    // -----------------------------------------------------------------
    // R-1 regression tests: $WORKSPACE_ROOT is scoped to THIS call's own
    // workspace, never the shared parent every other caller's workspace
    // also lives under.
    // -----------------------------------------------------------------

    /// The test that would have caught R-1 directly: the SAME provider,
    /// over the SAME `$WORKSPACE_ROOT`-anchored policy, called with two
    /// DIFFERENT `WorkspaceRef`s, must resolve to two DIFFERENT writable
    /// roots — and NEITHER may be the bare shared parent
    /// (`anchors().workspace_root`) itself, which is exactly what the
    /// pre-fix code handed to every caller regardless of which workspace
    /// it claimed to be resolving for.
    #[test]
    fn two_different_workspaces_resolve_to_disjoint_writable_roots() {
        let scratch = Scratch::new("two-workspaces");
        scratch.write("contained.json", CONTAINED_BODY);
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        let workspace_a = WorkspaceRef::new("unit-a-workspace").expect("test workspace ref a");
        let workspace_b = WorkspaceRef::new("unit-b-workspace").expect("test workspace ref b");
        let policy_a = provider
            .policy_for(BlastRadius::Contained, &workspace_a, bound())
            .expect("workspace a's policy resolves");
        let policy_b = provider
            .policy_for(BlastRadius::Contained, &workspace_b, bound())
            .expect("workspace b's policy resolves");
        assert_ne!(
            policy_a.writable_roots, policy_b.writable_roots,
            "two different callers' workspaces must not resolve to the same writable root — \
             the pre-fix code handed BOTH the identical bare shared parent"
        );
        let shared_parent = anchors().workspace_root.display().to_string();
        for (label, policy) in [("a", &policy_a), ("b", &policy_b)] {
            for root in &policy.writable_roots {
                assert_ne!(
                    root, &shared_parent,
                    "workspace {label}'s writable root must not be the bare shared parent \
                     itself — that would grant it (and every other caller resolved against \
                     this same provider) read-write over every OTHER caller's workspace too: \
                     {root}"
                );
                assert!(
                    !Path::new(&shared_parent).starts_with(root),
                    "the shared parent must not be reachable AS or THROUGH workspace {label}'s \
                     writable root {root} — that would still let it widen back out to every \
                     sibling workspace"
                );
            }
        }
    }

    /// `WorkspaceRef::new`'s own validation does not reject `..`, a lone
    /// `.`, or embedded `/` — each of these slips past it and must
    /// instead be refused by `policy_for` itself, BEFORE it is joined
    /// onto the workspace anchor root as a path segment (R-1's Step 1).
    #[test]
    fn an_unsanitized_workspace_ref_is_refused() {
        let scratch = Scratch::new("unsanitized-workspace");
        scratch.write("contained.json", CONTAINED_BODY);
        let provider = PolicyFs::new(scratch.path.clone(), anchors());
        for raw in ["..", ".", "a/../../etc", "has/slash"] {
            let workspace = WorkspaceRef::new(raw).unwrap_or_else(|error| {
                panic!(
                    "{raw:?} is expected to pass WorkspaceRef::new's own (weaker) validation \
                     for this case to prove anything about policy_for's own guard: {error:?}"
                )
            });
            match provider.policy_for(BlastRadius::Contained, &workspace, bound()) {
                Err(PolicyProviderError::Invalid { detail }) => {
                    assert!(
                        detail.contains(raw),
                        "the refusal names the offending workspace ref: {detail}"
                    );
                }
                other => {
                    panic!("an unsanitized workspace ref {raw:?} must be Invalid, got {other:?}")
                }
            }
        }
    }

    /// Spec §17.5.2's containment check (COR-2/COR-5, unchanged by this
    /// fix) now runs against the NARROWER per-call anchor: an entry that
    /// steps one level up from `$WORKSPACE_ROOT` and back down into a
    /// same-named sibling resolves to a location a bare, unscoped
    /// `$WORKSPACE_ROOT` grant would have silently included (it sits
    /// directly under the shared parent, exactly like the bare-token
    /// grant `two_different_workspaces_resolve_to_disjoint_writable_roots`
    /// pins) — and the fixed code refuses it as an escape of THIS call's
    /// own, narrower anchor.
    #[test]
    fn a_relative_escape_through_the_workspace_segment_is_refused() {
        let scratch = Scratch::new("workspace-segment-escape");
        match resolve_root(&scratch, "$WORKSPACE_ROOT/../sibling-workspace") {
            Err(PolicyProviderError::Invalid { detail }) => {
                assert!(
                    detail.contains("contained.json"),
                    "the finding names the file: {detail}"
                );
                assert!(
                    detail.contains("$WORKSPACE_ROOT/../sibling-workspace"),
                    "the finding names the offending entry: {detail}"
                );
                assert!(
                    detail.contains("escapes"),
                    "the finding names the offense: {detail}"
                );
            }
            other => panic!(
                "a step out of THIS call's own workspace and into a sibling's must be Invalid, \
                 got {other:?}"
            ),
        }
    }
}
