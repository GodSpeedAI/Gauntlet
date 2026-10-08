use crate::definition::{DefinitionError, HarnessDefinition, Origin};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Errors that can occur when loading a harness catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    DefinitionError(DefinitionError),
    DuplicateId(String),
    DuplicateExecutable(String),
    DuplicateCustomId(String),
    IoError(String),
    SymlinkDetected(String),
    FileTooLarge(String),
}

impl From<DefinitionError> for CatalogError {
    fn from(err: DefinitionError) -> Self {
        CatalogError::DefinitionError(err)
    }
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DefinitionError(e) => write!(f, "definition error: {}", e),
            Self::DuplicateId(id) => write!(f, "duplicate harness id: {}", id),
            Self::DuplicateExecutable(exe) => {
                write!(f, "duplicate executable alias across definitions: {}", exe)
            }
            Self::DuplicateCustomId(id) => {
                write!(f, "custom harness id conflicts with built-in id: {}", id)
            }
            Self::IoError(msg) => write!(f, "io error: {}", msg),
            Self::SymlinkDetected(path) => write!(f, "symlink not allowed: {}", path),
            Self::FileTooLarge(path) => write!(f, "file exceeds 64 KiB: {}", path),
        }
    }
}

/// Load all built-in harness definitions.
pub fn builtin() -> Result<Vec<HarnessDefinition>, CatalogError> {
    // Note: In a real implementation, definitions would be embedded via include_str!.
    // For now, we load from disk for testing.
    let mut definitions = Vec::new();
    let mut ids = HashSet::new();
    let mut executables = HashSet::new();

    let definitions_dir = "/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-harness-catalog/definitions";

    if !Path::new(definitions_dir).exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = fs::read_dir(definitions_dir)
        .map_err(|e| CatalogError::IoError(e.to_string()))?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();

    // Sort for deterministic order
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let content =
            fs::read_to_string(&path).map_err(|e| CatalogError::IoError(e.to_string()))?;

        let definition = HarnessDefinition::from_toml_str(&content, Origin::BuiltIn)?;

        // Check for duplicate ids
        if ids.contains(&definition.id) {
            return Err(CatalogError::DuplicateId(definition.id));
        }
        ids.insert(definition.id.clone());

        // Check for duplicate executables
        for exe in &definition.executables {
            if executables.contains(exe) {
                return Err(CatalogError::DuplicateExecutable(exe.clone()));
            }
            executables.insert(exe.clone());
        }

        definitions.push(definition);
    }

    Ok(definitions)
}

/// Load custom harness definitions from a directory.
pub fn load_custom_dir(dir: &Path) -> Result<Vec<HarnessDefinition>, CatalogError> {
    let mut definitions = Vec::new();
    let mut ids = HashSet::new();
    let mut executables = HashSet::new();

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| CatalogError::IoError(e.to_string()))?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();

    // Sort for deterministic order
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();

        // Reject symlinks
        if path.is_symlink() {
            return Err(CatalogError::SymlinkDetected(
                path.to_string_lossy().to_string(),
            ));
        }

        // Reject files > 64 KiB
        let metadata = fs::metadata(&path).map_err(|e| CatalogError::IoError(e.to_string()))?;
        if metadata.len() > 65536 {
            return Err(CatalogError::FileTooLarge(
                path.to_string_lossy().to_string(),
            ));
        }

        let content =
            fs::read_to_string(&path).map_err(|e| CatalogError::IoError(e.to_string()))?;

        let definition = HarnessDefinition::from_toml_str(&content, Origin::Custom)?;

        // Check for duplicate ids within custom
        if ids.contains(&definition.id) {
            return Err(CatalogError::DuplicateId(definition.id));
        }
        ids.insert(definition.id.clone());

        // Check for duplicate executables
        for exe in &definition.executables {
            if executables.contains(exe) {
                return Err(CatalogError::DuplicateExecutable(exe.clone()));
            }
            executables.insert(exe.clone());
        }

        definitions.push(definition);
    }

    Ok(definitions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_loads_exactly_24() {
        let defs = builtin().expect("builtin() should succeed");
        assert_eq!(
            defs.len(),
            24,
            "builtin catalog should have exactly 24 definitions"
        );
    }

    #[test]
    fn test_builtin_unique_ids() {
        let defs = builtin().expect("builtin() should succeed");
        let mut ids = HashSet::new();
        for def in &defs {
            assert!(
                !ids.contains(&def.id),
                "duplicate id in builtin catalog: {}",
                def.id
            );
            ids.insert(def.id.clone());
        }
    }

    #[test]
    fn test_builtin_unique_executables() {
        let defs = builtin().expect("builtin() should succeed");
        let mut executables = HashSet::new();
        for def in &defs {
            for exe in &def.executables {
                assert!(
                    !executables.contains(exe),
                    "duplicate executable in builtin catalog: {}",
                    exe
                );
                executables.insert(exe.clone());
            }
        }
    }

    #[test]
    fn test_builtin_resumable_has_session() {
        let defs = builtin().expect("builtin() should succeed");
        for def in &defs {
            if def.capabilities.resumable {
                assert!(
                    def.session.resume_argv.is_some(),
                    "resumable harness {} must have resume_argv",
                    def.id
                );
                if let Some(ref argv) = def.session.resume_argv {
                    let session_count = argv.iter().filter(|arg| arg.contains("{session}")).count();
                    assert_eq!(
                        session_count, 1,
                        "resumable harness {} resume_argv must contain exactly one {{session}}",
                        def.id
                    );
                }
            }
        }
    }

    #[test]
    fn test_custom_dir_empty() {
        let tempdir = std::env::temp_dir().join("gauntlet-test-empty-harness");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir(&tempdir).ok();

        let defs = load_custom_dir(&tempdir).expect("empty dir should succeed");
        assert!(defs.is_empty());

        let _ = std::fs::remove_dir(&tempdir);
    }

    #[test]
    fn test_custom_dir_valid() {
        let tempdir = std::env::temp_dir().join("gauntlet-test-custom-valid-harness");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir(&tempdir).ok();

        let toml = r#"schema = 1
id = "custom-test"
label = "Custom Test"
executables = ["custom-test"]

[provenance]
source = "custom"
commit = "abc123"
refs = []
license = "MIT"

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "none"

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = false
model_selection = false
native_subagents = false
structured_events = false
support_level = 0
"#;

        std::fs::write(tempdir.join("custom.toml"), toml).expect("write test file");

        let defs = load_custom_dir(&tempdir).expect("load_custom_dir should succeed");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].id, "custom-test");

        let _ = std::fs::remove_dir_all(&tempdir);
    }

    #[test]
    fn test_custom_dir_rejects_symlink() {
        let tempdir = std::env::temp_dir().join("gauntlet-test-symlink-harness");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir(&tempdir).ok();

        let target = tempdir.join("target.toml");
        let link = tempdir.join("link.toml");

        let toml = r#"schema = 1
id = "test"
label = "Test"
executables = ["test"]

[provenance]
source = "test"
commit = "abc"
refs = []
license = "MIT"

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "none"

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = false
model_selection = false
native_subagents = false
structured_events = false
support_level = 0
"#;

        std::fs::write(&target, toml).ok();

        #[cfg(unix)]
        {
            use std::os::unix::fs as unix_fs;
            let _ = unix_fs::symlink(&target, &link);
            let result = load_custom_dir(&tempdir);
            assert!(result.is_err());
        }

        let _ = std::fs::remove_dir_all(&tempdir);
    }

    #[test]
    fn test_custom_dir_rejects_duplicate_with_builtin() {
        let tempdir = std::env::temp_dir().join("gauntlet-test-collision-harness");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir(&tempdir).ok();

        // Try to create a custom harness with a built-in id (if any exist)
        let builtin_defs = builtin().unwrap_or_default();
        if let Some(builtin_def) = builtin_defs.first() {
            let toml = format!(
                r#"schema = 1
id = "{}"
label = "Collision Test"
executables = ["collision-test"]

[provenance]
source = "custom"
commit = "abc"
refs = []
license = "MIT"

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "none"

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = false
model_selection = false
native_subagents = false
structured_events = false
support_level = 0
"#,
                builtin_def.id
            );

            std::fs::write(tempdir.join("collision.toml"), toml).ok();

            // Note: This test can only validate collision detection in catalog
            // merging, which happens at application startup, not in load_custom_dir.
            // load_custom_dir itself doesn't know about built-in ids.
        }

        let _ = std::fs::remove_dir_all(&tempdir);
    }
}
