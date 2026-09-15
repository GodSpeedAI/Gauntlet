//! `gauntlet deliver [--run <run-id>] [--apply]` (plan §W5): the CLI half
//! of work-product delivery. `gauntlet run` seeds every workspace over a
//! run-scoped integration tree and the controller promotes a settled
//! unit's changes into it (`crates/adapters/gauntlet-adapter-workspace-fs`,
//! `gauntlet-app` controller `process_completion`); `deliver` is the ONLY
//! place that tree's delta ever reaches the actual project root — without
//! it a settled run's work product stays inside `<state_dir>/integration/`
//! forever.
//!
//! Without `--apply`: computes the delta of the run's integration tree
//! against its `baseline.json` (the filtered project-root snapshot taken
//! once at the run's first seeded workspace) and writes a bundle under
//! `<artifact_dir>/runs/<run>/delivery/` — `files/<path>` copies plus a
//! `manifest.json` describing what changed and what was deleted. Prints a
//! summary and the apply hint.
//!
//! With `--apply`: first verifies, for every touched path, that the
//! PROJECT file's current digest still equals its baseline digest (or is
//! absent when the baseline had none) — any drift refuses the whole apply
//! and lists the drifted paths, nothing written. Otherwise writes changed
//! files into the project root atomically and removes deleted ones.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gauntlet_adapter_workspace_fs::{self as workspace_fs, Manifest};
use gauntlet_app::config::defaults;
use gauntlet_domain::RunId;

use crate::context::{RuntimeContext, resolve_context};

/// One file this run's integration tree added or changed relative to its
/// baseline.
pub struct ChangedFile {
    pub path: String,
    pub sha256: String,
    pub executable: bool,
    pub baseline_sha256: Option<String>,
}

/// One file the baseline had that this run's integration tree no longer
/// does.
pub struct DeletedFile {
    pub path: String,
    pub baseline_sha256: String,
}

/// A run's work-product delta: everything `deliver` would write or
/// remove in the project root.
pub struct Delta {
    pub changed: Vec<ChangedFile>,
    pub deleted: Vec<DeletedFile>,
}

impl Delta {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.deleted.is_empty()
    }
}

/// `<state_dir>/integration/<run_id>/tree` and `.../baseline.json`.
fn integration_paths(context: &RuntimeContext, run_id: &RunId) -> (PathBuf, PathBuf) {
    let run_root = context
        .state_dir()
        .join(defaults::INTEGRATION_DIR_NAME)
        .join(run_id.as_str());
    (run_root.join("tree"), run_root.join("baseline.json"))
}

/// Finds the most recently touched run under `<state_dir>/integration/`
/// (by `baseline.json` modified time) — the runtime names no "list runs"
/// read surface today, so `deliver`'s own subject (the integration tree)
/// is what stands in for one when `--run` is absent.
fn most_recent_run(context: &RuntimeContext) -> Option<RunId> {
    let root = context.state_dir().join(defaults::INTEGRATION_DIR_NAME);
    let entries = std::fs::read_dir(&root).ok()?;
    let mut best: Option<(std::time::SystemTime, RunId)> = None;
    for entry in entries.flatten() {
        let Ok(run_id) = RunId::new(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        let baseline = entry.path().join("baseline.json");
        let Ok(metadata) = std::fs::metadata(&baseline) else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if best.as_ref().is_none_or(|(when, _)| modified > *when) {
            best = Some((modified, run_id));
        }
    }
    best.map(|(_, run_id)| run_id)
}

fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    workspace_fs::parse_manifest_json(&text)
        .map_err(|error| format!("parsing {}: {error}", path.display()))
}

/// Computes the delta of `tree` against `baseline` (already read).
fn compute_delta(tree: &Path, baseline: &Manifest) -> Result<Delta, String> {
    let current = workspace_fs::compute_manifest(tree, &[])
        .map_err(|error| format!("walking {}: {error}", tree.display()))?;
    let mut changed = Vec::new();
    for (path, digest) in &current {
        let baseline_digest = baseline.get(path);
        let is_changed = baseline_digest.is_none_or(|b| b.sha256 != digest.sha256);
        if is_changed {
            changed.push(ChangedFile {
                path: path.clone(),
                sha256: digest.sha256.clone(),
                executable: digest.executable,
                baseline_sha256: baseline_digest.map(|b| b.sha256.clone()),
            });
        }
    }
    let mut deleted = Vec::new();
    for (path, digest) in baseline {
        if !current.contains_key(path) {
            deleted.push(DeletedFile {
                path: path.clone(),
                baseline_sha256: digest.sha256.clone(),
            });
        }
    }
    changed.sort_by(|a, b| a.path.cmp(&b.path));
    deleted.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Delta { changed, deleted })
}

/// Whether this run has a pending, non-empty delivery — `gauntlet run`'s
/// terminal hint (never fails the run: any read trouble is "nothing to
/// report").
pub fn has_pending_delivery(context: &RuntimeContext, run_id: &RunId) -> bool {
    let (tree, baseline_path) = integration_paths(context, run_id);
    if !tree.is_dir() || !baseline_path.is_file() {
        return false;
    }
    let Ok(baseline) = read_manifest(&baseline_path) else {
        return false;
    };
    compute_delta(&tree, &baseline)
        .map(|delta| !delta.is_empty())
        .unwrap_or(false)
}

fn delivery_dir(context: &RuntimeContext, run_id: &RunId) -> PathBuf {
    context
        .resolved
        .evidence_dir
        .join(defaults::RUNS_DIR_NAME)
        .join(run_id.as_str())
        .join(defaults::DELIVERY_DIR_NAME)
}

fn json_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32))
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn write_manifest_json(path: &Path, run_id: &RunId, delta: &Delta) -> std::io::Result<()> {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"run_id\": {},\n",
        json_string(run_id.as_str())
    ));
    out.push_str("  \"changed\": [\n");
    for (index, file) in delta.changed.iter().enumerate() {
        let comma = if index + 1 < delta.changed.len() {
            ","
        } else {
            ""
        };
        let baseline = match &file.baseline_sha256 {
            Some(sha) => json_string(sha),
            None => "null".to_owned(),
        };
        out.push_str(&format!(
            "    {{\"path\": {}, \"sha256\": {}, \"baseline_sha256\": {}}}{comma}\n",
            json_string(&file.path),
            json_string(&file.sha256),
            baseline
        ));
    }
    out.push_str("  ],\n");
    out.push_str("  \"deleted\": [\n");
    for (index, file) in delta.deleted.iter().enumerate() {
        let comma = if index + 1 < delta.deleted.len() {
            ","
        } else {
            ""
        };
        out.push_str(&format!(
            "    {{\"path\": {}, \"baseline_sha256\": {}}}{comma}\n",
            json_string(&file.path),
            json_string(&file.baseline_sha256)
        ));
    }
    out.push_str("  ]\n}\n");
    std::fs::write(path, out)
}

/// Writes the delivery bundle (`files/<path>` copies + `manifest.json`)
/// under `<artifact_dir>/runs/<run>/delivery/`.
fn write_bundle(
    context: &RuntimeContext,
    run_id: &RunId,
    tree: &Path,
    delta: &Delta,
) -> Result<PathBuf, String> {
    let dir = delivery_dir(context, run_id);
    let files_dir = dir.join("files");
    std::fs::create_dir_all(&files_dir)
        .map_err(|error| format!("creating {}: {error}", files_dir.display()))?;
    for file in &delta.changed {
        let bytes = std::fs::read(tree.join(&file.path))
            .map_err(|error| format!("reading {}: {error}", file.path))?;
        workspace_fs::write_file_atomic(&files_dir.join(&file.path), &bytes, file.executable)
            .map_err(|error| format!("writing {}: {error}", file.path))?;
    }
    write_manifest_json(&dir.join("manifest.json"), run_id, delta)
        .map_err(|error| format!("writing manifest.json: {error}"))?;
    Ok(dir)
}

/// `gauntlet deliver [--run <run-id>] [--apply]`.
pub fn run(args: &[String]) -> ExitCode {
    let mut run_id_arg: Option<String> = None;
    let mut apply = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run" => match args.get(index + 1) {
                Some(value) => {
                    run_id_arg = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet deliver: --run requires a run id");
                    return ExitCode::from(2);
                }
            },
            "--apply" => {
                apply = true;
                index += 1;
            }
            other => {
                eprintln!("gauntlet deliver: unknown argument '{other}'");
                println!("usage: gauntlet deliver [--run <run-id>] [--apply]");
                return ExitCode::from(2);
            }
        }
    }
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet deliver: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let run_id = match run_id_arg {
        Some(raw) => match RunId::new(&raw) {
            Ok(run_id) => run_id,
            Err(_) => {
                eprintln!("gauntlet deliver: {raw} is not a valid run id (ULID grammar)");
                return ExitCode::FAILURE;
            }
        },
        None => match most_recent_run(&context) {
            Some(run_id) => run_id,
            None => {
                eprintln!(
                    "gauntlet deliver: no run has an integration tree under {} — nothing to \
                     deliver",
                    context
                        .state_dir()
                        .join(defaults::INTEGRATION_DIR_NAME)
                        .display()
                );
                return ExitCode::FAILURE;
            }
        },
    };
    let (tree, baseline_path) = integration_paths(&context, &run_id);
    if !tree.is_dir() || !baseline_path.is_file() {
        eprintln!(
            "gauntlet deliver: run {run_id} has no integration tree at {} — nothing to deliver",
            tree.display()
        );
        return ExitCode::FAILURE;
    }
    let baseline = match read_manifest(&baseline_path) {
        Ok(baseline) => baseline,
        Err(error) => {
            eprintln!("gauntlet deliver: {error}");
            return ExitCode::FAILURE;
        }
    };
    let delta = match compute_delta(&tree, &baseline) {
        Ok(delta) => delta,
        Err(error) => {
            eprintln!("gauntlet deliver: {error}");
            return ExitCode::FAILURE;
        }
    };
    if delta.is_empty() {
        println!("deliver: run_id={run_id} nothing changed since the baseline");
        return ExitCode::SUCCESS;
    }
    if apply {
        // Verify EVERY touched path against the project root's CURRENT
        // digest before writing anything — any drift refuses the whole
        // apply.
        let mut drifted = Vec::new();
        for file in delta
            .changed
            .iter()
            .map(|f| (f.path.as_str(), &f.baseline_sha256))
        {
            check_drift(
                &context.project_root,
                file.0,
                file.1.as_deref(),
                &mut drifted,
            );
        }
        for file in &delta.deleted {
            check_drift(
                &context.project_root,
                &file.path,
                Some(file.baseline_sha256.as_str()),
                &mut drifted,
            );
        }
        if !drifted.is_empty() {
            eprintln!(
                "gauntlet deliver: --apply refused — {} project path(s) drifted from the run's \
                 baseline since it started:",
                drifted.len()
            );
            for path in &drifted {
                eprintln!("  {path}");
            }
            return ExitCode::FAILURE;
        }
        for file in &delta.changed {
            let bytes = match std::fs::read(tree.join(&file.path)) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("gauntlet deliver: reading {}: {error}", file.path);
                    return ExitCode::FAILURE;
                }
            };
            if let Err(error) = workspace_fs::write_file_atomic(
                &context.project_root.join(&file.path),
                &bytes,
                file.executable,
            ) {
                eprintln!("gauntlet deliver: writing {}: {error}", file.path);
                return ExitCode::FAILURE;
            }
        }
        for file in &delta.deleted {
            let target = context.project_root.join(&file.path);
            if let Err(error) = std::fs::remove_file(&target)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("gauntlet deliver: removing {}: {error}", file.path);
                return ExitCode::FAILURE;
            }
        }
        println!(
            "deliver: run_id={run_id} applied changed={} deleted={}",
            delta.changed.len(),
            delta.deleted.len()
        );
    } else {
        println!(
            "deliver: run_id={run_id} changed={} deleted={} (dry run — pass --apply to write \
             into the project)",
            delta.changed.len(),
            delta.deleted.len()
        );
    }
    match write_bundle(&context, &run_id, &tree, &delta) {
        Ok(dir) => {
            println!("deliver: bundle={}", dir.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet deliver: writing the bundle failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn check_drift(
    project_root: &Path,
    path: &str,
    baseline_sha256: Option<&str>,
    drifted: &mut Vec<String>,
) {
    let current = workspace_fs::digest_file(&project_root.join(path))
        .ok()
        .flatten();
    let matches = match (&current, baseline_sha256) {
        (None, None) => true,
        (Some(digest), Some(expected)) => digest.sha256 == expected,
        _ => false,
    };
    if !matches {
        drifted.push(path.to_owned());
    }
}
