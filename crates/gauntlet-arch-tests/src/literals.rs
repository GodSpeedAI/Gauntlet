//! (c) Hardcoded-literal scan — spec §8.1: "No path, model, provider,
//! command, port number, timeout, tier policy, or tool identity is a literal
//! in domain or application code." AK-016 says the same list. Every class the
//! standard names now has a scanner; each one is documented below with its
//! rule and its false-positive posture, because a class whose posture is not
//! written down is a class that gets quietly weakened.
//!
//! | class | rule | false-positive posture |
//! |---|---|---|
//! | `hardcoded-absolute-path` | ANY string literal whose first byte is `/` and which is longer than one byte (complete, no list); plus a `/`-rooted segment from [`ABSOLUTE_PATH_ROOTS`] at a non-path boundary anywhere on the line, case-insensitive; plus any drive-letter root `[A-Za-z]:[\\/]` | flags absolute paths in comments too — naming a machine's layout in core prose is the same coupling. A lone `"/"` separator is not flagged |
//! | `hardcoded-relative-path` | a string literal that is path-shaped by STRUCTURE: rooted at `./`, `../`, `~/`, multi-segment with every segment filename-shaped — extension or not (critic-5 M2) — or carrying any explicit `..` parent-directory segment, whatever the other segments look like (critic-8 E1: `my dir/../../../vendor/store.rs` navigates the filesystem exactly like `../vendor/store.rs`) | wide on purpose: `"config/defaults"` resolves against the working directory exactly as `"config/defaults.toml"` does. Slash vocabulary and media types are exempted BY NAME in [`RELATIVE_PATH_EXEMPTIONS`], each with its reason, so the exceptions are argued rather than inferred |
//! | `hardcoded-provider-endpoint` | a string literal containing a dotted host whose final label is a registered TLD ([`ENDPOINT_TLDS`]), e.g. `api.some-vendor.com` with no scheme and no port | string literals only; `config.toml`/`state.db` end in no TLD, and the RFC 2606 documentation names are exempt with reasons ([`ENDPOINT_EXEMPTIONS`]). An endpoint under an unlisted TLD is the residual |
//! | `unscannable-declaration` | a `const`/`static`/`let` declaration that does not reach its `;` within [`crate::text::MAX_DECLARATION_LINES`] | not a literal class but a completeness report (I8): the timeout and tier classes need identifier and value in one unit, so a declaration the scanner cannot join is a hole it must name (critic-5 M1) |
//! | `hardcoded-url` | any `scheme://` (`wss`, `file`, `ftp`, `grpc`, …), not just http(s) | a `://` inside a non-URL string is vanishingly rare; accepted |
//! | `hardcoded-model-id` | a family from [`MODEL_FAMILIES`] at a word boundary followed by `-<alnum>`, or from [`CONCATENATED_FAMILIES`] followed by a digit | enumerated, so a family nobody listed is missed — see "residual" below |
//! | `hardcoded-provider` | a string literal that is exactly a provider from [`PROVIDER_IDENTITIES`] | whole literals only, so `azure`/`google` as prose never flag; enumerated, so an unlisted provider is missed — see "residual" (critic-4 M3) |
//! | `hardcoded-network-endpoint` | IPv4, `[v6]:port`, and `host:port` for ARBITRARY hosts (any dotted name, or `localhost`) | a dotted token followed by `:digits` in prose flags; rare enough to accept |
//! | `hardcoded-port` | a numeric literal whose value is a well-known port, in any Rust spelling: `8080`, `8_080`, `0x1f90`, `0o17620`, `8080u16` | a well-known port value used as a plain number flags; deliberate — outside the defaults table it is a configuration leak either way |
//! | `hardcoded-command` | a multi-token string literal whose first token is an executable from [`COMMAND_HEADS`] and which contains no English stopword; plus any `Command::new("…")` | scoped to STRING LITERALS only, so prose and identifiers never flag; a sentence beginning with a tool name is excluded by the stopword guard. Shell metacharacters (`>`, `\|`, `&`, quotes, globs) are EVIDENCE, never an exemption — round-3 V16 |
//! | `hardcoded-tool-identity` | a string literal that is exactly a product from [`TOOL_IDENTITIES`] **or an executable head from [`COMMAND_HEADS`]** — the union is computed, so the two lists cannot drift apart again (critic-4 M2) | string literals only; the risk is a config PARSER matching on `"git"` — see "residual" |
//! | `hardcoded-timeout` | a numeric literal ≥ 2 in a value position — introduced by `=`, `(`, `,`, `:` (struct-field initialisers), `>` (match arms) or `[` — on a line carrying a duration-shaped identifier ([`TIMEOUT_HINTS`]) | the hint requirement is what keeps `(spec §11.5)` and markdown tables from flagging; until critic-6 C2 the class was also narrow by punctuation, blind to `Timeouts { stall_ms: 900_000 }` |
//! | `hardcoded-tier-policy` | a string literal from [`TIER_TERMS`], or matching `tier[-_ ]<digit>`, on a `const`/`static`/`default` line, unless the literal sits INSIDE an attribute body (delimited by the lexing pass, so a `#[` in a comment no longer disables the class — critic-6 C3) | narrow ON PURPOSE — see "residual" |
//!
//! # Residual, recorded rather than hidden
//!
//! Three of these classes cannot be made both complete and mechanical today.
//! (The absolute-path class no longer belongs to this list: round-3 V8/V19
//! replaced its enumerated-only posture with a complete boundary rule over
//! string literals, keeping [`ABSOLUTE_PATH_ROOTS`] only as the line-level
//! reach into comments and identifiers.)
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
//! - **provider** (critic-4 M3) is an enumerated whole-literal match over
//!   [`PROVIDER_IDENTITIES`]. A provider nobody listed (`"acme-cloud"`)
//!   passes, exactly like an unlisted model family; the list is widened when
//!   a provider is met. As substrings these names collide with ordinary
//!   vocabulary (`gemini` is also a constellation), which is why the class is
//!   whole-literal only.
//! - **relative path** (critic-4 M4, widened by critic-5 M2) is a structural
//!   match over string literals: `./`-, `../`-, `~`-rooted literals, and any
//!   multi-segment literal whose segments are all filename-shaped —
//!   `"run/state"` and `"a/tmpfile"` now flag, because §8.1 says "no path",
//!   not "no path with an extension". Slash-separated vocabulary and media
//!   types (`"and/or"`, `"image/png"`) are exempted by NAME, each with a
//!   recorded reason, in [`RELATIVE_PATH_EXEMPTIONS`]; the residual is a
//!   slash-separated phrase nobody exempted, which fails the gate until it is
//!   argued in that list. That is the intended direction of error.
//! - **provider endpoint** (critic-5 M2) is a TLD-anchored match over string
//!   literals. `api.acme.internal` (no registered TLD) and a hostname written
//!   only in a comment are the residuals; both cost a name in prose rather
//!   than a coupling in compiled code.
//!
//! Four narrower residuals, recorded here for the same reason (round 3 +
//! critic 4):
//!
//! - **timeout** matches an enumerated [`TIMEOUT_HINTS`] fragment at an
//!   alphanumeric word boundary, so `BUILDER_TIMEOUT` is found and the
//!   unnaturally spelled `BUILDERTIMEOUT` is not, and a duration-shaped name
//!   nobody listed (`BUILDER_LIMIT`) is not either. The boundary is what keeps
//!   `install` from matching `stall`; dropping it would trade this residual for
//!   a false-positive factory.
//! - **command** now has exactly one prose guard, the [`STOPWORDS`] list: a
//!   multi-token string literal headed by a known executable and containing no
//!   English function word is a command. The character-class filter that used
//!   to sit after it was a bypass, not a guard (V16), and is gone. An English
//!   sentence with no stopword that begins with a tool name would flag; that is
//!   the direction the class is meant to err in.
//! - **absolute path** is complete inside string literals and enumerated
//!   outside them: an absolute path written only in a comment, rooted at a
//!   directory not in [`ABSOLUTE_PATH_ROOTS`], is not reported. Comments carry
//!   no compiled value, so this residual costs a name in prose, not a coupling
//!   in code.
//! - **runtime-assembled strings** (critic-4 m2) defeat every class here and
//!   every term in `crate::denylist`: `["mo", "ck"].join("")` contains no
//!   whole literal, and only a parser with dataflow could see the assembled
//!   value. Normalization closes the *splitting* family (continuations,
//!   `concat!`, escapes, statement breaks); it cannot close deliberate
//!   runtime assembly. Recorded, not hidden.
//!
//! Scope: the denylist scope, file for file — every regular text file under
//! the four core crates, including `gauntlet-cli` AND including the declared
//! §8.6 defaults module. The defaults exemption is DECLARATION-granular
//! (critic-7 C1, then critic-9 F2 / critic-8 E7): in that one file, each
//! setting's own declaration — head to terminating `;` — is blanked out
//! before matching, nothing else is, and the warrant
//! (`crate::check_defaults_exemption`) requires the file, if it exists, to
//! declare every §8.6 setting. Matching runs over `crate::text::scan_units`,
//! so a literal split by a line-continuation, by `concat!`, by a `\x`/`\u{}`
//! escape, by a rustfmt statement break, or by a break between a directive
//! head and its argument is still found. String, char, and comment
//! delimiting comes from the ONE lexing pass (`crate::text::regions`) — a
//! quote that is data can no longer delete the literals after it
//! (critic-6 C1).

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

/// Model-provider identities (spec §8.1/§8.4: the runtime names capabilities,
/// `GAUNTLET_PROVIDER_*`, never products). AK-016 and §8.1 name "provider" as
/// a class of its own; until critic-4 M3 it was the one class in that
/// sentence with no matcher and no disclosed residual. Matched as a WHOLE
/// string literal only — as substrings, `gemini` and `azure` collide with
/// ordinary vocabulary. The list is widened whenever a provider is met.
pub const PROVIDER_IDENTITIES: &[&str] = &[
    "anthropic",
    "azure",
    "bedrock",
    "cerebras",
    "chatgpt",
    "claude",
    "cohere",
    "copilot",
    "deepinfra",
    "deepseek",
    "fireworks",
    "gemini",
    "google",
    "grok",
    "groq",
    "huggingface",
    "mistral",
    "moonshot",
    "nvidia",
    "ollama",
    "openai",
    "openrouter",
    "perplexity",
    "phind",
    "qwen",
    "replicate",
    "together",
    "vertex",
    "vertex-ai",
    "vertex_ai",
    "watsonx",
    "xai",
    "zhipu",
];

/// Final labels that make a dotted token a NETWORK NAME rather than a file
/// name (critic-5 M2). `api.deepinfra.com/v1` passed every class before this
/// one existed: no `://` for the URL class, no `:port` for the endpoint class,
/// and not a whole-literal provider identity.
///
/// The list is what keeps the class precise: `config.toml`, `state.db` and
/// `gauntlet.rs` are dotted tokens too, and none of them ends in a registered
/// top-level domain. An endpoint under a TLD nobody listed is the recorded
/// residual, in the same shape as the model-family and provider lists.
pub const ENDPOINT_TLDS: &[&str] = &[
    "com", "net", "org", "io", "ai", "dev", "app", "cloud", "co", "sh", "xyz", "me", "us", "eu",
    "uk", "de", "gov", "edu", "info", "tech", "live", "cn", "jp", "inc", "llc", "systems",
];

/// Literals that look like a network name but are not one, each with the
/// reason it is exempt. `endpoint_exemptions_state_a_reason` fails the gate if
/// an entry is added without a reason, so the list cannot be used to quietly
/// widen a hole.
pub const ENDPOINT_EXEMPTIONS: &[(&str, &str)] = &[
    (
        "example.com",
        "RFC 2606 reserves example.com for documentation; it resolves to nothing operational, so \
         it is prose rather than configuration",
    ),
    (
        "example.org",
        "RFC 2606 reserves example.org for documentation, exactly like example.com",
    ),
    (
        "example.net",
        "RFC 2606 reserves example.net for documentation, exactly like example.com",
    ),
];

/// Slash-separated literals that are vocabulary, media types, or notation
/// rather than filesystem paths, each with the reason it is exempt (critic-5
/// M2 / spec §8.1 "no path … is a literal"). The path class is structural and
/// deliberately wide, so this list is where the exceptions are argued in the
/// open; `relative_path_exemptions_state_a_reason` fails the gate if an entry
/// is added without a reason.
pub const RELATIVE_PATH_EXEMPTIONS: &[(&str, &str)] = &[
    (
        "and/or",
        "an English conjunction: it names no file and resolves against no directory",
    ),
    (
        "input/output",
        "English vocabulary for the two directions of a stream, not a directory pair",
    ),
    (
        "read/write",
        "English vocabulary for an access mode, as in an open flag description",
    ),
    (
        "n/a",
        "the conventional written form of 'not applicable' in a report field",
    ),
    (
        "24/7",
        "a schedule idiom; the slash is notation, not a separator",
    ),
    (
        "application/json",
        "an IANA media type: a wire content type carried in a header, which names no location on \
         any filesystem",
    ),
    (
        "application/octet-stream",
        "an IANA media type, exactly like application/json",
    ),
    (
        "text/plain",
        "an IANA media type, exactly like application/json",
    ),
    (
        "text/markdown",
        "an IANA media type, exactly like application/json",
    ),
    (
        "image/png",
        "an IANA media type, exactly like application/json",
    ),
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
/// command line. Since round-3 V16 removed the character-class filter, this is
/// the command class's ONLY narrowing, which is why it is public and named in
/// the module's residual section.
pub const STOPWORDS: &[&str] = &[
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
///
/// The ONE exemption is DECLARATION-granular (critic-7 C1, then critic-9 F2 /
/// critic-8 E7): in the declared §8.6 defaults module
/// ([`crate::walk::DEFAULTS_MODULE`]), each declaration of one of
/// [`crate::walk::DEFAULTS_TABLE_SETTINGS`] — from its head to the code `;`
/// that terminates it — is blanked out before matching; the default's VALUE
/// is the exemption's whole warrant, so the warrant's extent is the
/// declaration itself, not the scan unit that happens to carry it. Everything
/// else in that file, and every unit of every other file, is policed by all
/// classes. Round 6 skipped the whole unit when it declared a setting, and a
/// unit can hold more than one declaration: a second one packed onto the
/// setting's line escaped every class. Until round 6 the whole FILE was
/// outside the literal scan, which made eight of the nine §8.1 classes
/// hardcodable by anything that landed at that path.
pub fn scan_literals(path: &str, text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |message: String, out: &mut Vec<String>| {
        if !out.contains(&message) {
            out.push(message);
        }
    };
    let defaults_scope = path == crate::walk::DEFAULTS_MODULE;

    // Critic-5 M1, invariant I8: the timeout and tier-policy classes need the
    // identifier and its value in ONE scan unit. A declaration this scanner
    // cannot join to its `;` within the budget is therefore a hole in those
    // two classes, and a hole is reported, not skipped.
    for line in crate::text::unjoinable_declarations(text) {
        push(
            format!(
                "unscannable-declaration: {path}:{line}: a const/static/let declaration does not \
                 reach its terminating ';' within {} lines, so the scanner cannot put its \
                 identifier and its value in one unit — the timeout and tier-policy classes are \
                 blind to it, and missing evidence is not passing evidence (I8, AK-016)",
                crate::text::MAX_DECLARATION_LINES
            ),
            &mut out,
        );
    }

    for unit in crate::text::scan_units(text) {
        let masked;
        let raw: &str = if defaults_scope {
            // Critic-9 F2 / critic-8 E7: the exemption's unit of skipping is
            // now its unit of warrant. Only the setting's own declaration —
            // head to terminating `;` — is blanked out; the rest of the unit,
            // including a second declaration packed onto the same physical
            // line, is policed by every class below.
            masked = mask_setting_declarations(&unit.text);
            masked.as_str()
        } else {
            unit.text.as_str()
        };
        let line_no = unit.line;
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
        if let Some(found) = find_absolute_path_literal(&crate::text::string_literals(raw)) {
            push(
                format!(
                    "hardcoded-absolute-path: {path}:{line_no}: '{found}' — a string literal \
                     rooted at '/' is an absolute path, and absolute paths are configuration \
                     (spec §8.1), not code"
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
        if let Some(found) = find_relative_path(&crate::text::string_literals(raw)) {
            push(
                format!(
                    "hardcoded-relative-path: {path}:{line_no}: '{found}' — a path literal is \
                     environment coupling wherever it is rooted (spec §8.1, AK-016); resolve \
                     paths from configuration"
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

        let literals = crate::text::string_literals(raw);
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
        if let Some(provider) = find_provider_identity(&literals) {
            push(
                format!(
                    "hardcoded-provider: {path}:{line_no}: '{provider}' — provider identities are \
                     configuration (spec §8.1, AK-016), never code"
                ),
                &mut out,
            );
        }
        if let Some(endpoint) = find_provider_endpoint(&literals) {
            push(
                format!(
                    "hardcoded-provider-endpoint: {path}:{line_no}: '{endpoint}' — a provider \
                     endpoint is configuration (spec §8.1, AK-016) whether or not it carries a \
                     scheme or a port"
                ),
                &mut out,
            );
        }
        if let Some(tier) = find_tier_policy(&lowered, raw) {
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

/// Whether a scan unit DECLARES one of the §8.6 default settings: after any
/// qualifiers, `const`/`static` and then an identifier that IS a §8.6 setting
/// (case-insensitive). This is the head shape whose declaration the
/// exemption masks — and nothing else. Prose and doc comments never qualify
/// (their first token is not a qualifier or a keyword), an identifier that
/// merely CONTAINS a setting name (`MAX_ROUNDS_LOG`) does not either, and
/// `crate::check_defaults_exemption` uses the same test as its
/// declaration-shaped warrant, so the exemption and the warrant cannot drift
/// apart.
pub(crate) fn is_defaults_setting_declaration(unit_text: &str) -> bool {
    let mut tokens = unit_text.split_whitespace();
    loop {
        match tokens.next() {
            Some(token)
                if token == "pub"
                    || token == "async"
                    || token == "unsafe"
                    || token == "extern"
                    || token.starts_with("pub(") =>
            {
                continue;
            }
            Some(token)
                if token.eq_ignore_ascii_case("const") || token.eq_ignore_ascii_case("static") =>
            {
                let Some(ident) = tokens.next() else {
                    return false;
                };
                let ident = ident
                    .trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .to_ascii_lowercase();
                return crate::walk::DEFAULTS_TABLE_SETTINGS
                    .iter()
                    .any(|setting| *setting == ident);
            }
            _ => return false,
        }
    }
}

/// Blank out the §8.6 setting declarations of one scan unit (critic-9 F2 /
/// critic-8 E7), replacing each declaration's bytes with spaces so the unit's
/// length — and therefore the line every class reports at — is unchanged. A
/// unit that declares no setting is returned unchanged.
///
/// Round 6 skipped the whole UNIT when it declared a setting, and a unit can
/// hold more than one declaration: `pub const MAX_ROUNDS: usize = 12; pub
/// const BIND: &str = "localhost:8080";` smuggled the second declaration past
/// every class. The exemption's unit of skipping is now its unit of warrant:
/// the setting's own declaration, nothing else on the line.
fn mask_setting_declarations(unit_text: &str) -> String {
    let spans = setting_declaration_spans(unit_text);
    if spans.is_empty() {
        return unit_text.to_string();
    }
    let mut out = String::with_capacity(unit_text.len());
    let mut at = 0usize;
    for span in &spans {
        out.push_str(&unit_text[at..span.start]);
        for _ in 0..span.len() {
            out.push(' ');
        }
        at = span.end;
    }
    out.push_str(&unit_text[at..]);
    out
}

/// The byte spans of the §8.6 setting declarations inside one scan unit: each
/// runs from the declaration's first byte to the CODE `;` that terminates it,
/// or to the end of the unit when the declaration does not terminate inside
/// it (its value then sits in another unit, which is policed). The unit is
/// split at code semicolons — a `;` inside a string, char, or comment region
/// of the ONE lexing pass is data and never ends a declaration — and each
/// chunk is tested with the same declaration-shape predicate the warrant
/// uses, so what the exemption masks is exactly what
/// `crate::check_defaults_exemption` accepts as a declaration.
fn setting_declaration_spans(unit_text: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = unit_text.as_bytes();
    let regions = crate::text::regions(unit_text);
    let mut spans = Vec::new();
    let mut chunk_start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if let Some(span) = regions
            .iter()
            .find(|region| region.span().contains(&i))
            .map(|region| region.span())
        {
            i = span.end;
            continue;
        }
        if bytes[i] == b';' {
            push_if_setting_declaration(unit_text, chunk_start, i + 1, &mut spans);
            chunk_start = i + 1;
        }
        i += 1;
    }
    push_if_setting_declaration(unit_text, chunk_start, bytes.len(), &mut spans);
    spans
}

fn push_if_setting_declaration(
    text: &str,
    from: usize,
    to: usize,
    spans: &mut Vec<std::ops::Range<usize>>,
) {
    if is_defaults_setting_declaration(&text[from..to]) {
        spans.push(from..to);
    }
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

/// A string literal that is itself rooted at `/`.
///
/// [`find_absolute_path`] is an ENUMERATED match over [`ABSOLUTE_PATH_ROOTS`],
/// which round 3 showed to be incomplete in two directions: `/app/config.toml`
/// and `/scratch/state` are absolute paths rooted at no listed name (V19), and
/// `"/./home/agent/state"` resolves to a LISTED root on every POSIX host while
/// defeating the matcher's boundary guard (V8). Inside a string literal the
/// complete rule is mechanical and needs no list: a literal whose first byte
/// is `/` and which is longer than that single byte is an absolute path.
///
/// A lone `"/"` — the path separator itself, as in `parts.join("/")` — is
/// deliberately not a violation. The enumerated line-level class stays, because
/// it also reaches absolute paths written in comments and identifiers, where
/// there is no literal to anchor on.
fn find_absolute_path_literal(literals: &[String]) -> Option<String> {
    literals
        .iter()
        .find(|literal| literal.len() > 1 && literal.starts_with('/'))
        .cloned()
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

/// A string literal that is path-shaped by structure (critic-4 M4): AK-016
/// says "no PATH is a hardcoded literal", and until this class the rule was
/// enforced as "no ABSOLUTE path" — `"src/state/db.sqlite"` and
/// `"./data"` compiled into core code with every locator green.
///
/// The boundary is structural, not enumerated: a literal rooted at `./`,
/// `../`, or `~/`, or a multi-segment literal whose final segment is
/// extension-shaped (`name.ext`, extension of 1–6 ASCII alphanumerics).
/// `"a/tmpfile"` and `"and/or"` carry no such markers and stay clean — the
/// recorded residual in the module table.
fn find_relative_path(literals: &[String]) -> Option<String> {
    literals
        .iter()
        .find(|literal| looks_like_relative_path(literal))
        .cloned()
}

fn looks_like_relative_path(literal: &str) -> bool {
    if RELATIVE_PATH_EXEMPTIONS
        .iter()
        .any(|(exempt, _)| exempt.eq_ignore_ascii_case(literal.trim()))
    {
        return false;
    }
    if literal.starts_with("./")
        || literal.starts_with("../")
        || literal.starts_with("~/")
        || literal.starts_with(".\\")
        || literal.starts_with("..\\")
        || literal.starts_with("~\\")
    {
        return true;
    }
    // Critic-5 M2 / spec §8.1: "no PATH is a literal" — not "no absolute path",
    // and not "no path with a file extension" either. A multi-segment literal
    // whose segments are all path-segment shaped IS a relative path;
    // `config/defaults` and `state/db` resolve against the process working
    // directory exactly as `state/db.sqlite` does. What keeps this from
    // flagging arbitrary prose is the segment shape (no spaces, no `:`, no
    // punctuation beyond the characters a filename uses) plus the exemption
    // list above, which is argued entry by entry rather than inferred.
    let normalized = literal.replace('\\', "/");
    let segments: Vec<&str> = normalized.split('/').collect();
    // Critic-8 E1, the backstop half: a literal carrying an explicit
    // parent-directory segment is a path by construction, whatever the other
    // segments look like. `my dir/../../../vendor/store.rs` is the critic's
    // escape payload — one space-bearing segment made the whole literal read
    // as prose to the every-segment rule, while it resolves against the
    // filesystem exactly like `../vendor/store.rs`. The `..` segment IS the
    // navigation; the spaces around it change nothing.
    if segments.contains(&"..") {
        return true;
    }
    segments.len() >= 2 && segments.iter().all(|segment| is_path_segment(segment))
}

/// Whether a `/`-separated fragment has the shape of a filesystem path
/// segment: non-empty, made only of the bytes a filename uses, and not a bare
/// separator artefact.
fn is_path_segment(segment: &str) -> bool {
    if segment == "." || segment == ".." {
        return true;
    }
    !segment.is_empty()
        && segment.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'+' | b'~' | b'@' | b'%')
        })
        && segment.bytes().any(|b| b.is_ascii_alphanumeric())
        && !segment.starts_with('%')
}

/// A scheme-less network name in a string literal: `api.some-vendor.com`,
/// `api.deepinfra.com/v1`, `collector.internal.example.io` (critic-5 M2).
///
/// The URL class needs `://` and the endpoint class needs `:port`; a bare
/// hostname carries neither, which is exactly the spelling that passed. The
/// rule is: a dotted token, every label filename-safe, whose FINAL label is a
/// registered top-level domain ([`ENDPOINT_TLDS`]) — so `config.toml` and
/// `state.db` do not match — and which is not on [`ENDPOINT_EXEMPTIONS`].
///
/// Scoped to string literals on purpose: a hostname named in prose is a
/// residual (like an absolute path in a comment), because a comment compiles
/// to nothing, and matching prose would make the class a false-positive
/// factory in exactly the documents that discuss providers.
fn find_provider_endpoint(literals: &[String]) -> Option<String> {
    for literal in literals {
        for token in literal.split(|b: char| !(b.is_ascii_alphanumeric() || b == '.' || b == '-')) {
            let lowered = token.to_ascii_lowercase();
            if ENDPOINT_EXEMPTIONS
                .iter()
                .any(|(exempt, _)| *exempt == lowered)
            {
                continue;
            }
            let labels: Vec<&str> = lowered.split('.').collect();
            if labels.len() < 2 || labels.iter().any(|label| !is_host_label(label)) {
                continue;
            }
            let tld = labels.last().expect("len >= 2");
            if ENDPOINT_TLDS.contains(tld) {
                return Some(lowered);
            }
        }
    }
    None
}

/// A DNS label: ASCII alphanumerics and hyphens, starting and ending
/// alphanumeric.
fn is_host_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && label
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && label
            .bytes()
            .next_back()
            .is_some_and(|b| b.is_ascii_alphanumeric())
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

/// A hardcoded command: `Command::new("…")`, or a command-shaped string
/// literal headed by a known executable.
///
/// Round-3 V16: this used to end with a character-class filter that dropped
/// any literal containing a byte outside `[A-Za-z0-9_.:/=+@%-]`, so
/// `"cargo build --workspace 2>&1"`, `"git log --pretty=%H | head"` and
/// `"sh -c 'cargo nextest run'"` all passed clean — a redirect, a pipe, a
/// quote or a glob made the class disable itself. Shell metacharacters make a
/// literal MORE command-shaped, not less, so the filter is gone. The token
/// cap (`> 12`) went with it: a thirteen-token command line is still a
/// command line.
///
/// What remains as the prose guard is the stopword list, which is the
/// documented narrowing: the class is scoped to string literals, headed by a
/// known executable, and containing no ordinary English function word.
fn find_command(lowered: &str, literals: &[String]) -> Option<String> {
    if lowered.contains("command::new(")
        && let Some(first) = literals.first()
        && !first.is_empty()
    {
        return Some(first.clone());
    }
    for literal in literals {
        let tokens: Vec<&str> = literal.split_whitespace().collect();
        if tokens.len() < 2 {
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
        return Some(literal.clone());
    }
    None
}

/// A string literal that is exactly a tool product name — either a
/// [`TOOL_IDENTITIES`] entry or any [`COMMAND_HEADS`] head. Critic-4 M2: the
/// two lists used to differ, so a single-token command head (`"bash"`,
/// `"just"`, `"sh"`) compiled into core code as a whole literal matched
/// nothing — including when it was laundered through a `const` before
/// `Command::new(…)`. Every executable head IS a tool product, and the union
/// is computed rather than maintained, so the lists cannot drift apart again.
fn find_tool_identity(literals: &[String]) -> Option<String> {
    literals
        .iter()
        .find(|literal| {
            let value = literal.trim().to_lowercase();
            TOOL_IDENTITIES.contains(&value.as_str()) || COMMAND_HEADS.contains(&value.as_str())
        })
        .cloned()
}

/// A string literal that is exactly a model-provider identity (critic-4 M3:
/// the "provider" clause of §8.1/AK-016 had no matcher and no disclosed
/// residual — `"deepinfra"` stopped one byte short of the `deepseek` term).
fn find_provider_identity(literals: &[String]) -> Option<String> {
    literals
        .iter()
        .find(|literal| PROVIDER_IDENTITIES.contains(&literal.trim().to_lowercase().as_str()))
        .cloned()
}

/// A duration/schedule policy number: a literal ≥ 2 introduced by a value
/// position — `=`, `(`, `,`, `:`, `>` (match arms), or `[` — on a line
/// carrying a duration-shaped identifier.
///
/// Critic-6 C2: the class used to accept only `=`, `(` and `,`, which made it
/// narrow by PUNCTUATION rather than by value: a struct-field initialiser
/// (`Timeouts { stall_ms: 900_000 }`), a match arm (`=> 900_000`), and an
/// array value (`= [900_000]`) all fell outside it. The hint requirement is
/// what actually narrows the class (spec cross-references and markdown tables
/// carry no duration-shaped identifier next to the number).
fn find_timeout(lowered: &str, numbers: &[NumericLiteral]) -> Option<(String, String)> {
    let hint = TIMEOUT_HINTS
        .iter()
        .find(|hint| contains_hint(lowered, hint))?
        .to_string();
    let number = numbers.iter().find(|n| {
        n.value.is_some_and(|v| v >= 2)
            && matches!(
                n.introducer,
                Some(b'=') | Some(b'(') | Some(b',') | Some(b':') | Some(b'>') | Some(b'[')
            )
    })?;
    Some((number.text.clone(), hint))
}

/// A tier/authority/review policy literal on a declaration line.
///
/// Critic-6 C3: the attribute carve-out used to be `lowered.contains("#[")` —
/// a blanket disable for the whole unit, including a `#[` inside a comment,
/// which is how `// matches the #[review] attribute` turned the class off.
/// The carve-out is now structural, from the lexing pass: a literal is exempt
/// only when it sits INSIDE an attribute body (`#[serde(rename =
/// "cross_cutting")]` is typed vocabulary, not policy). An attribute earlier
/// on the line, or a `#[` in a comment, no longer disables the class.
fn find_tier_policy(lowered: &str, raw: &str) -> Option<String> {
    if !(lowered.contains("const ") || lowered.contains("static ") || lowered.contains("default")) {
        return None;
    }
    let attributes = crate::text::attribute_spans(raw);
    for (at, literal) in crate::text::string_literal_spans(raw) {
        if attributes.iter().any(|span| span.contains(&at)) {
            continue;
        }
        let value = literal.trim().to_lowercase();
        if TIER_TERMS.contains(&value.as_str()) {
            return Some(literal);
        }
        if let Some(rest) = value.strip_prefix("tier")
            && rest
                .strip_prefix(['-', '_', ' '])
                .is_some_and(|d| d.bytes().next().is_some_and(|b| b.is_ascii_digit()))
        {
            return Some(literal);
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
            assert!(!violations.is_empty(), "{fixture} -> {violations:?}");
            assert!(
                violations
                    .iter()
                    .all(|v| v.starts_with("hardcoded-absolute-path: f.rs:1:")),
                "{fixture} -> {violations:?}"
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
    fn absolute_path_literals_outside_the_root_list_are_flagged() {
        // Round-3 V19: the class was an enumeration, so an absolute path
        // rooted at an unlisted directory passed clean. /app in particular is
        // the standard container working directory.
        for fixture in [
            "pub const CONFIG: &str = \"/app/config.toml\";",
            "pub const STATE: &str = \"/scratch/gauntlet\";",
            "pub const OUT: &str = \"/workdir/state\";",
            "pub const CODE: &str = \"/code/repo\";",
            // Round-3 V8: forms that resolve to a LISTED root but defeated
            // the boundary guard of the enumerated matcher.
            "pub const STATE_ROOT: &str = \"/./home/agent/gauntlet/state\";",
            "pub const STATE_ROOT: &str = \"//home/agent/state\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-absolute-path: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
        // The path separator itself is not an absolute path.
        assert!(scan_literals("f.rs", "let joined = parts.join(\"/\");").is_empty());
    }

    #[test]
    fn path_shaped_relative_literals_are_flagged_and_vocabulary_is_not() {
        // Critic-4 M4: the class used to be "no ABSOLUTE path", so these
        // compiled into core code with every locator green. The old unit test
        // pinned that narrowing (`relative_unix_paths_are_not_flagged`); it
        // was an undisclosed narrowing of a named AK-016 clause, and the
        // correction contract replaces it with the structural boundary form.
        for fixture in [
            "pub const STATE_DB: &str = \"state/gauntlet.db\";",
            "pub const DB: &str = \"src/state/db.sqlite\";",
            "let d = \"./data\";",
            "let d = \"../shared/data\";",
            "let home = \"~/.gauntlet/state\";",
            "let db = \"run/state.db\";",
            "let cfg = \"config/defaults.toml\";",
            "let notes = \"docs/dev/setup.md\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-relative-path: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
        // Critic-5 M2: the class no longer stops at an extension. §8.1 says
        // "no path", and `"run/state"` resolves against the working directory
        // exactly as `"run/state.db"` does. The old negative fixtures that
        // WERE paths are now positives; the ones that are vocabulary or media
        // types are exempt BY NAME, with the reason recorded beside them.
        for fixture in [
            "let p = \"a/tmpfile\";",
            "let p = \"run/state\";",
            "let p = \"config/defaults\";",
            "let p = \"crates/gauntlet-domain\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-relative-path")),
                "{fixture} -> {violations:?}"
            );
        }
        for fixture in [
            "let p = \"and/or\";",
            "let p = \"input/output\";",
            "let p = \"read/write\";",
            "let p = \"image/png\";",
            "let p = \"application/json\";",
            "let p = \"n/a\";",
            "let p = \"0.9.12+spec\";",
            // Not path-shaped: a segment carrying a space or a `:` is prose.
            "let p = \"either this / or that\";",
            "let p = \"key: value/other\";",
            "let joined = parts.join(\"/\");",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .all(|v| !v.starts_with("hardcoded-relative-path")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    /// The exemption list is the one place the widened path class can be
    /// quietly narrowed, so every entry must carry a stated reason — an entry
    /// added without one fails this test rather than the reviewer's attention.
    #[test]
    fn relative_path_exemptions_state_a_reason() {
        for (literal, reason) in RELATIVE_PATH_EXEMPTIONS {
            assert!(
                reason.split_whitespace().count() >= 6,
                "the path-class exemption {literal:?} carries the reason {reason:?} — say why it \
                 is not configuration, or let the class flag it"
            );
            assert!(
                !looks_like_relative_path(literal),
                "{literal:?} is on the exemption list but still flagged — the list would be \
                 decoration"
            );
        }
        // The exemptions are exact, not prefixes: a real path that merely
        // starts with an exempt word is still a path.
        assert!(looks_like_relative_path("image/png/../../etc/passwd"));
        assert!(looks_like_relative_path("and/or/config.toml"));
    }

    /// Critic-5 M2: the scheme-less, port-less provider endpoint that passed
    /// every other class.
    #[test]
    fn scheme_less_provider_endpoints_are_flagged() {
        for fixture in [
            "pub const PROVIDER_BASE: &str = \"api.deepinfra.com/v1\";",
            "pub const BASE: &str = \"api.some-vendor.com\";",
            "pub const COLLECTOR: &str = \"otel.acme.io\";",
            "let host = \"models.vendor.ai\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-provider-endpoint: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
        // Precision: dotted names that are file names, versions, or crate
        // paths are not hosts, and prose in a comment is not a literal.
        for fixture in [
            "let f = \"config.toml\";",
            "let f = \"gauntlet.db\";",
            "let v = \"0.9.12\";",
            "// see api.deepinfra.com for the endpoint shape",
            "//! The runtime resolves the provider base URL from configuration.",
            "let m = \"application/json\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .all(|v| !v.starts_with("hardcoded-provider-endpoint")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn endpoint_exemptions_state_a_reason() {
        for (literal, reason) in ENDPOINT_EXEMPTIONS {
            assert!(
                reason.split_whitespace().count() >= 6,
                "the endpoint exemption {literal:?} carries the reason {reason:?}"
            );
            assert!(
                find_provider_endpoint(&[(*literal).to_string()]).is_none(),
                "{literal:?} is exempt but still flagged"
            );
        }
        // And the exemption does not cover a real host that merely contains an
        // exempt name as a suffix label pair.
        assert!(find_provider_endpoint(&["api.example.com".to_string()]).is_some());
    }

    /// Critic-5 M1: a declaration split beyond any fixed join window. The
    /// classes that need identifier and value together must either see them
    /// (bounded join) or say they could not (I8) — never go quietly silent.
    #[test]
    fn a_declaration_split_past_the_old_window_is_still_seen() {
        let filler = "    //\n".repeat(40);
        let timeout =
            format!("pub const STALL_TIMEOUT_MS: u64 =\n{filler}    900_000;\npub fn f() {{}}\n");
        let violations = scan_literals("f.rs", &timeout);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-timeout")),
            "{violations:?}"
        );

        let tier = format!(
            "pub static REVIEW_MODE: &str =\n{filler}    \"adversarial\";\npub fn f() {{}}\n"
        );
        let violations = scan_literals("f.rs", &tier);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy")),
            "{violations:?}"
        );

        // Past the budget the scanner reports that it could not join, rather
        // than reporting nothing.
        let unjoinable = format!(
            "pub const STALL_TIMEOUT_MS: u64 =\n{}    900_000;\n",
            "    //\n".repeat(crate::text::MAX_DECLARATION_LINES + 2)
        );
        assert!(
            scan_literals("f.rs", &unjoinable)
                .iter()
                .any(|v| v.starts_with("unscannable-declaration: f.rs:1:")),
            "a declaration the scanner cannot join must be a violation, not silence"
        );

        // Negative: ordinary declarations produce no completeness report.
        assert!(
            scan_literals(
                "f.rs",
                "pub const A: usize = 1;\npub static B: &str = \"x\";\nlet c = compute();\n"
            )
            .iter()
            .all(|v| !v.starts_with("unscannable-declaration")),
        );
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
    fn commands_carrying_shell_metacharacters_are_flagged() {
        // Round-3 V16's exact evasions: a redirect, a pipe, a quote or a glob
        // used to make the class disable itself for the whole literal.
        for fixture in [
            "pub const BUILD_CMD: &str = \"cargo build --workspace 2>&1\";",
            "pub const LOG_CMD: &str = \"git log --pretty=%H | head\";",
            "pub const SHELL_CMD: &str = \"sh -c 'cargo nextest run'\";",
            "pub const GLOB_CMD: &str = \"rg --files crates/**/*.rs\";",
            "pub const REDIRECT: &str = \"cargo check > out.txt\";",
            // A thirteen-token command line, past the old `> 12` cap.
            "pub const LONG: &str = \"cargo nextest run -p one -p two -p three -p four -p five\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-command: f.rs:1:")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn timeouts_and_tier_policy_survive_a_rustfmt_line_break() {
        // Round-3 V17's exact evasions: the shape rustfmt produces once the
        // declaration exceeds the line width put the identifier and its value
        // in two different scan units.
        let timeout = "pub const GAUNTLET_BUILDER_INVOCATION_STALL_TIMEOUT_MILLISECONDS: u64 =\n    900_000;\n";
        let violations = scan_literals("f.rs", timeout);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-timeout: f.rs:1:")),
            "{violations:?}"
        );

        let tier = "pub static REVIEW_MODE: &str =\n    \"adversarial\";\n";
        let violations = scan_literals("f.rs", tier);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy: f.rs:1:")),
            "{violations:?}"
        );

        let model = "pub const BUILDER_MODEL: &str =\n    \"claude-3-5-sonnet\";\n";
        let violations = scan_literals("f.rs", model);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-model-id: f.rs:1:")),
            "{violations:?}"
        );
    }

    #[test]
    fn escaped_and_concatenated_literals_are_flagged() {
        // Round-3 V6/V11: a single escaped character, a raw-string fragment,
        // or an interleaved comment used to hide the compiled value.
        for (fixture, prefix) in [
            (
                "pub const ROOT: &str = \"/\\x74mp/gauntlet/state\";",
                "hardcoded-absolute-path",
            ),
            (
                "pub const ROOT: &str = \"\\u{2f}tmp/gauntlet\";",
                "hardcoded-absolute-path",
            ),
            (
                "pub const MODEL: &str = concat!(\"claude\", /* id */ \"-3-5-sonnet\");",
                "hardcoded-model-id",
            ),
            (
                "pub const MODEL: &str = concat!(\"clau\", r\"de-3-5-sonnet\");",
                "hardcoded-model-id",
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

    #[test]
    fn timeout_and_tier_survive_a_split_before_the_equals() {
        // Critic-4 M1's exact evasions: the identifier line carries the hint,
        // the value line the number, and neither unit held both.
        let timeout = "pub const STALL_TIMEOUT_MS: u64\n    = 900_000;\n";
        let violations = scan_literals("f.rs", timeout);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-timeout: f.rs:1:")),
            "{violations:?}"
        );

        let tier = "pub static REVIEW_MODE: &str\n    = \"adversarial\";\n";
        let violations = scan_literals("f.rs", tier);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy: f.rs:1:")),
            "{violations:?}"
        );
    }

    #[test]
    fn single_token_command_heads_and_define_then_use_laundering_are_flagged() {
        // Critic-4 M2's exact evasion: `"bash"` is one token, the command
        // class needs two, and `bash` was in no identity list.
        let violations = scan_literals("f.rs", "pub const SHELL: &str = \"bash\";\n");
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tool-identity: f.rs:1:")),
            "{violations:?}"
        );

        // The laundering half: the literal lives on a const, `Command::new`
        // takes the identifier — the compiled behavior is still a hardcoded
        // bash invocation, and the tool-identity half now catches it.
        let laundering = "pub const SHELL: &str = \"bash\";\npub fn shell() -> Command { \
                          std::process::Command::new(SHELL) }\n";
        let violations = scan_literals("f.rs", laundering);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tool-identity") && v.contains("'bash'")),
            "{violations:?}"
        );

        for head in ["just", "sh", "zsh", "python3", "make"] {
            let violations = scan_literals("f.rs", &format!("pub const H: &str = \"{head}\";\n"));
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-tool-identity")),
                "{head} -> {violations:?}"
            );
        }
    }

    #[test]
    fn provider_identities_are_flagged() {
        // Critic-4 M3's exact evasion: `deepinfra` contains no denied term,
        // no model family, no endpoint.
        let violations = scan_literals("f.rs", "pub const PROVIDER: &str = \"deepinfra\";\n");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("hardcoded-provider: f.rs:1:"),
            "{}",
            violations[0]
        );

        for provider in ["anthropic", "ollama", "openrouter", "vertex-ai"] {
            let violations =
                scan_literals("f.rs", &format!("pub const P: &str = \"{provider}\";\n"));
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-provider")),
                "{provider} -> {violations:?}"
            );
        }
        // Whole literals only: prose naming a provider in a sentence is not a
        // selection.
        assert!(scan_literals("f.rs", "//! we migrated off openrouter last quarter\n").is_empty());
    }

    /// The curated inventories are pinned to their exact contents (critic-4
    /// vacuity M3): the iterating tests shrink in lockstep with a deleted
    /// term, and the module's "do not delete the term" instruction was prose,
    /// not a gate. Deleting or silently editing any term now fails this test,
    /// and widening one is a deliberate edit that updates the pin.
    #[test]
    fn term_inventories_are_pinned() {
        assert_eq!(
            MODEL_FAMILIES,
            &[
                "gpt", "claude", "sonnet", "haiku", "opus", "glm", "gemini", "gemma", "grok",
                "qwen", "llama", "mistral", "mixtral", "deepseek", "kimi", "minimax", "moonshot",
                "nova", "phi", "o1", "o3", "o4",
            ]
        );
        assert_eq!(
            CONCATENATED_FAMILIES,
            &[
                "gpt", "claude", "sonnet", "haiku", "glm", "gemini", "gemma", "grok", "qwen",
                "llama", "mistral", "mixtral", "deepseek", "kimi",
            ]
        );
        assert_eq!(
            PROVIDER_IDENTITIES,
            &[
                "anthropic",
                "azure",
                "bedrock",
                "cerebras",
                "chatgpt",
                "claude",
                "cohere",
                "copilot",
                "deepinfra",
                "deepseek",
                "fireworks",
                "gemini",
                "google",
                "grok",
                "groq",
                "huggingface",
                "mistral",
                "moonshot",
                "nvidia",
                "ollama",
                "openai",
                "openrouter",
                "perplexity",
                "phind",
                "qwen",
                "replicate",
                "together",
                "vertex",
                "vertex-ai",
                "vertex_ai",
                "watsonx",
                "xai",
                "zhipu",
            ]
        );
        assert_eq!(
            WELL_KNOWN_PORTS,
            &[
                80, 443, 1080, 3000, 3306, 4317, 4318, 5000, 5173, 5432, 6379, 8000, 8080, 8443,
                8888, 9000, 9090, 9222, 27017,
            ]
        );
        assert_eq!(
            ABSOLUTE_PATH_ROOTS,
            &[
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
            ]
        );
        assert_eq!(
            COMMAND_HEADS,
            &[
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
            ]
        );
        assert_eq!(
            TOOL_IDENTITIES,
            &[
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
            ]
        );
        assert_eq!(
            TIMEOUT_HINTS,
            &[
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
            ]
        );
        assert_eq!(
            TIER_TERMS,
            &[
                "contained",
                "cross_cutting",
                "cross-cutting",
                "irreversible",
                "adaptive",
                "adversarial",
                "blast_radius",
                "blast-radius",
            ]
        );
        assert_eq!(
            STOPWORDS,
            &[
                "a", "an", "the", "is", "was", "are", "were", "be", "been", "not", "and", "or",
                "to", "of", "in", "on", "for", "with", "this", "that", "it", "must", "should",
                "may", "can", "cannot", "has", "have", "no", "at", "by", "from", "into",
            ]
        );
    }

    /// Critic-7 MJ2: the exemption inventories were the only ones a single
    /// added row could widen silently — the "state a reason" tests iterate the
    /// list, so they grow in lockstep with it, and one added row un-polices
    /// exactly one literal forever. Both are pinned to their exact contents
    /// now, exactly like `DENIED_TERMS`.
    #[test]
    fn exemption_inventories_are_pinned() {
        assert_eq!(
            RELATIVE_PATH_EXEMPTIONS,
            &[
                (
                    "and/or",
                    "an English conjunction: it names no file and resolves against no directory",
                ),
                (
                    "input/output",
                    "English vocabulary for the two directions of a stream, not a directory pair",
                ),
                (
                    "read/write",
                    "English vocabulary for an access mode, as in an open flag description",
                ),
                (
                    "n/a",
                    "the conventional written form of 'not applicable' in a report field",
                ),
                (
                    "24/7",
                    "a schedule idiom; the slash is notation, not a separator",
                ),
                (
                    "application/json",
                    "an IANA media type: a wire content type carried in a header, which names no \
                     location on any filesystem",
                ),
                (
                    "application/octet-stream",
                    "an IANA media type, exactly like application/json",
                ),
                (
                    "text/plain",
                    "an IANA media type, exactly like application/json",
                ),
                (
                    "text/markdown",
                    "an IANA media type, exactly like application/json",
                ),
                (
                    "image/png",
                    "an IANA media type, exactly like application/json",
                ),
            ]
        );
        assert_eq!(
            ENDPOINT_EXEMPTIONS,
            &[
                (
                    "example.com",
                    "RFC 2606 reserves example.com for documentation; it resolves to nothing \
                     operational, so it is prose rather than configuration",
                ),
                (
                    "example.org",
                    "RFC 2606 reserves example.org for documentation, exactly like example.com",
                ),
                (
                    "example.net",
                    "RFC 2606 reserves example.net for documentation, exactly like example.com",
                ),
            ]
        );
    }

    /// Critic-6 C1's exact evasion: the `'"'` char literal in
    /// `trim_matches('"')` contributed a delimiter quote to the old naive
    /// extractor, deleting every following literal from eight classes. The
    /// one lexing pass reads it as data.
    #[test]
    fn a_quote_that_is_data_no_longer_deletes_following_literals() {
        let fixture = "pub fn agent_for(spec: &str) -> &'static str {\n    match \
                       spec.trim_matches('\"') { \"fast\" => \"mock\", _ => \"prime\" }\n}\npub \
                       const ROOT: &str = \"/var/lib/gauntlet/state\";\n";
        let violations = scan_literals("f.rs", fixture);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-absolute-path") && v.contains("/var/lib")),
            "{violations:?}"
        );

        // The critic's second fixture: a quote inside a block comment.
        let commented = "pub fn db() -> &'static str { /* \" */ \"state/gauntlet.db\" }\n";
        let violations = scan_literals("f.rs", commented);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-relative-path") && v.contains("gauntlet.db")),
            "{violations:?}"
        );

        // Same line, following the char literal: the discriminating form. A
        // scanner that treats the `'"'` byte as a delimiter loses THIS
        // literal, because the shift starts on this very line.
        let same_line = "let key = spec.trim_matches('\"'); let db = \"state/gauntlet.db\";\n";
        let violations = scan_literals("f.rs", same_line);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-relative-path") && v.contains("gauntlet.db")),
            "{same_line} -> {violations:?}"
        );

        // A raw string carrying a quote, ahead of a model id.
        let raw = "pub const NOTE: &str = r#\"a \" quote\"#;\npub const MODEL: &str = \
                   \"claude-3-5-sonnet\";\n";
        let violations = scan_literals("f.rs", raw);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-model-id")),
            "{violations:?}"
        );
    }

    /// Critic-6 C2's exact evasion: the timeout class only saw values
    /// introduced by `=`, `(` or `,`. A struct-field initialiser, a match
    /// arm, and an array value all hid their durations.
    #[test]
    fn struct_field_and_match_arm_values_introduce_a_timeout() {
        for fixture in [
            "pub struct Timeouts { pub stall_ms: u64 }\npub const POLICY: Timeouts = Timeouts { \
             stall_ms: 900_000 };\n",
            "fn pick(mode: u8) -> u64 { let stall_timeout_ms = match mode { 0 => 900_000, _ => 1 \
             }; stall_timeout_ms }\n",
            "pub const STALL_LIMITS: [u64; 1] = [900_000];\n",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-timeout") && v.contains("900_000")),
                "{fixture} -> {violations:?}"
            );
        }
        // The prose guards still hold: a spec cross-reference and a markdown
        // table carry no duration-shaped identifier next to the number.
        for fixture in [
            "//! Retry and stall policy (spec §11.5).",
            "//! | max_wall_seconds | 3600 | bounded |",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .all(|v| !v.starts_with("hardcoded-timeout")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    /// Critic-6 C3's exact evasion: `#[` anywhere in the unit — including in a
    /// comment — used to disable the tier class for the whole unit. The
    /// carve-out is structural now: only a literal INSIDE an attribute body
    /// is exempt.
    #[test]
    fn a_hash_in_a_comment_does_not_disable_the_tier_class() {
        let fixture = "pub const REVIEW_MODE: &str = \"adversarial\"; // matches the #[review] \
                       attribute\n";
        let violations = scan_literals("f.rs", fixture);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy")),
            "{violations:?}"
        );

        // A REAL attribute earlier on the same line no longer disables the
        // class either — the literal sits outside the attribute body.
        let attributed = "#[allow(dead_code)] pub const REVIEW_MODE: &str = \"adversarial\";\n";
        assert!(
            scan_literals("f.rs", attributed)
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy")),
            "the literal is outside the attribute body: {:?}",
            scan_literals("f.rs", attributed)
        );

        // The documented narrowing survives: a literal that IS an attribute's
        // value is typed vocabulary, not policy.
        assert!(
            scan_literals(
                "f.rs",
                "#[serde(rename = \"cross_cutting\")]\npub enum R {}\n"
            )
            .is_empty()
        );
    }

    /// The §8.6 exemption is setting-scoped (critic-7 C1): in the declared
    /// defaults module, exactly the declarations of the ten settings are
    /// skipped — nothing else in that file, and nothing in any other file,
    /// even when the identifier matches a setting name.
    #[test]
    fn defaults_settings_exempt_only_their_own_declarations() {
        let defaults_path = "crates/gauntlet-app/src/config/defaults.rs";
        let settings = "pub const MAX_WALL_SECONDS: u64 = 3600;\npub const REVIEW_MODE: &str = \
                        \"adaptive\";\npub static SANDBOX_BACKEND: &str = \"none\";\n";
        assert!(
            scan_literals(defaults_path, settings)
                .iter()
                .all(|v| !v.starts_with("hardcoded-timeout")
                    && !v.starts_with("hardcoded-tier-policy")),
            "the setting declarations are the exemption: {:?}",
            scan_literals(defaults_path, settings)
        );

        // The same declarations in ANY other file are not exempt.
        let violations = scan_literals("crates/gauntlet-app/src/lib.rs", settings);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-timeout"))
                && violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-tier-policy")),
            "{violations:?}"
        );

        // Inside the defaults module, an identifier that merely CONTAINS a
        // setting name is not a setting.
        let sneaky = "pub const MAX_ROUNDS_LOG: &str = \"claude-3-5-sonnet\";\n";
        assert!(
            scan_literals(defaults_path, sneaky)
                .iter()
                .any(|v| v.starts_with("hardcoded-model-id")),
            "{:?}",
            scan_literals(defaults_path, sneaky)
        );
        // And unrelated literals in the same file are policed normally.
        let hardcoded = "pub const MODEL: &str = \"gpt-5-codex\";\nfn f() -> &'static str { \
                         \"adversarial\" }\n";
        let violations = scan_literals(defaults_path, hardcoded);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-model-id")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-tier-policy")),
            "a tier literal outside a setting declaration, in the defaults module: {violations:?}"
        );
    }

    /// Critic-9 F2 (critic-8 E7): the §8.6 exemption is DECLARATION-granular.
    /// Round 6 skipped a scan UNIT that declared a setting, and one unit can
    /// hold several declarations — the second one packed onto the setting's
    /// line escaped every class. Only the setting's own declaration, head to
    /// its terminating `;`, is exempt now.
    #[test]
    fn the_defaults_exemption_covers_only_the_setting_declaration() {
        let defaults_path = "crates/gauntlet-app/src/config/defaults.rs";
        // The critic's exact payload: one line, two declarations.
        let packed =
            "pub const MAX_ROUNDS: usize = 12; pub const BIND: &str = \"localhost:8080\";\n";
        let violations = scan_literals(defaults_path, packed);
        assert!(
            violations.iter().any(
                |v| v.starts_with("hardcoded-network-endpoint") && v.contains("localhost:8080")
            ),
            "the packed BIND declaration is inside the exempt unit but outside the exempt \
             declaration: {violations:?}"
        );
        // The setting's own declaration is exempt: the same line alone flags
        // nothing.
        assert!(
            scan_literals(defaults_path, "pub const MAX_ROUNDS: usize = 12;\n").is_empty(),
            "{:?}",
            scan_literals(defaults_path, "pub const MAX_ROUNDS: usize = 12;\n")
        );

        // Order does not matter: the mask is positional, not line-wide. The
        // smuggled declaration may lead and the setting trail.
        let reversed =
            "pub const BIND: &str = \"localhost:8080\"; pub const MAX_ROUNDS: usize = 12;\n";
        assert!(
            scan_literals(defaults_path, reversed)
                .iter()
                .any(|v| v.starts_with("hardcoded-network-endpoint")),
            "{:?}",
            scan_literals(defaults_path, reversed)
        );

        // Three declarations, one setting: the other two are policed, each by
        // its own class.
        let crowded = "pub const MAX_ROUNDS: usize = 12; pub const MODEL: &str = \"gpt-5-codex\"; \
                       pub const ROOT: &str = \"/tmp/gauntlet/state\";\n";
        let violations = scan_literals(defaults_path, crowded);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-model-id")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-absolute-path")),
            "{violations:?}"
        );

        // A `;` that is DATA — inside the setting's own value string — does
        // not end the exempt declaration early, so the setting's value stays
        // exempt and the following declaration is still policed.
        let data_semi = "pub const GIT_STRATEGY: &str = \"keep;semi\"; pub const BIND: &str = \
                         \"localhost:8080\";\n";
        let violations = scan_literals(defaults_path, data_semi);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("hardcoded-network-endpoint")),
            "{violations:?}"
        );
        assert!(
            violations.iter().all(|v| !v.contains("keep;semi")),
            "the setting's own declaration, including its data semicolon, stays exempt: \
             {violations:?}"
        );
    }

    /// Critic-8 E1's backstop half: a `../`-bearing string argument of an
    /// include-family directive is an escape regardless of spaces inside it.
    /// The old every-segment-filename-shaped rule let `my
    /// dir/../../../vendor/store.rs` read as prose, so when a directive
    /// matcher missed, no literal class fired either.
    #[test]
    fn parent_directory_navigation_is_path_shaped_regardless_of_other_segments() {
        for fixture in [
            "pub const P: &str = \"my dir/../../../vendor/store.rs\";",
            "pub const P: &str = \"my dir/../../shared/wiring.rs\";",
            "pub const P: &str = \"cfg dir/../escape.rs\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .any(|v| v.starts_with("hardcoded-relative-path")),
                "{fixture} -> {violations:?}"
            );
        }
        // The prose negatives keep their shape: no `..` segment, nothing new
        // flags — a segment carrying a space or a `:` is still not a path
        // unless the literal navigates with `..`.
        for fixture in [
            "let p = \"either this / or that\";",
            "let p = \"key: value/other\";",
            "let p = \"and/or\";",
        ] {
            let violations = scan_literals("f.rs", fixture);
            assert!(
                violations
                    .iter()
                    .all(|v| !v.starts_with("hardcoded-relative-path")),
                "{fixture} -> {violations:?}"
            );
        }
    }
}
