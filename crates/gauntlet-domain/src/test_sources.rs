//! Test-only helper for guard tests that must pin properties of this crate's
//! own source surface (AK-005's no-boolean-conversion rule; §5.3.3's frozen
//! reference rule). Reading sources keeps those pins mechanical: adding the
//! forbidden surface later fails a test, not a review.

/// Every `.rs` file under `src`, as (file name, text). Test-only; the
/// production crate never reads files.
pub(crate) fn read_all() -> Vec<(String, String)> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    let mut pending = vec![root];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).expect("src directory readable");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "rs") {
                let name = path
                    .file_name()
                    .expect("file entry has a name")
                    .to_string_lossy()
                    .to_string();
                let text = std::fs::read_to_string(&path).expect("source file readable");
                out.push((name, text));
            }
        }
    }
    out
}
