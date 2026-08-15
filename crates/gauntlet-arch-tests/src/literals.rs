//! (c) Hardcoded-literal scan — spec §8.1: "No path, model, provider,
//! command, port number, timeout, tier policy, or tool identity is a literal
//! in domain or application code." AK-016 says the same list. Every class the
//! standard names now has a scanner; each one is documented below with its
//! rule and its false-positive posture, because a class whose posture is not
//! written down is a class that gets quietly weakened.
//!
//! | class | rule | false-positive posture |
//! |---|---|---|
//! | `hardcoded-absolute-path` | a `/`-rooted segment from [`ABSOLUTE_PATH_ROOTS`] at a non-path boundary, case-insensitive, trailing slash NOT required; plus any drive-letter root `[A-Za-z]:[\\/]` | flags absolute paths in comments too — naming a machine's layout in core prose is the same coupling |
//! | `hardcoded-url` | any `scheme://` (`wss`, `file`, `ftp`, `grpc`, …), not just http(s) | a `://` inside a non-URL string is vanishingly rare; accepted |
//! | `hardcoded-model-id` | a family from [`MODEL_FAMILIES`] at a word boundary followed by `-<alnum>`, or from [`CONCATENATED_FAMILIES`] followed by a digit | enumerated, so a family nobody listed is missed — see "residual" below |
//! | `hardcoded-network-endpoint` | IPv4, `[v6]:port`, and `host:port` for ARBITRARY hosts (any dotted name, or `localhost`) | a dotted token followed by `:digits` in prose flags; rare enough to accept |
//! | `hardcoded-port` | a numeric literal whose value is a well-known port, in any Rust spelling: `8080`, `8_080`, `0x1f90`, `0o17620`, `8080u16` | a well-known port value used as a plain number flags; deliberate — outside the defaults table it is a configuration leak either way |
//! | `hardcoded-command` | a multi-token string literal whose first token is an executable from [`COMMAND_HEADS`], with no English stopword and only command-shaped tokens; plus any `Command::new("…")` | scoped to STRING LITERALS only, so prose and identifiers never flag; a sentence beginning with a tool name is excluded by the stopword guard |
//! | `hardcoded-tool-identity` | a string literal that is exactly a product from [`TOOL_IDENTITIES`] | string literals only; the risk is a config PARSER matching on `"git"` — see "residual" |
//! | `hardcoded-timeout` | a numeric literal ≥ 2 introduced by `=`, `(` or `,` on a line carrying a duration-shaped identifier ([`TIMEOUT_HINTS`]) | the `=`/`(`/`,` requirement is what keeps `(spec §11.5)` and markdown tables from flagging |
//! | `hardcoded-tier-policy` | a string literal from [`TIER_TERMS`], or matching `tier[-_ ]<digit>`, on a `const`/`static`/`default` line that is not an attribute | narrow ON PURPOSE — see "residual" |
//!
//! # Residual, recorded rather than hidden
//!
//! Three of these classes cannot be made both complete and mechanical today:
//!
//! - **model id** is an enumerated-family match. A brand-new family name
//!   (`"acme-1"`) is not detectable without a rule like "any hyphenated
//!   lowercase token in a `*_MODEL` binding", which false-positives on
//!   ordinary kebab-case data. The list is widened whenever a family is met.
//! - **tier policy** is restricted to `const`/`static`/`default` lines. The
//!   complete §8.1 reading — *any* tier string literal — collides head-on
//!   with the domain's own `FromStr`/`serde` parsing of the same vocabulary
//!   (`match s { "contained" => … }`), which is typed vocabulary, not policy.
//!   Making it complete needs a real Rust parser (syn) so the class can ask
//!   "is this literal a *value* or a *pattern*"; that is a dependency Task 2
//!   does not permit. Recorded, not silently dropped.
//! - **tool identity** has the same shape of risk in a configuration parser.
//!   The narrow remedy, if it ever fires falsely, is to exclude `match`
//!   patterns — not to delete the class.
//!
//! Scope: the denylist scope (every regular text file under the four core
//! crates, including `gauntlet-cli`) minus
//! `crates/gauntlet-app/src/config/defaults.rs`, the declared §8.6 defaults
//! table. That exemption is itself checked (`crate::check_defaults_exemption`)
//! so it can never be vacuous. Matching runs over `crate::text::scan_units`,
//! so a literal split by a line-continuation or `concat!` is still found.

/// Model family prefixes for the hyphenated form.
pub const MODEL_FAMILIES: &[&str] = &[
    "gpt", "claude", "sonnet", "haiku", "opus", "glm", "gemini", "gemma", "grok", "qwen", "llama",
    "mistral", "mixtral", "deepseek", "kimi", "minimax", "moonshot", "nova", "phi", "o1", "o3",
    "o4",
];

/// Model family prefixes for the concatenated (hyphenless) form. Families
/// whose bare word plus a digit also occurs in ordinary prose (`o1`, `nova`,
/// `phi`, `opus`) stay reachable only through the hyphenated form.
pub const CONCATENATED_FAMILIES: &[&str] = &[
    "gpt", "claude", "sonnet", "haiku", "glm", "gemini", "gemma", "grok", "qwen", "llama",
    "mistral", "mixtral", "deepseek", "kimi",
];

/// Well-known service ports. Curated, and deliberately erring toward
/// flagging: outside the defaults module a well-known port literal is a
/// configuration leak even if the number could theoretically be something
/// else.
/// `22` is deliberately absent: a two-digit number is indistinguishable from
/// a spec cross-reference (`§22.3`) without parsing.
pub const WELL_KNOWN_PORTS: &[u64] = &[
    80, 443, 1080, 3000, 3306, 4317, 4318, 5000, 5173, 5432, 6379, 8000, 8080, 8443, 8888, 9000,
    9090, 9222, 27017,
];

/// Absolute-path roots that name a machine's own filesystem layout. Matched
/// case-insensitively, with NO trailing slash required, so `"/tmp"` flags.
pub const ABSOLUTE_PATH_ROOTS: &[&str] = &[
    "applications",
    "workspaces",
    "workspace",
    "cygdrive",
    "library",
    "private",
    "volumes",
    "system",
    "media",
    "users",
    "lib64",
    "sbin",
    "snap",
    "boot",
    "data",
    "home",
    "root",
    "proc",
    "srv",
    "mnt",
    "opt",
    "tmp",
    "usr",
    "var",
    "etc",
    "dev",
    "sys",
    "run",
    "nix",
    "bin",
    "lib",
];

/// Executables that head a command line. Used only against the FIRST token of
/// a string literal.
pub const COMMAND_HEADS: &[&str] = &[
    "git",
    "cargo",
    "rustc",
    "rustup",
    "clippy",
    "nextest",
    "npm",
    "npx",
    "pnpm",
    "yarn",
    "node",
    "deno",
    "bun",
    "python",
    "python3",
    "pip",
    "pipx",
    "poetry",
    "ruff",
    "pytest",
    "mvn",
    "gradle",
    "cmake",
    "make",
    "just",
    "bash",
    "sh",
    "zsh",
    "pwsh",
    "powershell",
    "docker",
    "podman",
    "kubectl",
    "helm",
    "terraform",
    "ssh",
    "scp",
    "rsync",
    "curl",
    "wget",
    "jq",
    "yq",
    "rg",
    "ripgrep",
    "grep",
    "sed",
    "awk",
    "tar",
    "gh",
    "glab",
    "gcloud",
    "psql",
    "sqlite3",
    "aider",
    "codex",
    "go",
    "java",
];

/// Product names that are a tool identity all by themselves (spec §8.4: the
/// runtime names capabilities, `GAUNTLET_TOOL_*`, never products). Matched as
/// a WHOLE string literal only.
pub const TOOL_IDENTITIES: &[&str] = &[
    "git",
    "cargo",
    "nextest",
    "clippy",
    "rustup",
    "npm",
    "npx",
    "pnpm",
    "yarn",
    "poetry",
    "pytest",
    "ripgrep",
    "rg",
    "jq",
    "yq",
    "kubectl",
    "helm",
    "terraform",
    "docker",
    "podman",
    "curl",
    "wget",
    "ssh",
    "rsync",
    "gh",
    "glab",
    "gcloud",
    "psql",
    "sqlite3",
    "aider",
    "codex",
    "gradle",
];

/// Identifier fragments that mark a number as a duration or schedule policy.
pub const TIMEOUT_HINTS: &[&str] = &[
    "timeout",
    "deadline",
    "interval",
    "duration",
    "wall_seconds",
    "wall-seconds",
    "_secs",
    "_seconds",
    "_ms",
    "_millis",
    "stall",
    "backoff",
    "retry_base",
    "retry_max",
    "poll",
    "ttl",
    "delay",
    "sleep",
    "expiry",
    "expires",
    "heartbeat",
    "grace_period",
];

/// Tier/authority/review policy vocabulary (spec §5.2 `BlastRadius`, §13.3
/// review modes, §17). `local` and `single` are deliberately absent: as bare
/// string literals they are far too common to carry meaning.
pub const TIER_TERMS: &[&str] = &[
    "contained",
    "cross_cutting",
    "cross-cutting",
    "irreversible",
    "adaptive",
    "adversarial",
    "blast_radius",
    "blast-radius",
];

/// Short English words that mark a string literal as prose rather than a
/// command line.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "is", "was", "are", "were", "be", "been", "not", "and", "or", "to", "of",
    "in", "on", "for", "with", "this", "that", "it", "must", "should", "may", "can", "cannot",
    "has", "have", "no", "at", "by", "from", "into",
];

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Bytes that make a `/` part of a longer relative path or identifier rather
/// than the start of an absolute one.
fn is_path_context_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'/')
}

/// Occurrence check for a [`TIMEOUT_HINTS`] fragment at an alphanumeric word
/// boundary. `_`-delimited fragments (`_secs`) carry their own boundary; bare
/// words must not be swallowed by a longer one — this is what keeps
/// `settlement` from matching `ttl` and `install` from matching `stall`.
fn contains_hint(lowered: &str, hint: &str) -> bool {
    let bytes = lowered.as_bytes();
    let hint_bytes = hint.as_bytes();
    let open_ended_start = !hint_bytes[0].is_ascii_alphanumeric();
    let open_ended_end = !hint_bytes[hint_bytes.len() - 1].is_ascii_alphanumeric();
    let mut from = 0usize;
    while let Some(found) = lowered[from..].find(hint) {
        let start = from + found;
        let end = start + hint.len();
        let before = open_ended_start || start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after = open_ended_end || end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before && after {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Scan one text file (any extension, whole) for every class. Reports the
/// first hit per class per scan unit — enough to fail loudly and to point at
/// the exact line. Duplicate messages (the same finding seen in both the
/// physical and the normalized form of a line) are collapsed.
pub fn scan_literals(path: &str, text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |message: String, out: &mut Vec<String>| {
        if !out.contains(&message) {
            out.push(message);
        }
    };

    for unit in crate::text::scan_units(text) {
        let line_no = unit.line;
        let raw = unit.text.as_str();
        let lowered = raw.to_lowercase();

        if let Some(found) = find_absolute_path(&lowered) {
            push(
                format!(
                    "hardcoded-absolute-path: {path}:{line_no}: '{found}' — absolute paths are \
                     configuration (spec §8.1), not code"
                ),
                &mut out,
            );
        }
        if let Some(drive) = find_drive_root(raw) {
            push(
                format!(
                    "hardcoded-absolute-path: {path}:{line_no}: '{drive}' — drive-letter paths \
                     are configuration (spec §8.1), not code"
                ),
                &mut out,
            );
        }
        if let Some(scheme) = find_url_scheme(&lowered) {
            push(
                format!(
                    "hardcoded-url: {path}:{line_no}: '{scheme}://' — URLs are configuration \
                     (spec §8.1), not code"
                ),
                &mut out,
            );
        }
        if let Some(model_id) = find_model_id(&lowered) {
            push(
                format!(
                    "hardcoded-model-id: {path}:{line_no}: '{model_id}' — model identifiers are \
                     configuration (spec §8.1), not code"
                ),
                &mut out,
            );
        }

        let endpoints = find_network_endpoints(&lowered);
        if let Some((_, _, endpoint)) = endpoints.first() {
            push(
                format!(
                    "hardcoded-network-endpoint: {path}:{line_no}: '{endpoint}' — network \
                     endpoints are configuration (spec §8.1), not code"
                ),
                &mut out,
            );
        }

        let numbers = numeric_literals(&lowered);
        if let Some(port) = find_well_known_port(&numbers, &endpoints) {
            push(
                format!(
                    "hardcoded-port: {path}:{line_no}: '{port}' — well-known service ports are \
                     configuration (spec §8.1), not code"
                ),
                &mut out,
            );
        }
        if let Some((value, hint)) = find_timeout(&lowered, &numbers) {
            push(
                format!(
                    "hardcoded-timeout: {path}:{line_no}: '{value}' next to '{hint}' — timeouts \
                     and intervals are configuration (spec §8.1, §8.6), not code"
                ),
                &mut out,
            );
        }

        let literals = string_literals(raw);
        if let Some(command) = find_command(&lowered, &literals) {
            push(
                format!(
                    "hardcoded-command: {path}:{line_no}: '{command}' — commands are \
                     configuration (spec §8.1), not code"
                ),
                &mut out,
            );
        }
        if let Some(tool) = find_tool_identity(&literals) {
            push(
                format!(
                    "hardcoded-tool-identity: {path}:{line_no}: '{tool}' — the runtime names tool \
                     CAPABILITIES (spec §8.4), never tool products"
                ),
                &mut out,
            );
        }
        if let Some(tier) = find_tier_policy(&lowered, &literals) {
            push(
                format!(
                    "hardcoded-tier-policy: {path}:{line_no}: '{tier}' — tier and review policy \
                     is configuration (spec §8.1, §17), not code"
                ),
                &mut out,
            );
        }
    }
    out
}

/// Match an absolute path root on a lowercased line. The root need not be
/// followed by `/` (`"/tmp"` is as absolute as `"/tmp/x"`), and it must not be
/// the tail of a relative path (`docs/dev/x` does not match `/dev`).
fn find_absolute_path(lowered: &str) -> Option<String> {
    let bytes = lowered.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] != b'/' {
            continue;
        }
        if i > 0 && is_path_context_byte(bytes[i - 1]) {
            continue;
        }
        for root in ABSOLUTE_PATH_ROOTS {
            if !lowered[i + 1..].starts_with(root) {
                continue;
            }
            let after = i + 1 + root.len();
            let bounded = match bytes.get(after) {
                None => true,
                Some(b) => !(b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-'),
            };
            if bounded {
                return Some(format!("/{root}"));
            }
        }
    }
    None
}

/// Match a Windows drive-letter root `[A-Za-z]:[\\/]` at an ASCII word
/// boundary, so `https://` and `serde::fmt` never match.
fn find_drive_root(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        if !bytes[i].is_ascii_alphabetic() {
            continue;
        }
        if i > 0 && is_word_byte(bytes[i - 1]) {
            continue;
        }
        if bytes.get(i + 1) == Some(&b':') && matches!(bytes.get(i + 2), Some(b'\\') | Some(b'/')) {
            let mut drive = String::new();
            drive.push(bytes[i] as char);
            drive.push(':');
            drive.push(bytes[i + 2] as char);
            return Some(drive);
        }
    }
    None
}

/// Match any `scheme://` on a lowercased line (recritique R32): `wss://`,
/// `file://`, `ftp://`, `grpc://` are URLs too.
fn find_url_scheme(lowered: &str) -> Option<String> {
    let bytes = lowered.as_bytes();
    let mut from = 0usize;
    while let Some(found) = lowered[from..].find("://") {
        let at = from + found;
        from = at + 3;
        let mut start = at;
        while start > 0 {
            let b = bytes[start - 1];
            if b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-') {
                start -= 1;
            } else {
                break;
            }
        }
        if start < at && bytes[start].is_ascii_alphabetic() {
            return Some(lowered[start..at].to_string());
        }
    }
    None
}

/// Match the model-id pattern on a lowercased line, anchored at an ASCII word
/// boundary.
fn find_model_id(lowered: &str) -> Option<String> {
    let bytes = lowered.as_bytes();
    for i in 0..bytes.len() {
        if !bytes[i].is_ascii() {
            continue;
        }
        if i > 0 && is_word_byte(bytes[i - 1]) {
            continue;
        }
        for family in MODEL_FAMILIES {
            if !lowered[i..].starts_with(family) {
                continue;
            }
            let after_family = i + family.len();
            if CONCATENATED_FAMILIES.contains(family)
                && bytes.get(after_family).is_some_and(|b| b.is_ascii_digit())
            {
                let end = consume_id_tail(bytes, after_family + 1);
                return Some(trim_id_tail(lowered[i..end].to_string()));
            }
            let mut end = after_family + 1;
            if end >= bytes.len() || bytes[after_family] != b'-' {
                continue;
            }
            if !bytes[end].is_ascii_alphanumeric() {
                continue;
            }
            end += 1;
            end = consume_id_tail(bytes, end);
            return Some(trim_id_tail(lowered[i..end].to_string()));
        }
    }
    None
}

fn consume_id_tail(bytes: &[u8], mut end: usize) -> usize {
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.' || bytes[end] == b'-')
    {
        end += 1;
    }
    end
}

fn trim_id_tail(mut id: String) -> String {
    while id.ends_with('.') || id.ends_with('-') {
        id.pop();
    }
    id
}

/// One numeric literal in any Rust spelling.
#[derive(Clone, Debug)]
struct NumericLiteral {
    start: usize,
    text: String,
    value: Option<u64>,
    /// The nearest non-space byte before the literal, if any.
    introducer: Option<u8>,
}

/// Every numeric literal on a lowercased line, handling `_` separators, the
/// `0x`/`0o`/`0b` radix prefixes, and Rust integer type suffixes
/// (recritique R22).
fn numeric_literals(lowered: &str) -> Vec<NumericLiteral> {
    const SUFFIXES: &[&str] = &[
        "usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "f64", "f32",
        "u8", "i8",
    ];
    let bytes = lowered.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() || (i > 0 && is_word_byte(bytes[i - 1])) {
            i += 1;
            continue;
        }
        let start = i;
        let (radix, mut cursor) = match (bytes.get(i), bytes.get(i + 1)) {
            (Some(b'0'), Some(b'x')) => (16u32, i + 2),
            (Some(b'0'), Some(b'o')) => (8u32, i + 2),
            (Some(b'0'), Some(b'b')) => (2u32, i + 2),
            _ => (10u32, i),
        };
        let digits_start = cursor;
        while cursor < bytes.len() {
            let b = bytes[cursor];
            if b == b'_' || (b as char).is_digit(radix) {
                cursor += 1;
            } else {
                break;
            }
        }
        if cursor == digits_start {
            i = start + 1;
            continue;
        }
        let digits: String = lowered[digits_start..cursor].replace('_', "");
        let mut end = cursor;
        for suffix in SUFFIXES {
            if lowered[end..].starts_with(suffix) {
                end += suffix.len();
                break;
            }
        }
        let bounded = end == bytes.len() || !is_word_byte(bytes[end]);
        if bounded {
            let introducer = lowered[..start]
                .bytes()
                .rev()
                .find(|b| !matches!(b, b' ' | b'\t'));
            out.push(NumericLiteral {
                start,
                text: lowered[start..end].to_string(),
                value: u64::from_str_radix(&digits, radix).ok(),
                introducer,
            });
        }
        i = end.max(start + 1);
    }
    out
}

/// A well-known port literal that is not already reported as part of an
/// endpoint on the same line.
fn find_well_known_port(
    numbers: &[NumericLiteral],
    endpoints: &[(usize, usize, String)],
) -> Option<String> {
    numbers
        .iter()
        .find(|n| {
            n.value.is_some_and(|v| WELL_KNOWN_PORTS.contains(&v))
                && !endpoints
                    .iter()
                    .any(|(s, e, _)| n.start >= *s && n.start < *e)
        })
        .map(|n| n.text.clone())
}

/// Every network endpoint on a lowercased line, as `(start, end, text)`:
/// `host:port` for ARBITRARY hosts (recritique R23), `[v6]:port`, and bare
/// IPv4 literals.
fn find_network_endpoints(lowered: &str) -> Vec<(usize, usize, String)> {
    let bytes = lowered.as_bytes();
    let mut out = Vec::new();

    // host:port — the host is any dotted name or `localhost`.
    let mut i = 0usize;
    while i < bytes.len() {
        if !(bytes[i].is_ascii_alphanumeric()) || (i > 0 && is_host_byte(bytes[i - 1])) {
            i += 1;
            continue;
        }
        let start = i;
        let mut cursor = i;
        while cursor < bytes.len() && is_host_byte(bytes[cursor]) {
            cursor += 1;
        }
        let host = &lowered[start..cursor];
        if bytes.get(cursor) == Some(&b':') {
            let port_start = cursor + 1;
            let mut port_end = port_start;
            while port_end < bytes.len() && bytes[port_end].is_ascii_digit() {
                port_end += 1;
            }
            let has_port = port_end > port_start && port_end - port_start <= 5;
            let plausible_host = host == "localhost"
                || (host.contains('.') && !host.starts_with('.') && !host.ends_with('.'));
            if has_port && plausible_host {
                out.push((start, port_end, lowered[start..port_end].to_string()));
                i = port_end;
                continue;
            }
        }
        i = cursor.max(start + 1);
    }

    // [v6]:port
    let mut from = 0usize;
    while let Some(found) = lowered[from..].find('[') {
        let open = from + found;
        from = open + 1;
        let Some(close_rel) = lowered[open..].find(']') else {
            break;
        };
        let close = open + close_rel;
        let inner = &lowered[open + 1..close];
        if inner.is_empty()
            || !inner.contains(':')
            || !inner.bytes().all(|b| b.is_ascii_hexdigit() || b == b':')
        {
            continue;
        }
        if bytes.get(close + 1) != Some(&b':') {
            continue;
        }
        let mut port_end = close + 2;
        while port_end < bytes.len() && bytes[port_end].is_ascii_digit() {
            port_end += 1;
        }
        if port_end > close + 2 {
            out.push((open, port_end, lowered[open..port_end].to_string()));
        }
    }

    if out.is_empty()
        && let Some(ipv4) = find_ipv4(lowered)
    {
        let at = lowered.find(&ipv4).unwrap_or(0);
        out.push((at, at + ipv4.len(), ipv4));
    }
    out.sort_by_key(|(start, _, _)| *start);
    out
}

fn is_host_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')
}

/// Match an IPv4 literal that is not itself part of a longer dotted or digit
/// sequence.
fn find_ipv4(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if !bytes[i].is_ascii_digit() {
            continue;
        }
        if i > 0 && (bytes[i - 1].is_ascii_digit() || bytes[i - 1] == b'.') {
            continue;
        }
        let Some(end) = match_ipv4_at(bytes, i) else {
            continue;
        };
        if end == bytes.len() || !(bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            return Some(s[i..end].to_string());
        }
    }
    None
}

fn match_ipv4_at(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    for group in 0..4 {
        if group > 0 {
            if pos >= bytes.len() || bytes[pos] != b'.' {
                return None;
            }
            pos += 1;
        }
        let group_start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() && pos - group_start < 3 {
            pos += 1;
        }
        if pos == group_start {
            return None;
        }
    }
    Some(pos)
}

/// Every double-quoted string literal on a line, escapes resolved enough for
/// matching. Raw strings (`r"…"`) are read as ordinary ones, which is
/// sufficient: their content still ends up in the extracted text.
fn string_literals(raw: &str) -> Vec<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        i += 1;
        let mut current = String::new();
        while i < bytes.len() {
            match bytes[i] {
                b'"' => break,
                b'\\' if i + 1 < bytes.len() => {
                    current.push(bytes[i + 1] as char);
                    i += 2;
                }
                other => {
                    current.push(other as char);
                    i += 1;
                }
            }
        }
        i += 1;
        out.push(current);
    }
    out
}

/// A hardcoded command: `Command::new("…")`, or a command-shaped string
/// literal headed by a known executable.
fn find_command(lowered: &str, literals: &[String]) -> Option<String> {
    if lowered.contains("command::new(")
        && let Some(first) = literals.first()
        && !first.is_empty()
    {
        return Some(first.clone());
    }
    for literal in literals {
        let tokens: Vec<&str> = literal.split_whitespace().collect();
        if tokens.len() < 2 || tokens.len() > 12 {
            continue;
        }
        let head = tokens[0]
            .rsplit('/')
            .next()
            .unwrap_or(tokens[0])
            .to_lowercase();
        if !COMMAND_HEADS.contains(&head.as_str()) {
            continue;
        }
        if tokens
            .iter()
            .any(|t| STOPWORDS.contains(&t.to_lowercase().as_str()))
        {
            continue;
        }
        if !tokens.iter().all(|t| {
            t.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.:/=+@%-".contains(&b))
        }) {
            continue;
        }
        return Some(literal.clone());
    }
    None
}

/// A string literal that is exactly a tool product name.
fn find_tool_identity(literals: &[String]) -> Option<String> {
    literals
        .iter()
        .find(|literal| TOOL_IDENTITIES.contains(&literal.trim().to_lowercase().as_str()))
        .cloned()
}

/// A duration/schedule policy number: a literal ≥ 2 introduced by `=`, `(`,
/// or `,` on a line carrying a duration-shaped identifier.
fn find_timeout(lowered: &str, numbers: &[NumericLiteral]) -> Option<(String, String)> {
    let hint = TIMEOUT_HINTS
        .iter()
        .find(|hint| contains_hint(lowered, hint))?
        .to_string();
    let number = numbers.iter().find(|n| {
        n.value.is_some_and(|v| v >= 2)
            && matches!(n.introducer, Some(b'=') | Some(b'(') | Some(b','))
    })?;
    Some((number.text.clone(), hint))
}

/// A tier/authority/review policy literal on a declaration line.
fn find_tier_policy(lowered: &str, literals: &[String]) -> Option<String> {
    if lowered.contains("#[") {
        return None;
    }
    if !(lowered.contains("const ") || lowered.contains("static ") || lowered.contains("default")) {
        return None;
    }
    for literal in literals {
        let value = literal.trim().to_lowercase();
        if TIER_TERMS.contains(&value.as_str()) {
            return Some(literal.clone());
        }
        if let Some(rest) = value.strip_prefix("tier")
            && rest
                .strip_prefix(['-', '_', ' '])
                .is_some_and(|d| d.bytes().next().is_some_and(|b| b.is_ascii_digit()))
        {
            return Some(literal.clone());
        }
    }
    None
}

#[cfg(test)]
mod repo {

    #[test]
    fn core_crates_contain_no_hardcoded_literals() {
        let violations = crate::check_no_hardcoded_literals(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("hardcoded-literal", &violations)
        );
    }

    #[test]
    fn defaults_exemption_is_warranted_or_inactive() {
        let violations = crate::check_defaults_exemption(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("vacuous-exemption", &violations)
        );
    }
}

#[cfg(test)]
mod scanner_tests {
    use super::*;

    #[test]
    fn absolute_paths_are_flagged() {
        for fixture in [
            "let p = \"/home/alice/notes\";",
            "let p = \"/Users/bob/notes\";",
            "let p = \"/root/secrets\";",
            "let p = \"/etc/passwd\";",
            "let p = \"/var/log/gauntlet\";",
            "let p = \"/tmp/run.sock\";",
            "let p = \"C:\\\\tools\\\\bin\";",
            "let p = \"d:\\\\data\";",
            "let p = \"D:/x\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert_eq!(violations.len(), 1, "{fixture} -> {violations:?}");
            assert!(
                violations[0].starts_with("hardcoded-absolute-path: f.rs:1:"),
                "{}",
                violations[0]
            );
        }
    }

    #[test]
    fn absolute_paths_without_a_trailing_slash_and_new_roots_are_flagged() {
        // Recritique R32's exact evasions.
        for fixture in [
            "let dir = \"/tmp\";",
            "let dir = \"/workspace/repo\";",
            "let dir = \"/dev/shm/gauntlet.sock\";",
            "let dir = \"/proc/self/cwd\";",
            "let dir = \"/HOME/alice\";",
            "let dir = \"/srv/state\";",
            "let dir = \"/sys/fs/cgroup\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-absolute-path: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn relative_unix_paths_are_not_flagged() {
        assert!(scan_literals("f.rs", "let p = \"config/defaults.toml\";").is_empty());
        assert!(scan_literals("f.rs", "let p = \"notes/gauntlet.md\";").is_empty());
        assert!(scan_literals("f.rs", "let p = \"docs/dev/setup.md\";").is_empty());
        assert!(scan_literals("f.rs", "let p = \"a/tmpfile\";").is_empty());
    }

    #[test]
    fn urls_of_any_scheme_are_flagged() {
        let violations = scan_literals("f.rs", "const DOC: &str = \"HTTPS://example.invalid/x\";");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].starts_with("hardcoded-url: f.rs:1:"));

        // Recritique R32: non-http schemes are URLs too.
        for fixture in [
            "pub const EP: &str = \"wss://api.example.com/v1\";",
            "pub const EP: &str = \"file:///opt/x\";",
            "pub const EP: &str = \"ftp://mirror.example\";",
            "pub const EP: &str = \"grpc://collector\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-url: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn model_ids_are_flagged_and_bare_words_are_not() {
        for fixture in [
            "let model = \"gpt-4o\";",
            "let model = \"claude-3-5-sonnet\";",
            "let model = \"GLM-4.5\";",
            "let model = \"o3-mini\";",
            "let model = \"deepseek-v3\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert_eq!(violations.len(), 1, "{fixture} -> {violations:?}");
            assert!(
                violations[0].starts_with("hardcoded-model-id: f.rs:1:"),
                "{}",
                violations[0]
            );
        }
        for fixture in [
            "let model = \"glm4\";",
            "let model = \"qwen3\";",
            "let model = \"llama4\";",
            "let model = \"gpt5\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert_eq!(violations.len(), 1, "{fixture} -> {violations:?}");
            assert!(
                violations[0].starts_with("hardcoded-model-id: f.rs:1:"),
                "{}",
                violations[0]
            );
        }
        assert!(scan_literals("f.rs", "let v = \"gp42\";").is_empty());
        assert!(scan_literals("f.rs", "let llama_count = llama_count + 1;").is_empty());
        assert!(scan_literals("f.rs", "let port = \"o4\";").is_empty());
    }

    #[test]
    fn model_families_outside_the_original_list_are_flagged() {
        // Recritique R9's exact evasion.
        for fixture in [
            "pub const BUILDER_MODEL: &str = \"sonnet-4-5\";",
            "pub const CRITIC_MODEL: &str = \"kimi-k2-0905\";",
            "pub const CHEAP_MODEL: &str = \"phi-4\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-model-id: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn network_endpoints_are_flagged() {
        for fixture in [
            "let a = \"192.168.0.10\";",
            "let b = \"localhost:8080\";",
            "let c = \"0.0.0.0:9000\";",
            "let d = \"127.0.0.1:5432\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert_eq!(violations.len(), 1, "{fixture} -> {violations:?}");
            assert!(
                violations[0].starts_with("hardcoded-network-endpoint: f.rs:1:"),
                "{}",
                violations[0]
            );
        }
    }

    #[test]
    fn arbitrary_host_port_endpoints_are_flagged() {
        // Recritique R23's exact evasions.
        for fixture in [
            "pub const API_BIND: &str = \"api.internal:8443\";",
            "pub const DB: &str = \"db.prod:5432\";",
            "pub const V6: &str = \"[::1]:8080\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-network-endpoint: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn well_known_port_literals_are_flagged() {
        for fixture in [
            "let port: u16 = 8080;",
            "// port 8080",
            "let p = 443;",
            "let p = 5432;",
            "let p = 9222;",
            "let p = 27017;",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert_eq!(violations.len(), 1, "{fixture} -> {violations:?}");
            assert!(
                violations[0].starts_with("hardcoded-port: f.rs:1:"),
                "{}",
                violations[0]
            );
        }
    }

    #[test]
    fn ports_in_every_rust_spelling_are_flagged() {
        // Recritique R22's exact evasions.
        for fixture in [
            "const API_PORT: u16 = 8_080;",
            "const API_PORT: u16 = 0x1F90;",
            "const API_PORT: u16 = 0o17620;",
            "const API_PORT: u16 = 8080u16;",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-port: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn commands_and_tool_identities_are_flagged() {
        // Recritique R7/R18/R29's exact evasions.
        for (fixture, prefix) in [
            (
                "pub const AGENT_CMD: &str = \"npx --yes agent-runner --print\";",
                "hardcoded-command",
            ),
            (
                "pub const TEST_CMD: &str = \"cargo nextest run\";",
                "hardcoded-command",
            ),
            (
                "pub fn builder() -> Command { std::process::Command::new(\"git\") }",
                "hardcoded-command",
            ),
            (
                "pub const TOOL: &str = \"ripgrep\";",
                "hardcoded-tool-identity",
            ),
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations.iter().any(|v| v.starts_with(prefix)),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn prose_and_identifiers_are_not_commands() {
        // The class is scoped to string literals, and the stopword guard
        // keeps ordinary sentences out.
        assert!(scan_literals("f.rs", "let e = \"git is not available\";").is_empty());
        assert!(scan_literals("f.rs", "// run cargo nextest before review").is_empty());
        assert!(scan_literals("f.rs", "fn cargo_metadata() -> Metadata { todo!() }").is_empty());
    }

    #[test]
    fn timeouts_are_flagged() {
        // Recritique R7/R18/R29's exact evasions.
        for fixture in [
            "pub const BUILDER_TIMEOUT_SECS: u64 = 5400;",
            "pub const POLL_INTERVAL_MS: u64 = 250;",
            "pub const DEADLINE_SECS: u64 = 3600;",
            "pub const STALL_MS: u64 = 900_000;",
            "let d = Duration::from_secs(3600);",
            // Recritique R18: the previous unit test asserted this produced
            // NO flag, entrenching the gap — 3600 is the spec §8.6
            // `max_wall_seconds` default compiled into the application layer.
            "let ttl = 3600;",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-timeout: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn prose_with_a_duration_word_is_not_a_timeout() {
        // The `=`/`(`/`,` introducer requirement is what keeps spec
        // cross-references and markdown tables out.
        assert!(scan_literals("f.rs", "//! Retry and stall policy (spec §11.5).").is_empty());
        assert!(scan_literals("f.rs", "//! | max_wall_seconds | 3600 | bounded |").is_empty());
        assert!(scan_literals("f.rs", "let poll = configured_interval;").is_empty());
        // Hint fragments match at a word boundary, so `settlement` is not
        // `ttl` and `install` is not `stall`.
        assert!(scan_literals("f.rs", "let settlement = Settlement::new(8);").is_empty());
        assert!(scan_literals("f.rs", "let installed = install_count(9);").is_empty());
    }

    #[test]
    fn tier_policy_literals_are_flagged() {
        // Recritique R7/R29's exact evasions.
        for fixture in [
            "pub const TIER_POLICY: &str = \"irreversible\";",
            "pub const TIER: &str = \"tier-2\";",
            "pub const MODE: &str = \"adaptive\";",
            "static DEFAULT_RADIUS: &str = \"cross_cutting\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-tier-policy: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
        // Typed vocabulary is not a policy literal: a parser and a serde
        // attribute both stay clean (documented narrowing).
        assert!(
            scan_literals(
                "f.rs",
                "match s { \"contained\" => BlastRadius::Contained, _ => return Err(e) }"
            )
            .is_empty()
        );
        assert!(scan_literals("f.rs", "#[serde(rename = \"cross_cutting\")]").is_empty());
    }

    #[test]
    fn line_continuations_and_concat_fragments_are_flagged() {
        // Recritique R6's exact evasion, literal side.
        let split_model = "pub const MODEL: &str = \"claud\\\n    e-3-5-sonnet\";\n";
        let violations = scan_literals("f.rs", split_model);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-model-id: f.rs:1:")),
            "{violations:?}"
        );

        let split_path = "pub const ROOT: &str = \"/hom\\\n    e/agent/state\";\n";
        let violations = scan_literals("f.rs", split_path);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-absolute-path: f.rs:1:")),
            "{violations:?}"
        );
    }

    #[test]
    fn ordinary_numbers_and_localhost_prose_are_not_flagged() {
        assert!(scan_literals("f.rs", "let kib = 1024;").is_empty());
        assert!(scan_literals("f.rs", "let max = 65535;").is_empty());
        assert!(scan_literals("f.rs", "let ver = \"0.9.12+spec\";").is_empty());
        assert!(scan_literals("f.rs", "// run against localhost in the docs").is_empty());
        assert!(
            scan_literals("f.rs", "let octets = [127, 0, 0, 1];").is_empty(),
            "comma-separated octets are not an IPv4 literal"
        );
    }

    #[test]
    fn non_rust_text_files_are_scanned() {
        let json = scan_literals("f/wiring.json", "{\"model\": \"glm4\"}\n");
        assert_eq!(json.len(), 1, "{json:?}");
        assert!(json[0].starts_with("hardcoded-model-id: f/wiring.json:1:"));

        let txt = scan_literals("f/notes.txt", "serve on 0.0.0.0:9000\n");
        assert_eq!(txt.len(), 1, "{txt:?}");
        assert!(txt[0].starts_with("hardcoded-network-endpoint: f/notes.txt:1:"));
    }

    #[test]
    fn clean_source_produces_no_flags() {
        let fixture = "\
//! Configuration precedence: defaults, then file, then env, then flags.
pub fn resolve(defaults: &Defaults, env: &Env) -> Effective {
    Effective::merge(defaults, env) // no environment knowledge compiled in
}
";
        assert!(scan_literals("clean.rs", fixture).is_empty());
    }
}
