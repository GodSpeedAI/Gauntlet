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
//! AK-048) means the FIRST process is the sandbox. Baseline shape (the
//! Task-0 smoke): `--ro-bind / / --dev /dev --proc /proc --unshare-net
//! --die-with-parent` — the root filesystem read-only, an unshared
//! network namespace (deny-by-default), and no orphaned children. Writable
//! roots are bound back read-write and denied paths are masked AFTER them
//! — mounts apply in argument order, so the mask must come last to outrank
//! the bind that encloses it.
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

/// The baseline containment flags every probe and wrap share (Task-0
/// smoke shape): root read-only, dev and proc mounted, network unshared
/// (deny-by-default), children dying with the sandbox.
const BASELINE_ARGS: [&str; 9] = [
    "--ro-bind",
    "/",
    "/",
    "--dev",
    "/dev",
    "--proc",
    "/proc",
    "--unshare-net",
    "--die-with-parent",
];

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

    /// Probes that the backend is REAL and usable before any policy
    /// judgment: spawns the binary under the baseline containment shape
    /// running `true`, bounded by the deadline. A spawn failure, a nonzero
    /// exit, a signal death, or a deadline overrun is a typed
    /// [`SandboxError::BackendUnavailable`] — the requested backend cannot
    /// run here, and that fact precedes every policy opinion.
    fn probe_backend(&self, deadline: Deadline) -> Result<(), SandboxError> {
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
            .args(BASELINE_ARGS)
            .arg("true")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
                        // Reap so no zombie remains.
                        let _ = child.wait();
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
        match status {
            Some(status) if status.success() => Ok(()),
            Some(status) => Err(SandboxError::BackendUnavailable {
                backend,
                detail: format!(
                    "the backend probe at {} exited with {status} — the binary is not a usable \
                     sandbox here",
                    self.binary.display()
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
            }
        }
        findings
    }

    /// Renders the single shell-invocable command. Writable roots are
    /// bound back FIRST and denied paths are masked AFTER them: bubblewrap
    /// applies mount operations in ARGUMENT ORDER, so a later bind
    /// re-exposes what an earlier mask covered. The order is load-bearing,
    /// and it is the whole of the nesting guarantee — a denied path INSIDE
    /// a writable root is denied only because its mask outranks the bind
    /// enclosing it. Masking last costs the sibling case nothing (a
    /// sibling is masked either way, and is already outside every bind).
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
    ///   writable root the mount point is creatable, so the mask mounts
    ///   and the path is an empty tmpfs — denied either way; there is no
    ///   ordering in which a missing mask lets the process run.
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
        for root in &policy.writable_roots {
            words.push("--bind".to_owned());
            words.push(root.clone());
            words.push(root.clone());
        }
        for denied in &policy.denied_paths {
            if Path::new(denied).is_file() {
                words.push("--ro-bind".to_owned());
                words.push("/dev/null".to_owned());
                words.push(denied.clone());
            } else {
                words.push("--tmpfs".to_owned());
                words.push(denied.clone());
            }
        }
        words.push("--".to_owned());
        words.push("sh".to_owned());
        words.push("-c".to_owned());
        let mut rendered: Vec<String> = words.iter().map(|word| shell_word(word)).collect();
        rendered.push(format!("'{}'", single_quote_escape(command)));
        rendered.join(" ")
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
        let findings = Self::policy_findings(policy);
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
        let policy = contained_policy(vec!["/ws/root".to_owned()], vec!["/ws/denied".to_owned()]);
        let rendered = render_sandbox().render("echo hi", &policy);
        let prefix = format!(
            "{RENDER_BINARY} --ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent"
        );
        assert!(
            rendered.starts_with(&prefix),
            "the wrapped command starts with the backend invocation: {rendered}"
        );
        assert!(
            rendered.contains("--tmpfs /ws/denied"),
            "a directory-shaped denied path renders as a tmpfs mask: {rendered}"
        );
        assert!(
            rendered.contains("--bind /ws/root /ws/root"),
            "each writable root is bound back read-write: {rendered}"
        );
        assert!(
            rendered.ends_with("-- sh -c 'echo hi'"),
            "the command is the final single-quoted sh -c argument: {rendered}"
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
        let smuggler = "/ws/root --share-net";
        let policy = contained_policy(vec![smuggler.to_owned()], vec![smuggler.to_owned()]);
        let rendered = render_sandbox().render("true", &policy);
        assert!(
            !rendered.contains("--unshare-net --die-with-parent --tmpfs /ws/root --share-net"),
            "the smuggled flag must not stand as a word of its own: {rendered}"
        );
        assert!(
            rendered.contains("--tmpfs '/ws/root --share-net'"),
            "the hostile denied path is one quoted word: {rendered}"
        );
        assert!(
            rendered.contains("--bind '/ws/root --share-net' '/ws/root --share-net'"),
            "the hostile writable root is one quoted word on both sides: {rendered}"
        );

        let legitimate = "/home/some user/ws";
        let honest = contained_policy(vec![legitimate.to_owned()], Vec::new());
        let rendered = render_sandbox().render("true", &honest);
        assert!(
            rendered.contains("--bind '/home/some user/ws' '/home/some user/ws'"),
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
