//! Provenance refs for catalog definitions and report-only drift checks against a Herdr donor.
//!
//! A ref is `field=path:start[-end]` (1-based, inclusive lines). The field names the claim the
//! lines support; each claimed field needs exactly one ref. Paths are relative to the donor root
//! and fixed per field:
//!
//! | field                        | donor file                 | lines must mention            |
//! |------------------------------|----------------------------|-------------------------------|
//! | `executables`                | `src/detect/mod.rs`        | every executable, quoted      |
//! | `session.resume_argv`        | `src/agent_resume.rs`      | every literal argv token      |
//! | `lifecycle.detection_manifest` | `src/detect/manifest.rs` | `manifests/<name>.toml`       |
//! | `lifecycle.source` (process) | `src/detect/manifest.rs`   | the bundled registry, and no  |
//! |                              |                            | entry for the definition      |
//!
//! Nothing in this module writes to the donor or to definition files.

use crate::definition::{HarnessDefinition, LifecycleSource};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The Herdr commit the catalog was extracted from.
pub const HERDR_PINNED_COMMIT: &str = "4e624cd";
/// License recorded for every Herdr-derived definition.
pub const HERDR_LICENSE: &str = "Apache-2.0";
/// Provenance source tag for every Herdr-derived definition.
pub const HERDR_SOURCE: &str = "herdr";

const AGENT_LOOKUP_FILE: &str = "src/detect/mod.rs";
const AGENT_LOOKUP_FN: &str = "fn lookup_agent";
const RESUME_FILE: &str = "src/agent_resume.rs";
const MANIFEST_REGISTRY_FILE: &str = "src/detect/manifest.rs";
const MANIFEST_REGISTRY_MARKER: &str = "const BUNDLED_MANIFESTS";
const MANIFEST_DIR: &str = "src/detect/manifests";

/// The claim a provenance ref supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefField {
    Executables,
    ResumeArgv,
    DetectionManifest,
    LifecycleSource,
}

impl RefField {
    /// The prefix used in a ref string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Executables => "executables",
            Self::ResumeArgv => "session.resume_argv",
            Self::DetectionManifest => "lifecycle.detection_manifest",
            Self::LifecycleSource => "lifecycle.source",
        }
    }

    /// The only donor path a ref for this field may point at.
    pub fn donor_path(self) -> &'static str {
        match self {
            Self::Executables => AGENT_LOOKUP_FILE,
            Self::ResumeArgv => RESUME_FILE,
            Self::DetectionManifest | Self::LifecycleSource => MANIFEST_REGISTRY_FILE,
        }
    }

    fn parse(name: &str) -> Option<Self> {
        [
            Self::Executables,
            Self::ResumeArgv,
            Self::DetectionManifest,
            Self::LifecycleSource,
        ]
        .into_iter()
        .find(|f| f.name() == name)
    }
}

/// A parsed provenance ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRef {
    pub field: RefField,
    pub path: String,
    pub start: usize,
    pub end: usize,
}

/// Parse one `field=path:start[-end]` ref. Rejects unknown fields, wrong donor paths,
/// absolute or parent-relative paths, and malformed or inverted ranges.
pub fn parse_ref(raw: &str) -> Result<ProvenanceRef, String> {
    let (field_name, location) = raw
        .split_once('=')
        .ok_or_else(|| format!("ref {raw:?} has no field prefix"))?;
    let field = RefField::parse(field_name)
        .ok_or_else(|| format!("ref {raw:?} names unknown field {field_name:?}"))?;
    let (path, range) = location
        .rsplit_once(':')
        .ok_or_else(|| format!("ref {raw:?} has no :lines suffix"))?;
    if path != field.donor_path() {
        return Err(format!(
            "ref {raw:?}: field {} must point at {}",
            field.name(),
            field.donor_path()
        ));
    }
    let (first, last) = range.split_once('-').unwrap_or((range, range));
    let start: usize = first
        .parse()
        .map_err(|_| format!("ref {raw:?} has a non-numeric start line"))?;
    let end: usize = last
        .parse()
        .map_err(|_| format!("ref {raw:?} has a non-numeric end line"))?;
    if start == 0 || end < start {
        return Err(format!("ref {raw:?} has an empty or inverted range"));
    }
    Ok(ProvenanceRef {
        field,
        path: path.to_string(),
        start,
        end,
    })
}

/// The fields a definition claims, and therefore needs refs for.
pub fn claimed_fields(def: &HarnessDefinition) -> Vec<RefField> {
    let mut fields = vec![RefField::Executables];
    if def
        .session
        .resume_argv
        .as_ref()
        .is_some_and(|v| !v.is_empty())
    {
        fields.push(RefField::ResumeArgv);
    }
    if def.lifecycle.detection_manifest.is_some() {
        fields.push(RefField::DetectionManifest);
    }
    if matches!(def.lifecycle.source, LifecycleSource::Process) {
        fields.push(RefField::LifecycleSource);
    }
    fields
}

/// Result of a report-only check.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub definitions: usize,
    pub refs_checked: usize,
    pub refs_ok: usize,
    /// One human-readable line per drift finding. Empty means in sync.
    pub drift: Vec<String>,
}

/// Check every definition's provenance against a donor checkout. Never writes anything.
///
/// `donor_head` is the donor's resolved HEAD commit, or `None` when it cannot be determined.
pub fn check_against_donor(
    defs: &[HarnessDefinition],
    donor: &Path,
    donor_head: Option<&str>,
) -> SyncReport {
    let mut report = SyncReport {
        definitions: defs.len(),
        ..SyncReport::default()
    };

    match donor_head {
        None => report
            .drift
            .push("donor HEAD could not be determined".to_string()),
        Some(head) if !head.starts_with(HERDR_PINNED_COMMIT) => report.drift.push(format!(
            "donor HEAD {head} differs from pinned {HERDR_PINNED_COMMIT}"
        )),
        Some(_) => {}
    }

    for def in defs {
        let id = &def.id;
        let Some(prov) = &def.provenance else {
            report.drift.push(format!("{id}: no [provenance] table"));
            continue;
        };
        if prov.commit != HERDR_PINNED_COMMIT || prov.source != HERDR_SOURCE {
            report.drift.push(format!(
                "{id}: provenance source/commit ({} {}) is not herdr {HERDR_PINNED_COMMIT}",
                prov.source, prov.commit
            ));
        }

        let mut seen = BTreeSet::new();
        for raw in &prov.refs {
            report.refs_checked += 1;
            match parse_ref(raw) {
                Err(e) => report.drift.push(format!("{id}: {e}")),
                Ok(parsed) => {
                    if !seen.insert(parsed.field) {
                        report
                            .drift
                            .push(format!("{id}: duplicate ref for {}", parsed.field.name()));
                    }
                    match check_ref(def, &parsed, donor) {
                        Ok(()) => report.refs_ok += 1,
                        Err(e) => report.drift.push(format!("{id}: {raw}: {e}")),
                    }
                }
            }
        }
        for field in claimed_fields(def) {
            if !seen.contains(&field) {
                report
                    .drift
                    .push(format!("{id}: no ref for claimed field {}", field.name()));
            }
        }
    }

    report.drift.extend(inventory_drift(defs, donor));
    report
}

fn check_ref(def: &HarnessDefinition, r: &ProvenanceRef, donor: &Path) -> Result<(), String> {
    let file = donor.join(&r.path);
    let text = fs::read_to_string(&file).map_err(|_| format!("missing file {}", r.path))?;
    let lines: Vec<&str> = text.lines().collect();
    if r.end > lines.len() {
        return Err(format!(
            "range {}-{} is past end of file ({} lines)",
            r.start,
            r.end,
            lines.len()
        ));
    }
    let body = lines[r.start - 1..r.end].join("\n");
    match r.field {
        RefField::Executables => {
            for exe in &def.executables {
                if !body.contains(&format!("\"{exe}\"")) {
                    return Err(format!("executable \"{exe}\" not in range"));
                }
            }
            Ok(())
        }
        RefField::ResumeArgv => {
            let argv = def.session.resume_argv.as_deref().unwrap_or(&[]);
            for token in argv {
                if token.starts_with('{') {
                    continue;
                }
                let key = token.split('=').next().unwrap_or(token);
                if !body.contains(&format!("\"{key}")) {
                    return Err(format!("resume token \"{key}\" not in range"));
                }
            }
            Ok(())
        }
        RefField::DetectionManifest => {
            let name = def
                .lifecycle
                .detection_manifest
                .as_deref()
                .ok_or("definition has no detection_manifest")?;
            if !file_exists(donor, &format!("{MANIFEST_DIR}/{name}.toml")) {
                return Err(format!("missing manifest {MANIFEST_DIR}/{name}.toml"));
            }
            if !body.contains(&format!("manifests/{name}.toml\"")) {
                return Err(format!("manifests/{name}.toml not in range"));
            }
            Ok(())
        }
        RefField::LifecycleSource => {
            if !body.contains(MANIFEST_REGISTRY_MARKER) {
                return Err("bundled manifest registry not in range".to_string());
            }
            let mentions_def = body.contains(&format!("manifests/{}.toml\"", def.id))
                || def
                    .executables
                    .iter()
                    .any(|e| body.contains(&format!("\"{e}\"")));
            if mentions_def {
                return Err("range names this definition, so it is not process-only".to_string());
            }
            Ok(())
        }
    }
}

fn file_exists(donor: &Path, rel: &str) -> bool {
    donor.join(rel).is_file()
}

/// Donor agents and manifests that the catalog does not know about, and catalog executables
/// the donor no longer maps. Aliases containing whitespace are skipped because the catalog
/// validator rejects them as executable names.
fn inventory_drift(defs: &[HarnessDefinition], donor: &Path) -> Vec<String> {
    let mut drift = Vec::new();

    let donor_exes = match fs::read_to_string(donor.join(AGENT_LOOKUP_FILE)) {
        Ok(text) => lookup_agent_aliases(&text),
        Err(_) => {
            drift.push(format!("missing file {AGENT_LOOKUP_FILE}"));
            return drift;
        }
    };
    let catalog_exes: BTreeSet<String> = defs
        .iter()
        .flat_map(|d| d.executables.iter().cloned())
        .collect();
    for (alias, arm) in &donor_exes {
        if !catalog_exes.contains(alias) {
            drift.push(format!(
                "donor agent alias \"{alias}\" not in catalog ({AGENT_LOOKUP_FILE}:{arm})"
            ));
        }
    }
    for alias in &catalog_exes {
        if !donor_exes.iter().any(|(a, _)| a == alias) {
            drift.push(format!(
                "catalog executable \"{alias}\" not mapped by donor {AGENT_LOOKUP_FN}"
            ));
        }
    }

    let catalog_manifests: BTreeSet<String> = defs
        .iter()
        .filter_map(|d| d.lifecycle.detection_manifest.clone())
        .collect();
    match fs::read_dir(donor.join(MANIFEST_DIR)) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(stem) = name.strip_suffix(".toml")
                    && !catalog_manifests.contains(stem)
                {
                    drift.push(format!("donor manifest {stem} not in catalog"));
                }
            }
        }
        Err(_) => drift.push(format!("missing directory {MANIFEST_DIR}")),
    }
    drift
}

/// Quoted, whitespace-free aliases from the `=>` arms of the donor's `lookup_agent`, paired
/// with the 1-based line of the arm.
fn lookup_agent_aliases(text: &str) -> Vec<(String, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|l| l.starts_with(AGENT_LOOKUP_FN)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if *line == "}" {
            break;
        }
        if !line.contains("=>") {
            continue;
        }
        let arm_pattern = line.split("=>").next().unwrap_or("");
        for alias in arm_pattern.split('"').skip(1).step_by(2) {
            if !alias.is_empty() && !alias.contains(char::is_whitespace) {
                out.push((alias.to_string(), i + 1));
            }
        }
    }
    out
}
