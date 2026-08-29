//! The bubblewrap `Sandbox` adapter (spec §6.2, §17.3, §17.5.4; plan Task
//! 14) — the strongest real backend this host adjudicated (Task 0
//! environment report: bubblewrap 0.11.1; no container or VM backend
//! exists here). The product name lives in this adapter and nowhere else
//! (spec §24.3); the port sees only the opaque `bwrap` identity token.
//!
//! Validation PROBES the real binary before judging any policy (spec §6.2
//! dry run): an unusable backend is a typed [`SandboxError::BackendUnavailable`]
//! carrying the locus, never a policy opinion. The achieved rung is
//! honestly `Isolation::Process` — a user-namespace mount sandbox wraps
//! process trees, not kernels.
//!
//! The wrapped command is ONE shell-invocable string that STARTS with the
//! backend invocation: containment from process start (spec §17.5.4,
//! AK-048) means the FIRST process is the sandbox. The baseline unshares
//! EVERY namespace this backend can hold — user, pid, ipc, uts, cgroup and
//! net — mounts the root filesystem read-only, takes a new session with no
//! controlling terminal, and leaves no orphaned children
//! (`--die-with-parent`). The process namespace is not decoration: the
//! adapter reports `Isolation::Process`, and a backend that shares the
//! host pid namespace leaves the contained process able to signal, kill,
//! suspend and read the `/proc` entries of processes OUTSIDE it — a
//! read-only root and an unshared network stand in the way of none of
//! that. See [`BASELINE_ARGS`] for what each flag holds and why the
//! namespaces precede the mounts that depend on them. Writable roots are
//! bound back read-write and denied paths are masked AFTER them — mounts
//! apply in argument order, so the mask must come last to outrank the bind
//! that encloses it.
//!
//! Ordering is only half of the nesting guarantee; ANCHORING is the other.
//! A mask names a PATH, and a writable root lets the contained process move
//! content between paths, so a denial reachable through an ordinary
//! directory inside a writable root is defeated by renaming that directory
//! — a write the policy explicitly permits — after which the next wrap of
//! the identical policy masks the vacated path, bubblewrap creates that
//! path inside the writable root, and the sandbox STARTS with the denial
//! gone. This backend does not fake that containment: a mask is trustworthy
//! exactly when it is a MOUNT POINT sitting directly inside a bound root
//! (the kernel refuses to rename, remove, or clobber a mount point), and
//! any other nesting is refused as a validation finding rather than served
//! and silently degraded.
//!
//! An allowlist network posture is an honest validation FINDING here:
//! `--unshare-net` is deny-by-default and this backend cannot enforce a
//! host allowlist — the posture is never silently downgraded (spec §17.3
//! "instead of blanket network only if the claim requires it" — and only
//! if the backend can deliver it).

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gauntlet_domain::{BlastRadius, EpochSeconds};
use gauntlet_ports::agent_runner::Isolation;
use gauntlet_ports::contract::sandbox::SandboxFactory;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::sandbox::{
    NetworkPosture, PolicyValidation, Sandbox, SandboxBackendRef, SandboxError, SandboxPolicy,
    SandboxedCommand,
};

/// The backend identity token (§8.2 vocabulary). Its shape is pure syntax
/// over this constant — pinned by test below, so the one `expect` in
/// [`Sandbox::backend_identity`] guards an established invariant.
const BACKEND_TOKEN: &str = "bwrap";

/// The pause between child `try_wait` probes while the deadline is
/// enforced — the smallest expressible tick; the deadline is the policy,
/// not the tick (same posture as the command observer adapter).
const PROBE_PAUSE: Duration = Duration::from_millis(1);

/// The baseline containment flags every probe and wrap share: root
/// read-only, every namespace this backend can hold unshared, no
/// controlling terminal, dev and proc mounted, children dying with the
/// sandbox.
///
/// The namespaces are listed BEFORE the mounts that depend on them.
/// `--proc` is the one that must: with `--unshare-pid` it has to be a
/// procfs for the NEW pid namespace, or the contained process reads the
/// host's process table through a mount that no longer describes it.
/// bubblewrap 0.11.1 sets up every namespace before applying any mount
/// operation, so the order is not what makes this work on this host — it
/// is what makes the dependency legible, and what keeps it correct if a
/// later bubblewrap ever applied the two in argument order.
///
/// Each flag, and why it is not optional:
///
/// - `--unshare-user` — the user namespace is what lets an unprivileged
///   binary hold any of the others, and what makes the read-only root
///   unwritable. bubblewrap takes one implicitly when it is not setuid;
///   naming it makes the requirement explicit rather than inherited from
///   how the binary happens to be installed.
/// - `--unshare-pid` — WITHOUT IT THE SANDBOX IS NOT A PROCESS SANDBOX.
///   A read-only root and an unshared network say nothing about the
///   process space: sharing the host pid namespace leaves the contained
///   process able to enumerate host processes, read what they were
///   invoked with through `/proc`, and signal them — `SIGTERM` kills,
///   `SIGSTOP` suspends. The adapter reports `Isolation::Process`, and
///   §17.3 requires an above-contained runner to report `isolation ≥
///   process`; a backend that cannot confine the process space is not
///   delivering that rung. Pinned by the shared suite's
///   `the_process_space_is_confined` and by this crate's
///   `live_confinement_holds_the_host_process_space`.
/// - `--unshare-ipc` — the SysV IPC namespace. Shared, the contained
///   process reads and writes the host's shared-memory segments and
///   semaphores, which is a host-mutating channel that no filesystem mask
///   covers.
/// - `--unshare-uts` — the hostname and domain name are host state a
///   contained process must not be able to change (§17.3's
///   "host-mutating fallbacks explicitly refused").
/// - `--unshare-cgroup` — the cgroup hierarchy the payload can see.
///   Named strictly, not with bubblewrap's `-try` variant: a `-try` flag
///   that silently does nothing on a host without cgroup namespaces would
///   leave this backend claiming a rung it did not achieve. Strict means a
///   host that cannot provide it fails the backend PROBE and gets a typed
///   `BackendUnavailable` — an honest refusal instead of a quiet
///   downgrade.
/// - `--unshare-net` — the network, deny-by-default (§17.3).
/// - `--new-session` — a new session with no controlling terminal, so the
///   payload cannot push characters into the operator's terminal input
///   queue (`TIOCSTI`). Some kernels disable that ioctl outright; this
///   backend does not rely on a host sysctl for a guarantee it can hold
///   itself. Verified not to disturb the payload's stdio: the wrap's
///   stdin, stdout and stderr are the ones the caller handed it.
///
/// `--die-with-parent` stays LAST, where it was: it is the lifetime
/// guarantee, not a containment flag, and with `--unshare-pid` it is
/// stronger than before — killing the sandbox now takes down the whole pid
/// namespace, so the deadline path in [`BwrapSandbox::run_bounded`] can no
/// longer leave a grandchild behind.
const BASELINE_ARGS: [&str; 15] = [
    "--ro-bind",
    "/",
    "/",
    "--unshare-user",
    "--unshare-pid",
    "--unshare-ipc",
    "--unshare-uts",
    "--unshare-cgroup",
    "--unshare-net",
    "--new-session",
    "--dev",
    "/dev",
    "--proc",
    "/proc",
    "--die-with-parent",
];

/// The words that separate the containment flags from the payload. The
/// wrapped command and the dry run share them, so the dry run exercises
/// the SHAPE the wrap will emit, not a shape of its own.
const PAYLOAD_ARGS: [&str; 3] = ["--", "sh", "-c"];

/// How one bounded backend spawn ended. `status` is `None` when the child
/// was killed at the deadline; `stderr` is the backend's own complaint,
/// which is the most precise account of a startup refusal there is.
struct BoundedRun {
    status: Option<std::process::ExitStatus>,
    stderr: String,
}

impl BoundedRun {
    /// The backend's complaint, ready to append to a detail string — empty
    /// when it said nothing, so a refusal never carries a dangling colon.
    fn complaint(&self) -> String {
        if self.stderr.is_empty() {
            String::new()
        } else {
            format!(" (the backend reported: {})", self.stderr)
        }
    }
}

/// The bubblewrap sandbox: the binary path is injected (never a hardcoded
/// literal — an operator or the factory locates it).
#[derive(Debug, Clone)]
pub struct BwrapSandbox {
    binary: PathBuf,
}

impl BwrapSandbox {
    pub fn new(binary: PathBuf) -> Self {
        Self { binary }
    }

    /// Reads the wall clock for deadline arithmetic (the adapter owns no
    /// `Clock`; the probe's bound comes from the caller's `Deadline`
    /// against the real now).
    fn epoch_now() -> Result<EpochSeconds, SandboxError> {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| SandboxError::BackendUnavailable {
                backend: SandboxBackendRef::new(BACKEND_TOKEN)
                    .expect("the fixed bwrap token is always valid"),
                detail: format!("the system clock is not after the epoch: {err}"),
            })?
            .as_secs();
        Ok(EpochSeconds::new(secs))
    }

    /// Spawns the backend on one argument vector under the deadline and
    /// reports how it ended. IO failures — an unspawnable binary, a broken
    /// wait — are [`SandboxError::BackendUnavailable`] whatever the caller
    /// was asking, because they say nothing about the policy. What the
    /// backend DECIDED (its exit status and its complaint) comes back for
    /// the caller to interpret: the same run is a backend fact for the
    /// baseline probe and a policy fact for the dry run.
    fn run_bounded(&self, args: &[String], deadline: Deadline) -> Result<BoundedRun, SandboxError> {
        let backend =
            SandboxBackendRef::new(BACKEND_TOKEN).expect("the fixed bwrap token is always valid");
        let now = Self::epoch_now()?;
        if deadline.exceeded_at(now) {
            return Err(SandboxError::BackendUnavailable {
                backend,
                detail: format!(
                    "the validate deadline {deadline} has no time left at {now} — the backend \
                     probe is not spawned"
                ),
            });
        }
        let span = Duration::from_secs(deadline.instant().get() - now.get());
        let kill_at = Instant::now() + span;
        let mut child = Command::new(&self.binary)
            .args(args)
            .stdout(Stdio::null())
            // Captured, not discarded: the backend's own complaint is the
            // most precise statement of why a policy cannot start, and it
            // belongs in the finding. bubblewrap reports a startup refusal
            // as a single line and then exits, so the pipe cannot fill and
            // deadlock the wait below.
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| SandboxError::BackendUnavailable {
                backend: backend.clone(),
                detail: format!(
                    "spawning the sandbox backend at {} failed: {err}",
                    self.binary.display()
                ),
            })?;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if Instant::now() >= kill_at {
                        let _ = child.kill();
                        break None;
                    }
                    std::thread::sleep(PROBE_PAUSE);
                }
                Err(err) => {
                    return Err(SandboxError::BackendUnavailable {
                        backend: backend.clone(),
                        detail: format!("waiting on the backend probe failed: {err}"),
                    });
                }
            }
        };
        // Reaps the child (no zombie remains) and drains the captured pipe.
        let stderr = match child.wait_with_output() {
            Ok(output) => String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            Err(err) => {
                return Err(SandboxError::BackendUnavailable {
                    backend,
                    detail: format!("reaping the backend probe failed: {err}"),
                });
            }
        };
        Ok(BoundedRun { status, stderr })
    }

    /// Probes that the backend is REAL and usable before any policy
    /// judgment: spawns the binary under the baseline containment shape
    /// running `true`, bounded by the deadline. A spawn failure, a nonzero
    /// exit, a signal death, or a deadline overrun is a typed
    /// [`SandboxError::BackendUnavailable`] — the requested backend cannot
    /// run here, and that fact precedes every policy opinion.
    fn probe_backend(&self, deadline: Deadline) -> Result<(), SandboxError> {
        let backend =
            SandboxBackendRef::new(BACKEND_TOKEN).expect("the fixed bwrap token is always valid");
        let mut args: Vec<String> = BASELINE_ARGS
            .iter()
            .map(|flag| (*flag).to_owned())
            .collect();
        args.push("true".to_owned());
        let run = self.run_bounded(&args, deadline)?;
        match run.status {
            Some(status) if status.success() => Ok(()),
            Some(status) => Err(SandboxError::BackendUnavailable {
                backend,
                detail: format!(
                    "the backend probe at {} exited with {status} — the binary is not a usable \
                     sandbox here{}",
                    self.binary.display(),
                    run.complaint()
                ),
            }),
            None => Err(SandboxError::BackendUnavailable {
                backend,
                detail: format!(
                    "the backend probe exceeded the validate deadline {deadline} and was killed"
                ),
            }),
        }
    }

    /// The DRY RUN of the POLICY itself (spec §6.2: `validate` is a dry
    /// run). The backend probe above establishes that the binary works;
    /// this establishes that THIS POLICY works, by asking the backend to
    /// perform the policy's OWN binds and masks — the exact argument vector
    /// [`BwrapSandbox::render`] will emit — and then run the same trivial
    /// `true` payload.
    ///
    /// Without it, `validate` linted the policy's FIELDS and probed the
    /// BACKEND, and a policy that could not start at all still validated
    /// clean and wrapped `Ok`: a denied path that is a symlink, or a
    /// writable root that is a symlink, hard-aborted bubblewrap at process
    /// start. That is fail-closed, but it surfaced as a raw spawn error
    /// AFTER the gate had durably recorded an admission, instead of as the
    /// designed escalation. Here it is a finding, so it escalates through
    /// `ValidationFailed` like every other policy offense.
    ///
    /// Cost: one extra `true` under the real mounts, milliseconds, on the
    /// same deadline as the probe. It does no work of its own — the payload
    /// is `true` and nothing is written. Its ONLY footprint is the mount
    /// point bubblewrap must create for a mask whose target is missing
    /// inside a writable root, which is a directory the policy itself
    /// declares writable and which the real wrap would create anyway.
    ///
    /// What it cannot pre-check, honestly: it proves the policy STARTS, not
    /// that every mask covers what the author meant. A denied path that is
    /// a symlink RESOLVING INSIDE a writable root can mount successfully
    /// while masking the link rather than the target. And the tree can
    /// change between this dry run and the wrap that follows — `render`
    /// re-reads the shapes it depends on at wrap time, but no path-based
    /// backend can close that window.
    fn dry_run_policy(&self, policy: &SandboxPolicy, deadline: Deadline) -> Option<String> {
        let mut args: Vec<String> = BASELINE_ARGS
            .iter()
            .map(|flag| (*flag).to_owned())
            .collect();
        args.extend(Self::policy_args(policy));
        args.extend(PAYLOAD_ARGS.iter().map(|word| (*word).to_owned()));
        args.push("true".to_owned());
        match self.run_bounded(&args, deadline) {
            // An IO failure is a backend fact, not a policy opinion; the
            // probe that precedes this one reports it in its own terms.
            Err(_) => None,
            Ok(run) => match run.status {
                Some(status) if status.success() => None,
                Some(status) => Some(format!(
                    "the policy's own binds and masks do not start this backend: the dry run \
                     exited with {status}{} — a policy that cannot start is a validation \
                     finding, not a spawn-time abort after the gate has already admitted the \
                     dispatch (spec 6.2: validate is a dry run)",
                    run.complaint()
                )),
                None => Some(format!(
                    "the policy's own binds and masks did not start this backend within the \
                     validate deadline {deadline} and the dry run was killed"
                )),
            },
        }
    }

    /// Every rule this backend refuses a policy on, as findings (all
    /// applicable findings are collected so the escalation names every
    /// offense).
    fn policy_findings(policy: &SandboxPolicy) -> Vec<String> {
        let mut findings = Vec::new();
        if policy.tier == BlastRadius::Irreversible {
            findings.push(
                "no policy exists for the irreversible tier (spec 17.3) — the action never \
                 reaches the sandbox unattended"
                    .to_owned(),
            );
        }
        if policy.backend.as_str() != BACKEND_TOKEN {
            findings.push(format!(
                "the policy names backend {}; this backend is '{BACKEND_TOKEN}' — a policy \
                 naming another backend must not be validated here",
                policy.backend
            ));
        }
        if matches!(policy.network, NetworkPosture::Allowlist { .. }) {
            findings.push(
                "--unshare-net is deny-by-default and this backend cannot enforce a host \
                 allowlist — the posture is a finding, never a silent downgrade"
                    .to_owned(),
            );
        }
        for root in &policy.writable_roots {
            if !Path::new(root).is_absolute() {
                findings.push(format!(
                    "writable root {root:?} is not an absolute path — a bind source must be"
                ));
            } else if names_filesystem_root(root) {
                findings.push(format!(
                    "writable root {root:?} names the filesystem root — binding it read-write \
                     hands the contained process the whole host and leaves this backend nothing \
                     to contain"
                ));
            }
        }
        for denied in &policy.denied_paths {
            if !Path::new(denied).is_absolute() {
                findings.push(format!(
                    "denied path {denied:?} is not an absolute path — a mask target must be"
                ));
            } else if let Some(intervening) = relocatable_ancestor(denied, &policy.writable_roots) {
                findings.push(format!(
                    "denied path {denied:?} is reachable through {:?}, an ordinary directory \
                     inside a declared writable root — renaming it is a write this policy \
                     PERMITS, and it carries the denied content out from under a mask that \
                     names only the old path. This backend masks BY PATH, so it cannot keep \
                     'denied even if otherwise writable' across that rename and refuses the \
                     policy rather than degrading silently. Declare {:?} a writable root too \
                     (a bound root is a mount point, which the kernel refuses to rename), or \
                     deny a path directly under a writable root",
                    intervening.display(),
                    intervening.display()
                ));
            }
        }
        findings
    }

    /// Renders the single shell-invocable command. Writable roots are
    /// bound back FIRST and denied paths are masked AFTER them: bubblewrap
    /// applies mount operations in ARGUMENT ORDER, so a later bind
    /// re-exposes what an earlier mask covered. The order is load-bearing:
    /// a denied path INSIDE a writable root is denied only because its
    /// mask outranks the bind enclosing it. Masking last costs the sibling
    /// case nothing (a sibling is masked either way, and is already
    /// outside every bind).
    ///
    /// Order alone is not enough, which is why this renderer may assume
    /// what [`BwrapSandbox::policy_findings`] has already enforced: every
    /// denied path it is handed is either outside every writable root, or
    /// sits directly inside one (with every directory between it and that
    /// root being a bound root of its own). Both admitted shapes put the
    /// mask at a MOUNT POINT whose every enclosing directory up to the
    /// read-only baseline is also a mount point, and the kernel refuses to
    /// rename, remove, or clobber a mount point — so the contained process
    /// cannot move the denied content to a path the mask does not name.
    ///
    /// - a denied path that exists as a DIRECTORY is covered by an empty
    ///   tmpfs (`--tmpfs <path>`);
    /// - one that exists as a FILE is covered by a read-only bind of
    ///   `/dev/null` over it — bubblewrap cannot tmpfs over a file
    ///   ("Can't mkdir ... Not a directory" aborts the sandbox), and the
    ///   `/dev/null` mask is a real read-denial, stronger than ro;
    /// - a path that does not exist renders as `--tmpfs <path>`. Outside
    ///   every writable root that fails backend STARTUP (the mount point
    ///   cannot be created on the read-only root) — fail-closed: the
    ///   contained process never runs with a mask missing. Inside a
    ///   writable root the mount point IS creatable, so the mask mounts
    ///   over an empty path and the sandbox starts. That is safe only
    ///   because of the anchoring rule above: an admitted mask cannot have
    ///   been vacated by the contained process, so a missing target is a
    ///   path that holds nothing, and masking it denies nothing less.
    ///
    /// EVERY word of the result is shell-safe. The words are assembled RAW
    /// and each one is passed through [`shell_word`] on its way into the
    /// string, so a policy's own text — a writable root, a denied path,
    /// even the located binary path — is one shell WORD, never command
    /// structure. Assembling raw and quoting once at the end is what makes
    /// that structural: a later push cannot forget the quoting, because no
    /// push does the quoting. The payload keeps its own unconditional
    /// `'...'` wrapper (the documented shape of the final argument), which
    /// is why it is appended after the mapping rather than through it.
    fn render(&self, command: &str, policy: &SandboxPolicy) -> String {
        let mut words: Vec<String> = vec![self.binary.display().to_string()];
        words.extend(BASELINE_ARGS.iter().map(|flag| (*flag).to_owned()));
        words.extend(Self::policy_args(policy));
        words.extend(PAYLOAD_ARGS.iter().map(|word| (*word).to_owned()));
        let mut rendered: Vec<String> = words.iter().map(|word| shell_word(word)).collect();
        rendered.push(format!("'{}'", single_quote_escape(command)));
        rendered.join(" ")
    }

    /// The policy's OWN containment arguments — the writable binds and the
    /// denied-path masks, in the load-bearing order [`BwrapSandbox::render`]
    /// documents. Factored out because [`BwrapSandbox::dry_run_policy`]
    /// must exercise EXACTLY these: a dry run of a shape the wrap does not
    /// emit proves nothing about the wrap.
    ///
    /// The mask shape depends on what is at the target NOW, which is why
    /// this is re-derived at wrap time rather than cached from validation.
    /// A target that does not exist AND cannot be created — it lies outside
    /// every writable root, so the read-only baseline covers it — gets NO
    /// mask, and that is not a gap: there is nothing at the path to read,
    /// and the contained process cannot bring anything into existence there
    /// to write. Emitting `--tmpfs` for it instead aborts the backend at
    /// startup ("Can't mkdir …: Read-only file system"), which is
    /// fail-closed but is a FALSE refusal — it would make an ordinary
    /// credential-denying policy unusable on any host that simply has no
    /// `~/.aws`, while the denial it names is already fully enforced.
    fn policy_args(policy: &SandboxPolicy) -> Vec<String> {
        let mut words: Vec<String> = Vec::new();
        for root in &policy.writable_roots {
            words.push("--bind".to_owned());
            words.push(root.clone());
            words.push(root.clone());
        }
        for denied in &policy.denied_paths {
            let target = Path::new(denied);
            if target.is_file() {
                words.push("--ro-bind".to_owned());
                words.push("/dev/null".to_owned());
                words.push(denied.clone());
            } else if target.exists() || inside_a_writable_root(denied, &policy.writable_roots) {
                words.push("--tmpfs".to_owned());
                words.push(denied.clone());
            }
        }
        words
    }
}

/// Whether an absolute path names the FILESYSTEM ROOT once `.` and `..`
/// are resolved lexically: `/`, `/..`, and `/tmp/..` all do. A writable
/// root is a `--bind <root> <root>`, so a root that resolves to `/`
/// re-mounts the entire host read-write over the baseline's read-only
/// `--ro-bind / /` — the sandbox would then contain nothing. Lexical
/// resolution is the right reading here: the entry is judged as WRITTEN,
/// before any filesystem lookup, so a nonexistent path cannot dodge the
/// bound by failing to canonicalize.
fn names_filesystem_root(root: &str) -> bool {
    let mut depth: usize = 0;
    for component in Path::new(root).components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => depth = depth.saturating_sub(1),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    depth == 0
}

/// Normalizes a path LEXICALLY: `.` is dropped and `..` pops the component
/// before it. No filesystem lookup happens — the entry is judged as
/// WRITTEN, the same reading [`names_filesystem_root`] takes, so a path
/// that does not exist yet cannot dodge a bound by failing to canonicalize,
/// and two spellings of one path (`/w/a` and `/w/x/../a`) compare equal.
fn normalize_lexically(path: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Whether a path lies strictly inside a declared writable root — the
/// question "could the contained process bring this path into existence?".
/// Inside a root it could; outside every root the baseline binds `/`
/// read-only, so it could not. Lexical, like every other judgment this
/// backend makes on policy text: it reads the entry as written and cannot
/// see a symlink that resolves elsewhere.
fn inside_a_writable_root(path: &str, writable_roots: &[String]) -> bool {
    let path = normalize_lexically(path);
    writable_roots
        .iter()
        .filter(|root| Path::new(root.as_str()).is_absolute())
        .map(|root| normalize_lexically(root))
        .any(|root| path.starts_with(&root) && path != root)
}

/// The directory that defeats a nested denial by being RENAMEABLE, if the
/// policy has one.
///
/// A `--tmpfs` or `--ro-bind` mask is a MOUNT POINT, and the kernel refuses
/// to rename, remove, or clobber a mount point (`EBUSY`) — so a denied path
/// is safe from relocation as long as every directory between it and the
/// writable root that encloses it is a mount point too. In a rendered
/// command the mount points are exactly the writable roots (each one a
/// `--bind`) and the masks themselves.
///
/// So: an ancestor of the denied path is relocatable when it lies STRICTLY
/// INSIDE some writable root — which makes it writable, hence renameable —
/// without being a declared writable root itself. The first such ancestor
/// found is the offense, and it is returned so the finding can name it.
/// An ancestor outside every writable root is not relocatable: the
/// baseline binds `/` read-only, so nothing outside a granted root can be
/// renamed at all.
///
/// The comparison is lexical, and is honest about that: it judges the
/// policy as written and cannot see a symlink that makes a path resolve
/// somewhere else. Symlinked roots and mask targets are a separate
/// question, answered by the dry run rather than by this lint.
fn relocatable_ancestor(denied: &str, writable_roots: &[String]) -> Option<PathBuf> {
    let roots: Vec<PathBuf> = writable_roots
        .iter()
        .filter(|root| Path::new(root.as_str()).is_absolute())
        .map(|root| normalize_lexically(root))
        .collect();
    let denied = normalize_lexically(denied);
    // `ancestors` yields the path itself first, then each parent in turn,
    // so skipping one leaves the STRICT ancestors nearest-first — the
    // finding then names the innermost offense.
    denied
        .ancestors()
        .skip(1)
        .find(|ancestor| {
            let inside_a_root = roots
                .iter()
                .any(|root| ancestor.starts_with(root) && *ancestor != root.as_path());
            inside_a_root && !roots.iter().any(|root| root.as_path() == *ancestor)
        })
        .map(Path::to_path_buf)
}

/// Renders one assembled word so the shell that reads the command passes
/// exactly this string as exactly one argument. A word made only of
/// [`is_shell_safe`] bytes already means itself, and is emitted unchanged
/// (the baseline flags and ordinary paths stay readable); anything else —
/// a space, a quote, a semicolon, a dollar sign, a newline — is wrapped in
/// the single-quote dance, which no shell metacharacter survives.
///
/// This is the defense against policy text becoming command structure: a
/// denied path of the shape `<path> --share-net` would otherwise render as
/// two words, the second a real backend flag arriving AFTER the baseline's
/// `--unshare-net` — and last flag wins, so a validation that reported
/// clean would have handed the contained process the host network. Quoted,
/// it is one nonexistent path, and the mask fails closed.
///
/// Refusing paths that contain spaces is deliberately NOT how this is
/// closed: an absolute path may legitimately contain one, and it must keep
/// binding. Quoting serves both.
fn shell_word(word: &str) -> String {
    if !word.is_empty() && word.chars().all(is_shell_safe) {
        word.to_owned()
    } else {
        format!("'{}'", single_quote_escape(word))
    }
}

/// Bytes that mean only themselves to a POSIX shell in an unquoted word.
/// The set is deliberately a conservative allowlist: anything outside it —
/// including every byte above ASCII — takes the quoted path, so a new
/// metacharacter cannot be admitted by omission.
fn is_shell_safe(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | ',' | '+' | '=' | '@')
}

/// The standard POSIX single-quote dance: the body sits inside `'...'` and
/// each embedded `'` becomes `'\''` — everything else (double quotes,
/// semicolons, dollar signs, backslashes, spaces) is literal inside single
/// quotes, so no payload and no policy word can escape its own argument.
fn single_quote_escape(command: &str) -> String {
    let mut out = String::with_capacity(command.len());
    for ch in command.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out
}

impl Sandbox for BwrapSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        // FIRST the probe: the backend must be real and usable before any
        // policy judgment (an unusable backend is not a policy opinion).
        self.probe_backend(deadline)?;
        // THEN the policy's own fields, and only if they are clean, the DRY
        // RUN of the policy itself (spec §6.2). The lints run first because
        // they are free and because dry-running a policy already known to
        // be ineligible would spend a spawn to learn nothing; a policy that
        // clears the lints is dry-run so that "it cannot even start" is a
        // finding here rather than a spawn-time abort later, after the gate
        // has durably recorded an admission.
        let mut findings = Self::policy_findings(policy);
        if findings.is_empty()
            && let Some(refusal) = self.dry_run_policy(policy, deadline)
        {
            findings.push(refusal);
        }
        if !findings.is_empty() {
            return Err(SandboxError::ValidationFailed {
                policy: policy.reference.clone(),
                findings,
            });
        }
        Ok(PolicyValidation {
            policy: policy.reference.clone(),
            backend: policy.backend.clone(),
            isolation: Isolation::Process,
            findings: Vec::new(),
        })
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        _deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        if validation.policy != policy.reference {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} does not cover policy {} — wrap requires the validation its \
                     own policy earned",
                    validation.policy.as_str(),
                    policy.reference.as_str()
                ),
            });
        }
        if !validation.is_clean() {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} carries {} finding(s) — a failed validation authorizes \
                     nothing (spec 21.3: escalate, never fall back)",
                    validation.policy.as_str(),
                    validation.findings.len()
                ),
            });
        }
        if validation.isolation != Isolation::Process {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation claims the {} rung; this backend achieves process isolation — \
                     a foreign validation is a cross-wiring",
                    validation.isolation.token()
                ),
            });
        }
        // A hand-forged clean validation must not bootstrap what `validate`
        // would refuse: the wrap re-derives the policy's eligibility here.
        let findings = Self::policy_findings(policy);
        if !findings.is_empty() {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "the policy is not eligible at this backend: {}",
                    findings.join("; ")
                ),
            });
        }
        Ok(SandboxedCommand {
            command: self.render(command, policy),
            policy: policy.reference.clone(),
            validation: validation.clone(),
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        // The constant's shape is an established invariant (pinned by
        // the identity-token test), so this expect cannot fire.
        SandboxBackendRef::new(BACKEND_TOKEN).expect("the fixed bwrap token is always valid")
    }
}

/// Locates the backend binary: PATH first, then the common locations. The
/// factory and operators resolve the path ONCE and inject it; nothing in
/// this adapter hardcodes a host location as authority.
/// Locates the backend binary (capability name: composition roots may not
/// spell the vendor flavor — the boundary suite denies it outside
/// adapters, and `crate::locate_bwrap`'s old name did).
pub fn locate() -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(BACKEND_TOKEN);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    ["/usr/bin", "/usr/local/bin", "/bin"]
        .iter()
        .map(|dir| Path::new(dir).join(BACKEND_TOKEN))
        .find(|candidate| candidate.is_file())
}

/// Capability-named façade for composition roots (spec §4.1): the crate
/// name may be spelled by the composition root (the boundary suite masks
/// crate names), but the vendor-flavored TYPE name may not — so the
/// re-export carries the semantic capability instead.
pub use BwrapSandbox as ProcessIsolationBackend;

/// Suite scaffolding for the shared `Sandbox` contract suite (spec §7.3):
/// the bubblewrap adapter runs the same suite every `Sandbox` adapter
/// must pass. `build` fails honestly when the backend is absent — the
/// suite treats a build failure as a finding, which is the correct
/// report on a host without the backend.
pub struct BwrapSandboxFactory;

impl SandboxFactory for BwrapSandboxFactory {
    fn label(&self) -> &'static str {
        "bwrap-backend"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        let binary = locate().ok_or_else(|| {
            format!("the '{BACKEND_TOKEN}' backend binary was not found on PATH or at the common locations")
        })?;
        Ok(Box::new(BwrapSandbox::new(binary)))
    }

    /// The suite's fixture directories, made REAL under the system temp
    /// dir. This backend's `validate` is an honest dry run, so it binds
    /// and masks the very paths a policy names — an invented fixture path
    /// would be refused, and rightly.
    fn fixture_dir(&self, segment: &str) -> PathBuf {
        let path = std::env::temp_dir()
            .join(format!(
                "gauntlet-adapter-sandbox-bwrap-suite-{}",
                std::process::id()
            ))
            .join(segment);
        let _ = std::fs::create_dir_all(&path);
        path
    }

    /// Runs a command this backend rendered. `render` emits ONE
    /// shell-invocable string starting with the located binary, so a shell
    /// is exactly the right runner — the same way the crate's live legs
    /// execute a wrap. Only the payload's stdout comes back; its exit
    /// status is an observation the suite reads off the markers, not a
    /// failure of the run.
    fn run_rendered(&self, command: &str) -> Result<String, String> {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|err| format!("spawning a shell for the rendered command failed: {err}"))?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::SandboxPolicyRef;
    use gauntlet_ports::contract::sandbox::sandbox_suite;

    /// The live tests' real wall span, on its own line (no duration-shaped
    /// identifier shares a line with a numeric literal).
    const LIVE_BOUND_SPAN: u64 = 30;

    /// Per-test scratch tree under the system temp dir; removed on drop.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "gauntlet-adapter-sandbox-bwrap-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("the scratch tree is creatable");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn live_bound() -> Deadline {
        let now = BwrapSandbox::epoch_now().expect("the wall clock reads");
        Deadline::at_epoch(EpochSeconds::new(now.get() + LIVE_BOUND_SPAN))
    }

    /// A fixed binary path for rendering tests (never executed).
    const RENDER_BINARY: &str = "/usr/bin/bwrap";

    fn render_sandbox() -> BwrapSandbox {
        BwrapSandbox::new(PathBuf::from(RENDER_BINARY))
    }

    fn contained_policy(writable: Vec<String>, denied: Vec<String>) -> SandboxPolicy {
        SandboxPolicy {
            reference: SandboxPolicyRef::new("tier-contained-test").expect("test ref"),
            tier: BlastRadius::Contained,
            backend: SandboxBackendRef::new(BACKEND_TOKEN).expect("test backend ref"),
            network: NetworkPosture::DenyByDefault,
            writable_roots: writable,
            denied_paths: denied,
        }
    }

    /// A live-backend policy validation; skips the test honestly when the
    /// backend is absent (on this host it is present — Task 0).
    fn live_backend() -> Option<BwrapSandbox> {
        locate().map(BwrapSandbox::new)
    }

    #[test]
    fn the_identity_token_is_a_valid_backend_ref() {
        // Pins the invariant behind backend_identity's expect.
        assert!(SandboxBackendRef::new(BACKEND_TOKEN).is_ok());
    }

    #[test]
    fn the_rendered_command_starts_with_the_backend_and_carries_every_root() {
        // Real paths: the mask a denied path renders as depends on what is
        // at the target, so an invented path would pin the wrong shape.
        let scratch = Scratch::new("render-shape");
        let root = scratch.path.join("root");
        let denied = scratch.path.join("denied");
        std::fs::create_dir_all(&root).expect("the writable root is creatable");
        std::fs::create_dir_all(&denied).expect("the denied dir is creatable");
        let policy = contained_policy(
            vec![root.display().to_string()],
            vec![denied.display().to_string()],
        );
        let rendered = render_sandbox().render("echo hi", &policy);
        let prefix = format!(
            "{RENDER_BINARY} --ro-bind / / --unshare-user --unshare-pid --unshare-ipc \
             --unshare-uts --unshare-cgroup --unshare-net --new-session --dev /dev --proc /proc \
             --die-with-parent"
        );
        assert!(
            rendered.starts_with(&prefix),
            "the wrapped command starts with the backend invocation: {rendered}"
        );
        assert!(
            rendered.contains(&format!("--tmpfs {}", denied.display())),
            "a directory-shaped denied path renders as a tmpfs mask: {rendered}"
        );
        assert!(
            rendered.contains(&format!("--bind {root} {root}", root = root.display())),
            "each writable root is bound back read-write: {rendered}"
        );
        assert!(
            rendered.ends_with("-- sh -c 'echo hi'"),
            "the command is the final single-quoted sh -c argument: {rendered}"
        );
    }

    /// The mask a denied path renders as is decided by what is AT the
    /// target, and a target that does not exist and cannot be created —
    /// outside every writable root, where the read-only baseline rules —
    /// gets no mask at all. That is not a gap: nothing is there to read,
    /// and the contained process cannot make anything appear there. It is
    /// what keeps an ordinary credential-denying policy usable on a host
    /// that simply has no `~/.aws`, instead of aborting the backend at
    /// startup on a denial that is already fully enforced.
    #[test]
    fn a_mask_target_that_cannot_exist_renders_no_mask_and_one_that_can_still_does() {
        let scratch = Scratch::new("absent-mask");
        let workspace = scratch.path.join("ws");
        std::fs::create_dir_all(&workspace).expect("the writable root is creatable");
        let uncreatable = scratch.path.join("no-such-credentials");
        let creatable = workspace.join(".git");
        let policy = contained_policy(
            vec![workspace.display().to_string()],
            vec![
                uncreatable.display().to_string(),
                creatable.display().to_string(),
            ],
        );
        let rendered = render_sandbox().render("true", &policy);
        assert!(
            !rendered.contains(&uncreatable.display().to_string()),
            "a missing target outside every writable root renders no mask — masking it \
             aborts the backend on a denial the read-only baseline already holds: {rendered}"
        );
        assert!(
            rendered.contains(&format!("--tmpfs {}", creatable.display())),
            "a missing target INSIDE a writable root still renders its mask — the contained \
             process could otherwise create and use it: {rendered}"
        );
    }

    #[test]
    fn single_quote_escaping_round_trips_the_adversarial_probe() {
        // The probe carries every character class that is dangerous
        // inside a double-quoted or unquoted shell argument.
        let probe = "a'b\"c;d$e\\f";
        let escaped = single_quote_escape(probe);
        assert_eq!(escaped, "a'\\''b\"c;d$e\\f");
        let policy = contained_policy(Vec::new(), Vec::new());
        let rendered = render_sandbox().render(probe, &policy);
        let tail = rendered
            .rsplit_once("sh -c '")
            .expect("the render ends with the sh -c argument")
            .1;
        assert_eq!(
            tail,
            format!("{escaped}'"),
            "the probe — and nothing else — is the quoted body"
        );
    }

    #[test]
    fn adversarial_payloads_stay_inside_the_quoted_argument() {
        // Payload one: a command carrying backend flags of its own.
        let flag_payload = "echo --bind /etc/passwd /tmp/hostage";
        // Payload two: a command chaining destruction after a semicolon.
        let chain_payload = "echo hi; rm -rf /";
        for payload in [flag_payload, chain_payload] {
            let policy = contained_policy(Vec::new(), Vec::new());
            let rendered = render_sandbox().render(payload, &policy);
            let head = rendered
                .split_once("sh -c '")
                .expect("the render separates flags from the payload")
                .0;
            assert!(
                !head.contains(payload),
                "the payload never leaks into the backend-flag section: {rendered}"
            );
            let tail = rendered
                .rsplit_once("sh -c '")
                .expect("the render ends with the sh -c argument")
                .1;
            assert_eq!(
                tail,
                format!("{}'", single_quote_escape(payload)),
                "the payload is exactly the quoted body, with nothing after it: {rendered}"
            );
        }
    }

    /// Policy text is DATA. A writable root or denied path carrying a
    /// flag after a space must not contribute that flag as a word of its
    /// own — `--share-net` arriving after the baseline's `--unshare-net`
    /// would win (last flag wins) and hand the contained process the host
    /// network under a validation that still reports clean. And the fix is
    /// QUOTING, not refusal: a legitimate absolute path containing a space
    /// must still bind, as one word.
    #[test]
    fn a_policy_path_cannot_smuggle_a_backend_flag_and_a_spaced_path_still_binds() {
        // A REAL directory whose own name carries the smuggled flag, so
        // the mask renders and the quoting assertion still bites.
        let scratch = Scratch::new("smuggler");
        let smuggler_path = scratch.path.join("root --share-net");
        std::fs::create_dir_all(&smuggler_path).expect("the hostile dir is creatable");
        let smuggler = smuggler_path.display().to_string();
        let policy = contained_policy(vec![smuggler.clone()], vec![smuggler.clone()]);
        let rendered = render_sandbox().render("true", &policy);
        assert!(
            !rendered.contains(&format!(
                "--unshare-net --die-with-parent --tmpfs {smuggler}"
            )),
            "the smuggled flag must not stand as a word of its own: {rendered}"
        );
        assert!(
            rendered.contains(&format!("--tmpfs '{smuggler}'")),
            "the hostile denied path is one quoted word: {rendered}"
        );
        assert!(
            rendered.contains(&format!("--bind '{smuggler}' '{smuggler}'")),
            "the hostile writable root is one quoted word on both sides: {rendered}"
        );

        let spaced = scratch.path.join("some user").join("ws");
        std::fs::create_dir_all(&spaced).expect("the spaced dir is creatable");
        let legitimate = spaced.display().to_string();
        let honest = contained_policy(vec![legitimate.clone()], Vec::new());
        let rendered = render_sandbox().render("true", &honest);
        assert!(
            rendered.contains(&format!("--bind '{legitimate}' '{legitimate}'")),
            "a legitimate path containing a space still binds, as one word: {rendered}"
        );
    }

    #[test]
    fn file_shaped_denied_paths_mask_with_dev_null_and_dirs_with_tmpfs() {
        let scratch = Scratch::new("mask-shapes");
        let denied_file = scratch.path.join("secret.txt");
        std::fs::write(&denied_file, "known-secret-payload").expect("the file is writable");
        let denied_dir = scratch.path.join("secrets");
        std::fs::create_dir_all(&denied_dir).expect("the dir is creatable");
        let policy = contained_policy(
            Vec::new(),
            vec![
                denied_file.display().to_string(),
                denied_dir.display().to_string(),
            ],
        );
        let rendered = render_sandbox().render("true", &policy);
        assert!(
            rendered.contains(&format!("--ro-bind /dev/null {}", denied_file.display())),
            "an existing FILE is masked by a read-only /dev/null bind: {rendered}"
        );
        assert!(
            rendered.contains(&format!("--tmpfs {}", denied_dir.display())),
            "an existing DIRECTORY is masked by an empty tmpfs: {rendered}"
        );
    }

    /// The rendered MOUNT ORDER is the whole of the nesting guarantee.
    /// bubblewrap applies mount operations in argument order, so a mask
    /// that precedes the writable bind containing it is undone by that
    /// bind. Every denied path — nested or sibling, dir-shaped or
    /// file-shaped — must therefore appear AFTER the last writable bind.
    #[test]
    fn every_mask_is_rendered_after_the_last_writable_bind() {
        let scratch = Scratch::new("mask-order");
        let workspace = scratch.path.join("ws");
        std::fs::create_dir_all(workspace.join(".git")).expect("the nested dir is creatable");
        let nested_file = workspace.join("nested-secret.txt");
        std::fs::write(&nested_file, "known-secret-payload").expect("the file is writable");
        let sibling = scratch.path.join("sibling-denied");
        std::fs::create_dir_all(&sibling).expect("the sibling dir is creatable");

        let policy = contained_policy(
            vec![workspace.display().to_string()],
            vec![
                workspace.join(".git").display().to_string(),
                nested_file.display().to_string(),
                sibling.display().to_string(),
            ],
        );
        let rendered = render_sandbox().render("true", &policy);

        let bind = format!("--bind {ws} {ws}", ws = workspace.display());
        let last_bind = rendered
            .rfind(&bind)
            .unwrap_or_else(|| panic!("the writable root is bound back: {rendered}"));
        for mask in [
            format!("--tmpfs {}", workspace.join(".git").display()),
            format!("--ro-bind /dev/null {}", nested_file.display()),
            format!("--tmpfs {}", sibling.display()),
        ] {
            let at = rendered
                .find(&mask)
                .unwrap_or_else(|| panic!("the mask {mask:?} is rendered: {rendered}"));
            assert!(
                at > last_bind,
                "the mask {mask:?} must be rendered AFTER the writable bind {bind:?} — \
                 bubblewrap applies mounts in argument order, so an earlier mask is \
                 re-exposed by a later bind: {rendered}"
            );
        }
    }

    /// The LIVE nested-denial evidence: a denied path INSIDE the writable
    /// root must be unreadable and unwritable, and the host original must
    /// survive untouched. This is the case in which the port's documented
    /// "denied even if otherwise writable" has content — every other
    /// denied fixture in this crate is a SIBLING of the writable root, and
    /// a sibling is already covered by the read-only baseline root.
    #[test]
    fn live_confinement_denies_a_path_nested_inside_the_writable_root() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the nested denial test is skipped");
            return;
        };
        let scratch = Scratch::new("live-nested");
        let workspace = scratch.path.join("ws");
        let denied_dir = workspace.join(".git");
        std::fs::create_dir_all(&denied_dir).expect("the nested denied dir is creatable");
        let denied_dir_file = denied_dir.join("config");
        std::fs::write(&denied_dir_file, "known-secret-payload").expect("the secret is writable");
        let denied_file = workspace.join("nested-secret.txt");
        std::fs::write(&denied_file, "known-secret-payload").expect("the secret is writable");

        let policy = contained_policy(
            vec![workspace.display().to_string()],
            vec![
                denied_dir.display().to_string(),
                denied_file.display().to_string(),
            ],
        );
        let validation = sandbox
            .validate(&policy, live_bound())
            .expect("the nested policy validates clean");
        assert!(validation.is_clean());

        let run = |command: &str| {
            let wrapped = sandbox
                .wrap(command, &policy, &validation, live_bound())
                .expect("the wrap renders");
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&wrapped.command)
                .output()
                .expect("the wrapped command executes")
        };

        // A write inside the writable root but OUTSIDE any denied path
        // still succeeds — the fix must not close the writable root.
        let inside_marker = workspace.join("inside.txt");
        let outcome = run(&format!("echo inside > {}", inside_marker.display()));
        assert!(
            outcome.status.success(),
            "a write inside the writable root must still succeed: {}",
            String::from_utf8_lossy(&outcome.stderr)
        );

        for target in [&denied_dir_file, &denied_file] {
            let outcome = run(&format!("cat {}", target.display()));
            assert!(
                !String::from_utf8_lossy(&outcome.stdout).contains("known-secret-payload"),
                "a denied path nested in a writable root must never be readable: {}",
                target.display()
            );
            let _ = run(&format!("echo PWNED > {}", target.display()));
            assert_eq!(
                std::fs::read_to_string(target).expect("the host original survives"),
                "known-secret-payload",
                "a denied path nested in a writable root must never be writable: {}",
                target.display()
            );
        }
    }

    /// A process this test OWNS, killed and reaped on drop so a failing
    /// assertion cannot leave it behind. The process-space legs must never
    /// name a PID they did not create.
    struct Victim {
        child: std::process::Child,
    }

    impl Victim {
        fn spawn() -> Self {
            let child = Command::new("sleep")
                .arg(VICTIM_LIFETIME_SECS.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("the victim process is spawnable");
            Self { child }
        }

        fn pid(&self) -> u32 {
            self.child.id()
        }

        fn is_alive(&mut self) -> bool {
            matches!(self.child.try_wait(), Ok(None))
        }
    }

    impl Drop for Victim {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// The victim's wall lifetime, on its own line (no duration-shaped
    /// identifier shares a line with a numeric literal).
    const VICTIM_LIFETIME_SECS: u64 = 30;

    /// The LIVE process-space evidence: the baseline must confine the
    /// PROCESS space, not only the filesystem and the network. A read-only
    /// root and an unshared network say nothing about who the contained
    /// process may signal or observe — a sandbox that shares the host PID
    /// namespace lets the payload `SIGTERM` a host process and read what it
    /// was invoked with, neither of which the filesystem legs would notice.
    ///
    /// The victim is this test's OWN child ([`Victim`]), killed and reaped
    /// on drop; no host PID the test did not create is ever named, and the
    /// signal actually attempted is `SIGTERM` — the destructive one — so the
    /// leg proves the refusal rather than a permission check standing in for
    /// it.
    #[test]
    fn live_confinement_holds_the_host_process_space() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the process-space test is skipped");
            return;
        };
        let policy = contained_policy(Vec::new(), Vec::new());
        let validation = sandbox
            .validate(&policy, live_bound())
            .expect("the empty contained policy validates clean");
        assert!(validation.is_clean());
        assert!(
            validation.isolation >= Isolation::Process,
            "this backend claims the process rung; the case below is what that claim means"
        );

        let mut victim = Victim::spawn();
        let victim_pid = victim.pid();
        let run = |command: &str| {
            let wrapped = sandbox
                .wrap(command, &policy, &validation, live_bound())
                .expect("the wrap renders");
            let output = Command::new("sh")
                .arg("-c")
                .arg(&wrapped.command)
                .output()
                .expect("the wrapped command executes");
            String::from_utf8_lossy(&output.stdout).into_owned()
        };

        // The payload's own /proc must still work — the fix confines the
        // process space, it does not remove the mount the sandbox needs.
        assert!(
            run("test -r /proc/self/status && echo SELF_PROC_OK").contains("SELF_PROC_OK"),
            "the contained process must still read its OWN /proc"
        );

        // 1. Signalling a process outside the sandbox.
        let signalled = run(&format!(
            "kill -TERM {victim_pid} && echo SIGNAL_LANDED || echo SIGNAL_REFUSED"
        ));
        assert!(
            !signalled.contains("SIGNAL_LANDED"),
            "a contained process must not signal a process outside the sandbox (pid \
             {victim_pid}): {signalled:?}"
        );
        assert!(
            signalled.contains("SIGNAL_REFUSED"),
            "the signalling probe did not run inside the sandbox: {signalled:?}"
        );
        assert!(
            victim.is_alive(),
            "the host process the payload signalled must still be running"
        );

        // 2. Reading a host process's /proc entry.
        let observed = run(&format!("cat /proc/{victim_pid}/cmdline"));
        assert!(
            !observed.contains("sleep"),
            "a contained process must not read the /proc entry of a process outside the \
             sandbox (pid {victim_pid}): {observed:?}"
        );
    }

    /// The anchoring rule, stated over paths alone — including the dodges.
    /// The rule is not "reject nesting": it is "every directory between a
    /// denied path and the writable root enclosing it must be a mount
    /// point", and the only mount points a rendered command has are the
    /// bound writable roots and the masks themselves.
    #[test]
    fn only_a_relocatable_ancestor_is_a_finding() {
        let root = Path::new(&std::env::temp_dir()).join("anchor-probe");
        let ws = root.join("ws");
        let roots = vec![ws.display().to_string()];

        // Direct child: the mask IS the mount point — admitted.
        assert_eq!(
            relocatable_ancestor(&ws.join(".git").display().to_string(), &roots),
            None
        );
        // Sibling, outside every root: the read-only baseline holds it.
        assert_eq!(
            relocatable_ancestor(&root.join("elsewhere").display().to_string(), &roots),
            None
        );
        // One intervening ordinary directory: renameable — a finding
        // naming that directory.
        assert_eq!(
            relocatable_ancestor(&ws.join("proj").join(".git").display().to_string(), &roots),
            Some(ws.join("proj"))
        );
        // Deeper: the INNERMOST offense is named.
        assert_eq!(
            relocatable_ancestor(
                &ws.join("a").join("b").join(".git").display().to_string(),
                &roots
            ),
            Some(ws.join("a").join("b"))
        );
        // A dotted spelling of the same relocatable path must not dodge
        // the rule by failing to compare equal.
        assert_eq!(
            relocatable_ancestor(
                &ws.join("x")
                    .join("..")
                    .join("proj")
                    .join(".git")
                    .display()
                    .to_string(),
                &roots
            ),
            Some(ws.join("proj"))
        );
        // And a dotted spelling that resolves to a DIRECT child is
        // admitted — the rule judges the path, not its spelling.
        assert_eq!(
            relocatable_ancestor(
                &ws.join("proj")
                    .join("..")
                    .join(".git")
                    .display()
                    .to_string(),
                &roots
            ),
            None
        );
        // The intervening directory declared a writable root of its own is
        // bound, hence a mount point, hence unrenameable — admitted.
        let both = vec![
            ws.display().to_string(),
            ws.join("proj").display().to_string(),
        ];
        assert_eq!(
            relocatable_ancestor(&ws.join("proj").join(".git").display().to_string(), &both),
            None
        );
        // But one level deeper under that root is relocatable again.
        assert_eq!(
            relocatable_ancestor(
                &ws.join("proj")
                    .join("sub")
                    .join(".git")
                    .display()
                    .to_string(),
                &both
            ),
            Some(ws.join("proj").join("sub"))
        );
    }

    /// The RELOCATION attack, executed end to end against the live
    /// backend — two sessions of the IDENTICAL policy with a permitted
    /// write in between.
    ///
    /// Half one, the ADMITTED shape: a denied path DIRECTLY under the
    /// writable root. Its mask is a mount point inside the bound root, and
    /// the kernel refuses to rename, remove, or clobber a mount point
    /// (`EBUSY`), so session one cannot move the payload; session two of
    /// the same policy still finds it masked. This is the shape that keeps
    /// working, and this leg is the proof that it does.
    ///
    /// Half two, the REFUSED shape: a denied path reachable through an
    /// INTERVENING directory inside the writable root. Renaming that
    /// directory is a write the policy explicitly permits, and it carries
    /// the denied content out from under the mask — after which the next
    /// wrap masks a vacated path, bubblewrap CREATES that path inside the
    /// writable root, and the sandbox starts with the denial defeated. The
    /// policy must therefore be refused at `validate` AND at `wrap` (a
    /// forged clean validation must not bootstrap it), naming the
    /// intervening directory.
    #[test]
    fn a_nested_denial_survives_a_rename_between_sessions() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the relocation test is skipped");
            return;
        };
        let scratch = Scratch::new("live-relocation");
        let workspace = scratch.path.join("ws");
        // The ADMITTED shape: both denials are direct children of the root.
        let denied_dir = workspace.join(".git");
        std::fs::create_dir_all(&denied_dir).expect("the denied dir is creatable");
        let denied_dir_file = denied_dir.join("config");
        std::fs::write(&denied_dir_file, "known-secret-payload").expect("the secret is writable");
        let denied_file = workspace.join("nested-secret.txt");
        std::fs::write(&denied_file, "known-secret-payload").expect("the secret is writable");

        let policy = contained_policy(
            vec![workspace.display().to_string()],
            vec![
                denied_dir.display().to_string(),
                denied_file.display().to_string(),
            ],
        );
        let session = |command: &str| {
            // Each session re-validates and re-wraps from scratch: the
            // attack turns on what the SECOND validation sees.
            let validation = sandbox
                .validate(&policy, live_bound())
                .expect("the direct-child nested policy validates clean");
            assert!(validation.is_clean());
            let wrapped = sandbox
                .wrap(command, &policy, &validation, live_bound())
                .expect("the wrap renders");
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&wrapped.command)
                .output()
                .expect("the wrapped command executes")
        };

        // Session one: every relocation the contained process can attempt
        // against a mask that is a mount point.
        let moved_dir = workspace.join("git-moved");
        let moved_file = workspace.join("secret-moved.txt");
        for attack in [
            format!("mv {} {}", denied_dir.display(), moved_dir.display()),
            format!("mv {} {}", denied_file.display(), moved_file.display()),
            format!("rm -rf {}", denied_dir.display()),
            format!("rm -f {}", denied_file.display()),
        ] {
            let _ = session(&attack);
        }
        assert!(
            !moved_dir.exists() && !moved_file.exists(),
            "no denied path may be relocated out from under its own mask"
        );
        assert_eq!(
            std::fs::read_to_string(&denied_dir_file).expect("the denied dir survives session one"),
            "known-secret-payload"
        );
        assert_eq!(
            std::fs::read_to_string(&denied_file).expect("the denied file survives session one"),
            "known-secret-payload"
        );

        // Session two, the IDENTICAL policy: still denied, both ways.
        for target in [&denied_dir_file, &denied_file] {
            let outcome = session(&format!("cat {}", target.display()));
            assert!(
                !String::from_utf8_lossy(&outcome.stdout).contains("known-secret-payload"),
                "the second session must not read the denied payload at {}",
                target.display()
            );
            let _ = session(&format!("echo PWNED > {}", target.display()));
            assert_eq!(
                std::fs::read_to_string(target).expect("the host original survives session two"),
                "known-secret-payload",
                "the second session must not overwrite the denied payload at {}",
                target.display()
            );
        }

        // The REFUSED shape: an intervening directory between the writable
        // root and the denial. The contained process may rename it, so this
        // backend cannot hold the denial and must say so.
        let intervening = workspace.join("proj");
        let relocatable = intervening.join(".git");
        let relocatable_policy = contained_policy(
            vec![workspace.display().to_string()],
            vec![relocatable.display().to_string()],
        );
        match sandbox.validate(&relocatable_policy, live_bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|found| found.contains(&intervening.display().to_string())
                            && found.contains(&relocatable.display().to_string())),
                    "the refusal names the denied path and the intervening directory: \
                     {findings:?}"
                );
            }
            other => panic!("a relocatable nested denial must be refused, got {other:?}"),
        }
        // And a forged clean validation must not bootstrap the wrap.
        let forged = PolicyValidation {
            policy: relocatable_policy.reference.clone(),
            backend: relocatable_policy.backend.clone(),
            isolation: Isolation::Process,
            findings: Vec::new(),
        };
        match sandbox.wrap("true", &relocatable_policy, &forged, live_bound()) {
            Err(SandboxError::WrapFailed { detail }) => {
                assert!(
                    detail.contains(&intervening.display().to_string()),
                    "the wrap refusal names the intervening directory: {detail}"
                );
            }
            other => panic!("a forged clean validation must not wrap it, got {other:?}"),
        }

        // A denied path whose intervening directory IS itself a declared
        // writable root stays admitted: that directory is bound, so it is
        // a mount point too, and equally unrenameable.
        let both_roots = SandboxPolicy {
            writable_roots: vec![
                workspace.display().to_string(),
                intervening.display().to_string(),
            ],
            ..relocatable_policy.clone()
        };
        std::fs::create_dir_all(&relocatable).expect("the nested denied dir is creatable");
        assert!(
            sandbox
                .validate(&both_roots, live_bound())
                .expect("an intervening writable root is admitted")
                .is_clean()
        );
    }

    /// `validate` is a DRY RUN OF THE POLICY (spec §6.2), not only a probe
    /// of the backend. Each shape here validated CLEAN and wrapped `Ok`
    /// before this correction, then hard-aborted bubblewrap at process
    /// start — fail-closed, but surfacing as a raw spawn error AFTER the
    /// gate had durably recorded an admission, instead of escalating
    /// through the designed path. Each must now be a typed
    /// `ValidationFailed` carrying the backend's own complaint.
    #[test]
    fn validate_dry_runs_the_policys_own_binds_and_masks() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the policy dry-run test is skipped");
            return;
        };
        let scratch = Scratch::new("dry-run");
        let workspace = scratch.path.join("ws");
        std::fs::create_dir_all(&workspace).expect("the writable root is creatable");
        let real = scratch.path.join("real-target");
        std::fs::write(&real, "known-secret-payload").expect("the target is writable");

        let refusal_names =
            |policy: &SandboxPolicy, shape: &str| match sandbox.validate(policy, live_bound()) {
                Err(SandboxError::ValidationFailed { findings, .. }) => {
                    assert!(
                        findings
                            .iter()
                            .any(|found| found.contains("do not start this backend")
                                && found.contains("the backend reported")),
                        "the {shape} refusal carries the backend's own complaint: {findings:?}"
                    );
                }
                other => panic!("{shape} must be a validation finding, got {other:?}"),
            };

        // (1) A denied path that is a SYMLINK.
        let linked_denied = workspace.join("linked-secret");
        std::os::unix::fs::symlink(&real, &linked_denied).expect("the symlink is creatable");
        refusal_names(
            &contained_policy(
                vec![workspace.display().to_string()],
                vec![linked_denied.display().to_string()],
            ),
            "a symlinked denied path",
        );

        // (2) A writable root that is a SYMLINK.
        let linked_root = scratch.path.join("linked-root");
        std::os::unix::fs::symlink(&workspace, &linked_root).expect("the symlink is creatable");
        refusal_names(
            &contained_policy(vec![linked_root.display().to_string()], Vec::new()),
            "a symlinked writable root",
        );

        // (3) A writable root that does not exist at all — the same class:
        // a root that cannot be granted must not validate clean.
        refusal_names(
            &contained_policy(
                vec![scratch.path.join("no-such-root").display().to_string()],
                Vec::new(),
            ),
            "a missing writable root",
        );

        // And the third shape the falsifier executed — a NONEXISTENT denied
        // path OUTSIDE every writable root — is not an offense at all now:
        // it renders no mask (nothing is there, and the read-only baseline
        // stops the contained process creating it), so the policy starts
        // and the denial holds. A refusal here would be a FALSE one, and it
        // would make an ordinary credential-denying policy unusable on any
        // host that simply has no such directory.
        let absent = contained_policy(
            vec![workspace.display().to_string()],
            vec![
                scratch
                    .path
                    .join("no-such-credentials")
                    .display()
                    .to_string(),
            ],
        );
        let validation = sandbox
            .validate(&absent, live_bound())
            .expect("a denial of a path that cannot exist is enforceable, not an offense");
        assert!(validation.is_clean());
        let wrapped = sandbox
            .wrap("true", &absent, &validation, live_bound())
            .expect("the wrap renders");
        let outcome = std::process::Command::new("sh")
            .arg("-c")
            .arg(&wrapped.command)
            .output()
            .expect("the wrapped command executes");
        assert!(
            outcome.status.success(),
            "the policy must actually START — a validation that reports clean and then \
             aborts at spawn is the defect: {}",
            String::from_utf8_lossy(&outcome.stderr)
        );
        // The denial it names is still enforced: the read-only baseline
        // refuses to let the contained process create anything there.
        let target = scratch.path.join("no-such-credentials");
        let outcome = std::process::Command::new("sh")
            .arg("-c")
            .arg(
                &sandbox
                    .wrap(
                        &format!("mkdir -p {}", target.display()),
                        &absent,
                        &validation,
                        live_bound(),
                    )
                    .expect("the wrap renders")
                    .command,
            )
            .output()
            .expect("the wrapped command executes");
        assert!(
            !outcome.status.success() && !target.exists(),
            "the contained process must not be able to create the denied path"
        );
    }

    #[test]
    fn a_missing_binary_is_backend_unavailable_not_a_policy_opinion() {
        let scratch = Scratch::new("missing-binary");
        let sandbox = BwrapSandbox::new(scratch.path.join("no-such-backend"));
        match sandbox.validate(&contained_policy(Vec::new(), Vec::new()), live_bound()) {
            Err(SandboxError::BackendUnavailable { backend, detail }) => {
                assert_eq!(backend.as_str(), BACKEND_TOKEN);
                assert!(
                    detail.contains("spawning"),
                    "the refusal names the spawn failure: {detail}"
                );
            }
            other => panic!("a missing binary must be BackendUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn an_expired_deadline_refuses_without_spawning() {
        let scratch = Scratch::new("expired");
        let sandbox = BwrapSandbox::new(scratch.path.join("no-such-backend"));
        // The expiry check precedes the spawn, so even a missing binary
        // reports the deadline, proving nothing was spawned. Expiry is
        // strict (`now > at`), so the bound must be one second BEHIND the
        // read of now — a bound equal to now is still alive.
        let now = BwrapSandbox::epoch_now().expect("the wall clock reads");
        let expired = Deadline::at_epoch(EpochSeconds::new(now.get().saturating_sub(1)));
        match sandbox.validate(&contained_policy(Vec::new(), Vec::new()), expired) {
            Err(SandboxError::BackendUnavailable { detail, .. }) => {
                assert!(
                    detail.contains("no time left") && detail.contains("not spawned"),
                    "the refusal names the expired bound: {detail}"
                );
            }
            other => panic!("an expired deadline must be a typed refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_allowlist_posture_is_an_honest_validation_finding() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the allowlist refusal test is skipped");
            return;
        };
        let mut policy = contained_policy(Vec::new(), Vec::new());
        policy.network = NetworkPosture::Allowlist {
            hosts: vec!["example.test".to_owned()],
        };
        match sandbox.validate(&policy, live_bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("cannot enforce a host allowlist")),
                    "the refusal names the unenforceable posture: {findings:?}"
                );
            }
            other => panic!("an allowlist must be a finding, got {other:?}"),
        }
    }

    #[test]
    fn relative_roots_and_irreversible_tiers_are_refused() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the policy refusal test is skipped");
            return;
        };
        let relative = contained_policy(vec!["relative/root".to_owned()], Vec::new());
        match sandbox.validate(&relative, live_bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("not an absolute path")),
                    "the refusal names the relative root: {findings:?}"
                );
            }
            other => panic!("a relative root must be refused, got {other:?}"),
        }
        let mut irreversible = contained_policy(Vec::new(), Vec::new());
        irreversible.tier = BlastRadius::Irreversible;
        match sandbox.validate(&irreversible, live_bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("irreversible")),
                    "the refusal names the top tier: {findings:?}"
                );
            }
            other => panic!("the irreversible tier must be refused, got {other:?}"),
        }
    }

    /// The BACKEND's own bound on an unbounded writable root. A policy
    /// naming the filesystem root — or any ancestor of it — as writable
    /// asks this backend to render `--bind / /`, which hands the contained
    /// process the whole host read-write and leaves the sandbox with
    /// nothing to contain. Anchoring in the policy provider is the first
    /// line; this is the second, and it holds for any SandboxPolicy
    /// reaching the port however it was built.
    #[test]
    fn the_filesystem_root_as_a_writable_root_is_a_finding() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the unbounded-root refusal test is skipped");
            return;
        };
        let unbounded = contained_policy(vec!["/".to_owned()], Vec::new());
        match sandbox.validate(&unbounded, live_bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("filesystem root")),
                    "the refusal names the unbounded root: {findings:?}"
                );
            }
            other => {
                panic!("the filesystem root as a writable root must be refused, got {other:?}")
            }
        }
        // The dotted spelling of the same root must not slip past.
        let dotted = contained_policy(vec!["/tmp/..".to_owned()], Vec::new());
        match sandbox.validate(&dotted, live_bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("filesystem root")),
                    "the refusal names the dotted spelling of the root: {findings:?}"
                );
            }
            other => panic!("a dotted spelling of the root must be refused, got {other:?}"),
        }
    }

    #[test]
    fn wrap_gates_on_its_own_clean_process_rung_validation() {
        let Some(sandbox) = live_backend() else {
            eprintln!("bwrap is absent — the wrap gating test is skipped");
            return;
        };
        let policy = contained_policy(Vec::new(), Vec::new());
        let validation = sandbox
            .validate(&policy, live_bound())
            .expect("the well-formed policy validates");
        assert_eq!(validation.isolation, Isolation::Process);
        assert!(validation.is_clean());
        let wrapped = sandbox
            .wrap("echo hi", &policy, &validation, live_bound())
            .expect("a clean validation of an eligible policy wraps");
        assert!(
            wrapped
                .command
                .starts_with(&sandbox.binary.display().to_string())
        );
        // A foreign validation is refused.
        let mut other_policy = policy.clone();
        other_policy.reference = SandboxPolicyRef::new("tier-contained-other").expect("test ref");
        assert!(
            sandbox
                .wrap("echo hi", &other_policy, &validation, live_bound())
                .is_err()
        );
        // A findings-carrying validation is refused.
        let mut dirty = validation.clone();
        dirty.findings.push("forged finding".to_owned());
        assert!(
            sandbox
                .wrap("echo hi", &policy, &dirty, live_bound())
                .is_err()
        );
        // A downgraded rung is refused.
        let mut downgraded = validation.clone();
        downgraded.isolation = Isolation::None;
        assert!(
            sandbox
                .wrap("echo hi", &policy, &downgraded, live_bound())
                .is_err()
        );
        // A forged clean validation for an ineligible policy is refused.
        let mut irreversible = policy.clone();
        irreversible.tier = BlastRadius::Irreversible;
        let forged = PolicyValidation {
            policy: irreversible.reference.clone(),
            backend: irreversible.backend.clone(),
            isolation: Isolation::Process,
            findings: Vec::new(),
        };
        assert!(
            sandbox
                .wrap("echo hi", &irreversible, &forged, live_bound())
                .is_err()
        );
    }

    /// The LIVE confinement evidence (plan Task 14, the real-backend
    /// proof): one policy, one writable root, denied file + denied dir.
    /// (a) a write INSIDE the writable root succeeds; (b) a write to a
    /// sibling OUTSIDE the writable roots fails on the read-only root;
    /// (c) reading the denied FILE fails (the /dev/null mask denies); (d)
    /// the denied DIR's content is gone (the tmpfs mask empties it).
    #[test]
    fn live_confinement_holds_the_writable_root_and_masks_denied_paths() {
        let Some(sandbox) = live_backend() else {
            // Graceful only on genuine absence; on this host (Task 0:
            // bubblewrap 0.11.1) the test runs.
            eprintln!("bwrap is absent — the live confinement test is skipped");
            return;
        };
        let scratch = Scratch::new("live");
        let workspace = scratch.path.join("ws");
        std::fs::create_dir_all(&workspace).expect("the writable root is creatable");
        let sibling = scratch.path.join("sibling");
        std::fs::create_dir_all(&sibling).expect("the sibling dir is creatable");
        let denied_file = scratch.path.join("secret.txt");
        std::fs::write(&denied_file, "known-secret-payload").expect("the secret is writable");
        let denied_dir = scratch.path.join("secrets");
        std::fs::create_dir_all(&denied_dir).expect("the denied dir is creatable");
        std::fs::write(denied_dir.join("creds.txt"), "more-secret-payload")
            .expect("the denied file is writable");

        let policy = contained_policy(
            vec![workspace.display().to_string()],
            vec![
                denied_file.display().to_string(),
                denied_dir.display().to_string(),
            ],
        );
        let validation = sandbox
            .validate(&policy, live_bound())
            .expect("the live policy validates clean");
        assert!(validation.is_clean());
        assert_eq!(validation.isolation, Isolation::Process);

        let run = |command: &str| {
            let wrapped = sandbox
                .wrap(command, &policy, &validation, live_bound())
                .expect("the wrap renders");
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&wrapped.command)
                .output()
                .expect("the wrapped command executes")
        };

        // (a) inside the writable root: must succeed.
        let inside_marker = workspace.join("inside.txt");
        let outcome = run(&format!("echo inside > {}", inside_marker.display()));
        assert!(
            outcome.status.success(),
            "a write inside the writable root must succeed: {}",
            String::from_utf8_lossy(&outcome.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(&inside_marker).expect("the marker exists"),
            "inside\n"
        );

        // (b) outside the writable roots: must FAIL (read-only root).
        let outside_marker = sibling.join("outside.txt");
        let outcome = run(&format!("echo outside > {}", outside_marker.display()));
        assert!(
            !outcome.status.success(),
            "a write outside the writable roots must fail — the root is read-only"
        );
        assert!(
            !outside_marker.exists(),
            "no artifact may appear outside the writable roots"
        );

        // (c) the denied FILE: reading must fail or see nothing of the
        // payload (/dev/null mask).
        let outcome = run(&format!("cat {}", denied_file.display()));
        assert!(
            !outcome.status.success() || String::from_utf8_lossy(&outcome.stdout).trim().is_empty(),
            "reading the denied file must fail or see nothing"
        );
        assert!(
            !String::from_utf8_lossy(&outcome.stdout).contains("known-secret-payload"),
            "the denied payload must never be readable inside the sandbox"
        );

        // (d) the denied DIR: its content must be gone (tmpfs mask).
        let outcome = run(&format!(
            "test -s {}",
            denied_dir.join("creds.txt").display()
        ));
        assert!(
            !outcome.status.success(),
            "the denied directory's content must not exist inside the sandbox"
        );
        // The host-side originals are untouched: masking is a namespace
        // overlay, never a host mutation.
        assert_eq!(
            std::fs::read_to_string(&denied_file).expect("the host secret survives"),
            "known-secret-payload"
        );
        assert_eq!(
            std::fs::read_to_string(denied_dir.join("creds.txt"))
                .expect("the host denied file survives"),
            "more-secret-payload"
        );
    }

    #[test]
    fn the_shared_contract_suite_is_clean() {
        if locate().is_none() {
            // The factory would fail its build honestly; the suite treats
            // that as a finding. On a host without the backend the
            // suite-run is skipped rather than reported as a contract
            // violation of an adapter that cannot exist there.
            eprintln!("bwrap is absent — the suite run is skipped");
            return;
        }
        let findings = sandbox_suite(&BwrapSandboxFactory);
        assert!(
            findings.is_empty(),
            "the bwrap backend must run the shared sandbox suite clean:\n{}",
            findings
                .iter()
                .map(|finding| finding.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
