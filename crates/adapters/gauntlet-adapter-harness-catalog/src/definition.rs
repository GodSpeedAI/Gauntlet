use crate::subset_toml::{ParseError, Parser, Table, Value};
use std::fmt;

/// Origin of a harness definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    BuiltIn,
    Custom,
}

/// Support level indicating the sophistication of the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupportLevel {
    Process = 0,
    Interactive = 1,
    Session = 2,
    Structured = 3,
}

impl fmt::Display for SupportLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process => write!(f, "process"),
            Self::Interactive => write!(f, "interactive"),
            Self::Session => write!(f, "session"),
            Self::Structured => write!(f, "structured"),
        }
    }
}

/// Session identity type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIdentity {
    Id,
    Path,
    None,
}

impl fmt::Display for SessionIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id => write!(f, "id"),
            Self::Path => write!(f, "path"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Prompt input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    ArgvTail,
    Flag,
    Stdin,
    PtyPaste,
    Unknown,
}

impl fmt::Display for PromptMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArgvTail => write!(f, "argv-tail"),
            Self::Flag => write!(f, "flag"),
            Self::Stdin => write!(f, "stdin"),
            Self::PtyPaste => write!(f, "pty-paste"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Lifecycle source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleSource {
    Structured,
    Process,
    Screen,
    None,
}

impl fmt::Display for LifecycleSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structured => write!(f, "structured"),
            Self::Process => write!(f, "process"),
            Self::Screen => write!(f, "screen"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Interrupt signal kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptKind {
    Sigint,
    CtrlC,
    Esc,
    None,
}

impl fmt::Display for InterruptKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sigint => write!(f, "sigint"),
            Self::CtrlC => write!(f, "ctrl-c"),
            Self::Esc => write!(f, "esc"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Optional provenance metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub source: String,
    pub commit: String,
    pub refs: Vec<String>,
    pub license: String,
}

/// Launch configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub args: Vec<String>,
    pub model_arg: Option<Vec<String>>,
    pub prompt_mode: PromptMode,
    pub prompt_flag: Option<String>,
}

/// Session management configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub identity: SessionIdentity,
    pub resume_argv: Option<Vec<String>>,
}

/// Lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lifecycle {
    pub source: LifecycleSource,
    pub detection_manifest: Option<String>,
}

/// Control behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub interrupt: InterruptKind,
}

/// Capabilities and support level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub interactive: bool,
    pub resumable: bool,
    pub model_selection: bool,
    pub native_subagents: bool,
    pub structured_events: bool,
    pub support_level: SupportLevel,
}

/// Parsed and validated harness definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessDefinition {
    pub id: String,
    pub label: String,
    pub executables: Vec<String>,
    pub provenance: Option<Provenance>,
    pub launch: Launch,
    pub session: Session,
    pub lifecycle: Lifecycle,
    pub control: Control,
    pub capabilities: Capabilities,
}

/// Definition error with details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionError {
    ParseError(ParseError),
    MissingField(String),
    InvalidField(String),
    InvalidSchema,
    InvalidId,
    InvalidExecutables,
    InvalidPlaceholder(String),
    DuplicateExecutableInResume,
    DuplicateExecutableInLaunchArgs,
    DuplicateExecutableInModelArg,
    MissingSessionInResumeArgv,
    MultipleSessionInResumeArgv,
    ResumableAgreement,
    ModelArgAgreement,
    PromptModeJustification,
    ResumableJustification,
    LifecycleJustification,
    SupportLevel4Illegal,
    CustomSupportLevelTooHigh,
    CustomLifecycleStructured,
    PromptFlagWithoutMode,
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError(e) => write!(f, "parse error at line {}: {}", e.line, e.kind),
            Self::MissingField(field) => write!(f, "missing required field: {}", field),
            Self::InvalidField(field) => write!(f, "invalid field: {}", field),
            Self::InvalidSchema => write!(f, "schema must equal 1"),
            Self::InvalidId => write!(f, "id must match [a-z][a-z0-9-]{{0,31}}"),
            Self::InvalidExecutables => {
                write!(f, "executables must be non-empty array of valid basenames")
            }
            Self::InvalidPlaceholder(ph) => write!(f, "invalid placeholder usage: {}", ph),
            Self::DuplicateExecutableInResume => write!(f, "executable cannot be in resume_argv"),
            Self::DuplicateExecutableInLaunchArgs => {
                write!(f, "executable cannot be in launch.args")
            }
            Self::DuplicateExecutableInModelArg => {
                write!(f, "executable cannot be in launch.model_arg")
            }
            Self::MissingSessionInResumeArgv => {
                write!(f, "resume_argv must contain exactly one {{session}}")
            }
            Self::MultipleSessionInResumeArgv => {
                write!(f, "resume_argv must contain exactly one {{session}}")
            }
            Self::ResumableAgreement => write!(
                f,
                "capabilities.resumable must agree with resume_argv presence"
            ),
            Self::ModelArgAgreement => write!(
                f,
                "capabilities.model_selection must be true when model_arg is present"
            ),
            Self::PromptModeJustification => {
                write!(f, "support_level 1+ requires prompt_mode != unknown")
            }
            Self::ResumableJustification => write!(f, "support_level 2+ requires resumable"),
            Self::LifecycleJustification => {
                write!(f, "support_level 3+ requires lifecycle.source = structured")
            }
            Self::SupportLevel4Illegal => write!(f, "support_level 4 is illegal in definitions"),
            Self::CustomSupportLevelTooHigh => {
                write!(f, "custom origins may not exceed support_level 2")
            }
            Self::CustomLifecycleStructured => write!(
                f,
                "custom origins may not have lifecycle.source = structured"
            ),
            Self::PromptFlagWithoutMode => {
                write!(f, "prompt_flag only allowed with prompt_mode = flag")
            }
        }
    }
}

pub type DefinitionResult<T> = Result<T, DefinitionError>;

impl HarnessDefinition {
    /// Parse and validate a harness definition from TOML string.
    pub fn from_toml_str(input: &str, origin: Origin) -> DefinitionResult<Self> {
        let parser = Parser::new(input).map_err(DefinitionError::ParseError)?;
        let table = parser.parse().map_err(DefinitionError::ParseError)?;

        Self::from_table(&table, origin)
    }

    fn from_table(table: &Table, origin: Origin) -> DefinitionResult<Self> {
        // Validate schema = 1
        let schema = table
            .get("schema")
            .ok_or(DefinitionError::MissingField("schema".to_string()))?;
        if !matches!(schema, Value::Int(1)) {
            return Err(DefinitionError::InvalidSchema);
        }

        // Extract and validate id
        let id = Self::extract_string(table, "id")?;
        Self::validate_id(&id)?;

        // Extract label
        let label = Self::extract_string(table, "label")?;

        // Extract executables
        let executables = Self::extract_executables(table)?;

        // Extract optional provenance
        let provenance = Self::extract_provenance(table)?;

        // Extract launch config
        let launch = Self::extract_launch(table, &executables)?;

        // Extract session config
        let session = Self::extract_session(table, &executables)?;

        // Extract lifecycle
        let lifecycle = Self::extract_lifecycle(table)?;

        // Extract control
        let control = Self::extract_control(table)?;

        // Extract capabilities
        let capabilities = Self::extract_capabilities(table)?;

        // Validate resumable agreement
        let has_resume = session.resume_argv.is_some();
        if has_resume != capabilities.resumable {
            return Err(DefinitionError::ResumableAgreement);
        }

        // Validate model_arg agreement
        let has_model_arg = launch.model_arg.is_some();
        if has_model_arg && !capabilities.model_selection {
            return Err(DefinitionError::ModelArgAgreement);
        }

        // Validate support level justifications
        Self::validate_support_level(
            capabilities.support_level,
            launch.prompt_mode,
            capabilities.resumable,
            lifecycle.source,
            origin,
        )?;

        // Validate resume_argv if present
        if let Some(ref resume_argv) = session.resume_argv {
            Self::validate_resume_argv(resume_argv, &executables, session.identity)?;
        }

        // Validate custom origin restrictions
        if origin == Origin::Custom {
            if capabilities.support_level > SupportLevel::Session {
                return Err(DefinitionError::CustomSupportLevelTooHigh);
            }
            if lifecycle.source == LifecycleSource::Structured {
                return Err(DefinitionError::CustomLifecycleStructured);
            }
        }

        Ok(HarnessDefinition {
            id,
            label,
            executables,
            provenance,
            launch,
            session,
            lifecycle,
            control,
            capabilities,
        })
    }

    fn extract_string(table: &Table, key: &str) -> DefinitionResult<String> {
        match table.get(key) {
            Some(Value::Str(s)) => Ok(s.clone()),
            Some(_) => Err(DefinitionError::InvalidField(key.to_string())),
            None => Err(DefinitionError::MissingField(key.to_string())),
        }
    }

    fn validate_id(id: &str) -> DefinitionResult<()> {
        if id.is_empty() || id.len() > 32 {
            return Err(DefinitionError::InvalidId);
        }

        let first_char = id.chars().next().unwrap();
        if !first_char.is_ascii_lowercase() {
            return Err(DefinitionError::InvalidId);
        }

        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(DefinitionError::InvalidId);
        }

        Ok(())
    }

    fn extract_executables(table: &Table) -> DefinitionResult<Vec<String>> {
        match table.get("executables") {
            Some(Value::Array(arr)) => {
                if arr.is_empty() {
                    return Err(DefinitionError::InvalidExecutables);
                }

                let mut executables = Vec::new();
                for val in arr {
                    if let Value::Str(s) = val {
                        Self::validate_executable(s)?;
                        executables.push(s.clone());
                    } else {
                        return Err(DefinitionError::InvalidExecutables);
                    }
                }

                Ok(executables)
            }
            _ => Err(DefinitionError::MissingField("executables".to_string())),
        }
    }

    fn validate_executable(exe: &str) -> DefinitionResult<()> {
        if exe.is_empty() {
            return Err(DefinitionError::InvalidExecutables);
        }

        // No path separators
        if exe.contains('/') || exe.contains('\\') {
            return Err(DefinitionError::InvalidExecutables);
        }

        // No whitespace
        if exe.chars().any(|c| c.is_whitespace()) {
            return Err(DefinitionError::InvalidExecutables);
        }

        // No shell metacharacters
        if exe.chars().any(|c| {
            matches!(
                c,
                '|' | '&' | ';' | '<' | '>' | '$' | '(' | ')' | '{' | '}' | '*' | '?'
            )
        }) {
            return Err(DefinitionError::InvalidExecutables);
        }

        // No NUL
        if exe.contains('\0') {
            return Err(DefinitionError::InvalidExecutables);
        }

        Ok(())
    }

    fn extract_provenance(table: &Table) -> DefinitionResult<Option<Provenance>> {
        match table.get("provenance") {
            Some(Value::Table(t)) => {
                let source = Self::extract_string(t, "source")?;
                let commit = Self::extract_string(t, "commit")?;
                let license = Self::extract_string(t, "license")?;

                let refs = match t.get("refs") {
                    Some(Value::Array(arr)) => {
                        let mut ref_vec = Vec::new();
                        for val in arr {
                            if let Value::Str(s) = val {
                                ref_vec.push(s.clone());
                            } else {
                                return Err(DefinitionError::InvalidField(
                                    "provenance.refs".to_string(),
                                ));
                            }
                        }
                        ref_vec
                    }
                    None => Vec::new(),
                    _ => return Err(DefinitionError::InvalidField("provenance.refs".to_string())),
                };

                Ok(Some(Provenance {
                    source,
                    commit,
                    refs,
                    license,
                }))
            }
            None => Ok(None),
            _ => Err(DefinitionError::InvalidField("provenance".to_string())),
        }
    }

    fn extract_launch(table: &Table, executables: &[String]) -> DefinitionResult<Launch> {
        let launch_table = match table.get("launch") {
            Some(Value::Table(t)) => t,
            None => return Err(DefinitionError::MissingField("launch".to_string())),
            _ => return Err(DefinitionError::InvalidField("launch".to_string())),
        };

        // Extract args
        let args = match launch_table.get("args") {
            Some(Value::Array(arr)) => {
                let mut arg_vec = Vec::new();
                for val in arr {
                    if let Value::Str(s) = val {
                        arg_vec.push(s.clone());
                    } else {
                        return Err(DefinitionError::InvalidField("launch.args".to_string()));
                    }
                }
                arg_vec
            }
            None => Vec::new(),
            _ => return Err(DefinitionError::InvalidField("launch.args".to_string())),
        };

        // Check args don't contain executables
        for exe in executables {
            if args.iter().any(|a| a == exe) {
                return Err(DefinitionError::DuplicateExecutableInLaunchArgs);
            }
        }

        // Validate placeholders in args
        for arg in &args {
            Self::validate_placeholder(arg)?;
        }

        // Extract model_arg
        let model_arg = match launch_table.get("model_arg") {
            Some(Value::Array(arr)) => {
                let mut arg_vec = Vec::new();
                for val in arr {
                    if let Value::Str(s) = val {
                        arg_vec.push(s.clone());
                    } else {
                        return Err(DefinitionError::InvalidField(
                            "launch.model_arg".to_string(),
                        ));
                    }
                }
                // Validate model_arg contains exactly one {model}
                let model_count = arg_vec.iter().filter(|a| a.contains("{model}")).count();
                if model_count != 1 {
                    return Err(DefinitionError::InvalidPlaceholder(
                        "model_arg must contain exactly one {model}".to_string(),
                    ));
                }
                // Check doesn't contain executables
                for exe in executables {
                    if arg_vec.iter().any(|a| a == exe) {
                        return Err(DefinitionError::DuplicateExecutableInModelArg);
                    }
                }
                // Validate placeholders
                for arg in &arg_vec {
                    Self::validate_placeholder(arg)?;
                }
                Some(arg_vec)
            }
            None => None,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "launch.model_arg".to_string(),
                ));
            }
        };

        // Extract prompt_mode
        let prompt_mode = match launch_table.get("prompt_mode") {
            Some(Value::Str(s)) => match s.as_str() {
                "argv-tail" => PromptMode::ArgvTail,
                "flag" => PromptMode::Flag,
                "stdin" => PromptMode::Stdin,
                "pty-paste" => PromptMode::PtyPaste,
                "unknown" => PromptMode::Unknown,
                _ => {
                    return Err(DefinitionError::InvalidField(
                        "launch.prompt_mode".to_string(),
                    ));
                }
            },
            None => {
                return Err(DefinitionError::MissingField(
                    "launch.prompt_mode".to_string(),
                ));
            }
            _ => {
                return Err(DefinitionError::InvalidField(
                    "launch.prompt_mode".to_string(),
                ));
            }
        };

        // Extract prompt_flag
        let prompt_flag = match launch_table.get("prompt_flag") {
            Some(Value::Str(s)) => Some(s.clone()),
            None => None,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "launch.prompt_flag".to_string(),
                ));
            }
        };

        // Validate prompt_flag only with flag mode
        if prompt_flag.is_some() && prompt_mode != PromptMode::Flag {
            return Err(DefinitionError::PromptFlagWithoutMode);
        }

        Ok(Launch {
            args,
            model_arg,
            prompt_mode,
            prompt_flag,
        })
    }

    fn extract_session(table: &Table, executables: &[String]) -> DefinitionResult<Session> {
        let session_table = match table.get("session") {
            Some(Value::Table(t)) => t,
            None => return Err(DefinitionError::MissingField("session".to_string())),
            _ => return Err(DefinitionError::InvalidField("session".to_string())),
        };

        // Extract identity
        let identity = match session_table.get("identity") {
            Some(Value::Str(s)) => match s.as_str() {
                "id" => SessionIdentity::Id,
                "path" => SessionIdentity::Path,
                "none" => SessionIdentity::None,
                _ => {
                    return Err(DefinitionError::InvalidField(
                        "session.identity".to_string(),
                    ));
                }
            },
            None => {
                return Err(DefinitionError::MissingField(
                    "session.identity".to_string(),
                ));
            }
            _ => {
                return Err(DefinitionError::InvalidField(
                    "session.identity".to_string(),
                ));
            }
        };

        // Extract resume_argv
        let resume_argv = match session_table.get("resume_argv") {
            Some(Value::Array(arr)) => {
                let mut arg_vec = Vec::new();
                for val in arr {
                    if let Value::Str(s) = val {
                        arg_vec.push(s.clone());
                    } else {
                        return Err(DefinitionError::InvalidField(
                            "session.resume_argv".to_string(),
                        ));
                    }
                }

                // Check doesn't contain executables
                for exe in executables {
                    if arg_vec.iter().any(|a| a == exe) {
                        return Err(DefinitionError::DuplicateExecutableInResume);
                    }
                }

                // Validate placeholders
                for arg in &arg_vec {
                    Self::validate_placeholder(arg)?;
                }

                Some(arg_vec)
            }
            None => None,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "session.resume_argv".to_string(),
                ));
            }
        };

        Ok(Session {
            identity,
            resume_argv,
        })
    }

    fn extract_lifecycle(table: &Table) -> DefinitionResult<Lifecycle> {
        let lifecycle_table = match table.get("lifecycle") {
            Some(Value::Table(t)) => t,
            None => return Err(DefinitionError::MissingField("lifecycle".to_string())),
            _ => return Err(DefinitionError::InvalidField("lifecycle".to_string())),
        };

        // Extract source
        let source = match lifecycle_table.get("source") {
            Some(Value::Str(s)) => match s.as_str() {
                "structured" => LifecycleSource::Structured,
                "process" => LifecycleSource::Process,
                "screen" => LifecycleSource::Screen,
                "none" => LifecycleSource::None,
                _ => {
                    return Err(DefinitionError::InvalidField(
                        "lifecycle.source".to_string(),
                    ));
                }
            },
            None => {
                return Err(DefinitionError::MissingField(
                    "lifecycle.source".to_string(),
                ));
            }
            _ => {
                return Err(DefinitionError::InvalidField(
                    "lifecycle.source".to_string(),
                ));
            }
        };

        // Extract detection_manifest
        let detection_manifest = match lifecycle_table.get("detection_manifest") {
            Some(Value::Str(s)) => Some(s.clone()),
            None => None,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "lifecycle.detection_manifest".to_string(),
                ));
            }
        };

        Ok(Lifecycle {
            source,
            detection_manifest,
        })
    }

    fn extract_control(table: &Table) -> DefinitionResult<Control> {
        let control_table = match table.get("control") {
            Some(Value::Table(t)) => t,
            None => return Err(DefinitionError::MissingField("control".to_string())),
            _ => return Err(DefinitionError::InvalidField("control".to_string())),
        };

        // Extract interrupt
        let interrupt = match control_table.get("interrupt") {
            Some(Value::Str(s)) => match s.as_str() {
                "sigint" => InterruptKind::Sigint,
                "ctrl-c" => InterruptKind::CtrlC,
                "esc" => InterruptKind::Esc,
                "none" => InterruptKind::None,
                _ => {
                    return Err(DefinitionError::InvalidField(
                        "control.interrupt".to_string(),
                    ));
                }
            },
            None => {
                return Err(DefinitionError::MissingField(
                    "control.interrupt".to_string(),
                ));
            }
            _ => {
                return Err(DefinitionError::InvalidField(
                    "control.interrupt".to_string(),
                ));
            }
        };

        Ok(Control { interrupt })
    }

    fn extract_capabilities(table: &Table) -> DefinitionResult<Capabilities> {
        let cap_table = match table.get("capabilities") {
            Some(Value::Table(t)) => t,
            None => return Err(DefinitionError::MissingField("capabilities".to_string())),
            _ => return Err(DefinitionError::InvalidField("capabilities".to_string())),
        };

        let interactive = match cap_table.get("interactive") {
            Some(Value::Bool(b)) => *b,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "capabilities.interactive".to_string(),
                ));
            }
        };

        let resumable = match cap_table.get("resumable") {
            Some(Value::Bool(b)) => *b,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "capabilities.resumable".to_string(),
                ));
            }
        };

        let model_selection = match cap_table.get("model_selection") {
            Some(Value::Bool(b)) => *b,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "capabilities.model_selection".to_string(),
                ));
            }
        };

        let native_subagents = match cap_table.get("native_subagents") {
            Some(Value::Bool(b)) => *b,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "capabilities.native_subagents".to_string(),
                ));
            }
        };

        let structured_events = match cap_table.get("structured_events") {
            Some(Value::Bool(b)) => *b,
            _ => {
                return Err(DefinitionError::InvalidField(
                    "capabilities.structured_events".to_string(),
                ));
            }
        };

        let support_level = match cap_table.get("support_level") {
            Some(Value::Int(i)) => match i {
                0 => SupportLevel::Process,
                1 => SupportLevel::Interactive,
                2 => SupportLevel::Session,
                3 => SupportLevel::Structured,
                _ => {
                    return Err(DefinitionError::InvalidField(
                        "capabilities.support_level".to_string(),
                    ));
                }
            },
            _ => {
                return Err(DefinitionError::InvalidField(
                    "capabilities.support_level".to_string(),
                ));
            }
        };

        Ok(Capabilities {
            interactive,
            resumable,
            model_selection,
            native_subagents,
            structured_events,
            support_level,
        })
    }

    fn validate_placeholder(s: &str) -> DefinitionResult<()> {
        // Check for any braces - if present, must be known placeholders
        if !s.contains('{') && !s.contains('}') {
            // No braces at all, OK
            return Ok(());
        }

        // If braces are present, check they're only for known placeholders
        let has_model = s.contains("{model}");
        let has_session = s.contains("{session}");
        let has_prompt = s.contains("{prompt}");

        // Count known placeholders
        let known_placeholders = (has_model as i32) + (has_session as i32) + (has_prompt as i32);

        // If braces but no known placeholders, reject (e.g., {unknown})
        if known_placeholders == 0 {
            return Err(DefinitionError::InvalidPlaceholder(s.to_string()));
        }

        // Legal forms: entire element or --flag={placeholder}
        if s.starts_with("--") && s.contains('=') {
            // Format: --[a-z][a-z0-9-]*={placeholder}
            let parts: Vec<&str> = s.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(DefinitionError::InvalidPlaceholder(s.to_string()));
            }

            let flag_part = parts[0];
            let value_part = parts[1];

            // Validate flag part: --[a-z][a-z0-9-]*
            if !flag_part.starts_with("--") {
                return Err(DefinitionError::InvalidPlaceholder(s.to_string()));
            }

            let flag_name = &flag_part[2..];
            if flag_name.is_empty() {
                return Err(DefinitionError::InvalidPlaceholder(s.to_string()));
            }

            let first_char = flag_name.chars().next().unwrap();
            if !first_char.is_ascii_lowercase() {
                return Err(DefinitionError::InvalidPlaceholder(s.to_string()));
            }

            if !flag_name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(DefinitionError::InvalidPlaceholder(s.to_string()));
            }

            // Value must be exactly a placeholder (no extra text)
            if value_part != "{model}" && value_part != "{session}" && value_part != "{prompt}" {
                return Err(DefinitionError::InvalidPlaceholder(s.to_string()));
            }

            Ok(())
        } else {
            // Entire element must be a placeholder
            if s == "{model}" || s == "{session}" || s == "{prompt}" {
                Ok(())
            } else {
                Err(DefinitionError::InvalidPlaceholder(s.to_string()))
            }
        }
    }

    fn validate_resume_argv(
        resume_argv: &[String],
        _executables: &[String],
        identity: SessionIdentity,
    ) -> DefinitionResult<()> {
        // Must require session.identity != "none"
        if identity == SessionIdentity::None {
            return Err(DefinitionError::MissingField(
                "resume_argv requires identity != none".to_string(),
            ));
        }

        // Must contain exactly one {session}
        let session_count = resume_argv
            .iter()
            .filter(|a| a.contains("{session}"))
            .count();

        if session_count == 0 {
            return Err(DefinitionError::MissingSessionInResumeArgv);
        }

        if session_count > 1 {
            return Err(DefinitionError::MultipleSessionInResumeArgv);
        }

        Ok(())
    }

    fn validate_support_level(
        level: SupportLevel,
        prompt_mode: PromptMode,
        resumable: bool,
        lifecycle_source: LifecycleSource,
        _origin: Origin,
    ) -> DefinitionResult<()> {
        match level {
            SupportLevel::Process => Ok(()),
            SupportLevel::Interactive => {
                // Needs prompt_mode != unknown
                if prompt_mode == PromptMode::Unknown {
                    Err(DefinitionError::PromptModeJustification)
                } else {
                    Ok(())
                }
            }
            SupportLevel::Session => {
                // Needs resumable
                if !resumable {
                    Err(DefinitionError::ResumableJustification)
                } else {
                    Ok(())
                }
            }
            SupportLevel::Structured => {
                // Needs lifecycle.source = structured
                if lifecycle_source != LifecycleSource::Structured {
                    Err(DefinitionError::LifecycleJustification)
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_valid_toml() -> &'static str {
        r#"schema = 1
id = "test"
label = "Test Harness"
executables = ["test"]

[provenance]
source = "herdr"
commit = "abc123"
refs = []
license = "Apache-2.0"

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
"#
    }

    #[test]
    fn test_parse_valid_builtin() {
        let result = HarnessDefinition::from_toml_str(minimal_valid_toml(), Origin::BuiltIn);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_invalid_schema() {
        let toml = minimal_valid_toml().replace("schema = 1", "schema = 2");
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(result, Err(DefinitionError::InvalidSchema)));
    }

    #[test]
    fn test_reject_invalid_id_uppercase() {
        let toml = minimal_valid_toml().replace(r#"id = "test""#, r#"id = "Test""#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(result, Err(DefinitionError::InvalidId)));
    }

    #[test]
    fn test_reject_id_too_long() {
        let long_id = "a".repeat(33);
        let toml =
            minimal_valid_toml().replace(r#"id = "test""#, &format!(r#"id = "{}""#, long_id));
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(result, Err(DefinitionError::InvalidId)));
    }

    #[test]
    fn test_reject_executable_with_slash() {
        let toml = minimal_valid_toml()
            .replace(r#"executables = ["test"]"#, r#"executables = ["test/bin"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(result, Err(DefinitionError::InvalidExecutables)));
    }

    #[test]
    fn test_reject_executable_with_space() {
        let toml = minimal_valid_toml()
            .replace(r#"executables = ["test"]"#, r#"executables = ["test bin"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(result, Err(DefinitionError::InvalidExecutables)));
    }

    #[test]
    fn test_reject_placeholder_in_middle() {
        let toml = minimal_valid_toml().replace(r#"args = []"#, r#"args = ["cmd-{model}-end"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::InvalidPlaceholder(_))
        ));
    }

    #[test]
    fn test_accept_placeholder_as_whole_element() {
        let toml = minimal_valid_toml()
            .replace(r#"prompt_mode = "unknown""#, r#"prompt_mode = "flag""#)
            .replace(r#"support_level = 0"#, r#"support_level = 1"#)
            .replace(r#"args = []"#, r#"args = ["{prompt}"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(result.is_ok());
    }

    #[test]
    fn test_accept_placeholder_in_flag() {
        let toml = minimal_valid_toml()
            .replace(r#"prompt_mode = "unknown""#, r#"prompt_mode = "flag""#)
            .replace(r#"support_level = 0"#, r#"support_level = 1"#)
            .replace(r#"args = []"#, r#"args = ["--prompt={prompt}"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_duplicate_executable_in_resume() {
        let toml = r#"schema = 1
id = "test"
label = "Test Harness"
executables = ["myexe"]

[provenance]
source = "herdr"
commit = "abc123"
refs = []
license = "Apache-2.0"

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "id"
resume_argv = ["myexe", "--id", "{session}"]

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = true
model_selection = false
native_subagents = false
structured_events = false
support_level = 2
"#;
        let result = HarnessDefinition::from_toml_str(toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::DuplicateExecutableInResume)
        ));
    }

    #[test]
    fn test_reject_resume_argv_without_session() {
        let toml = r#"schema = 1
id = "test"
label = "Test Harness"
executables = ["test"]

[provenance]
source = "herdr"
commit = "abc123"
refs = []
license = "Apache-2.0"

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "id"
resume_argv = ["--id", "xyz"]

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = true
model_selection = false
native_subagents = false
structured_events = false
support_level = 2
"#;
        let result = HarnessDefinition::from_toml_str(toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::MissingSessionInResumeArgv)
        ));
    }

    #[test]
    fn test_reject_resumable_mismatch() {
        let toml = minimal_valid_toml().replace(r#"resumable = false"#, r#"resumable = true"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(result, Err(DefinitionError::ResumableAgreement)));
    }

    #[test]
    fn test_reject_support_level_1_unknown_prompt() {
        let toml = minimal_valid_toml().replace(r#"support_level = 0"#, r#"support_level = 1"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::PromptModeJustification)
        ));
    }

    #[test]
    fn test_reject_support_level_2_not_resumable() {
        let toml = minimal_valid_toml()
            .replace(r#"support_level = 0"#, r#"support_level = 2"#)
            .replace(r#"prompt_mode = "unknown""#, r#"prompt_mode = "stdin""#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::ResumableJustification)
        ));
    }

    #[test]
    fn test_reject_model_arg_without_selection() {
        let toml = minimal_valid_toml().replace(
            r#"args = []"#,
            r#"args = []
model_arg = ["--model", "{model}"]"#,
        );
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(result, Err(DefinitionError::ModelArgAgreement)));
    }

    #[test]
    fn test_reject_prompt_flag_without_flag_mode() {
        let toml = minimal_valid_toml().replace(
            r#"prompt_mode = "unknown""#,
            r#"prompt_mode = "stdin"
prompt_flag = "-p""#,
        );
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::PromptFlagWithoutMode)
        ));
    }

    #[test]
    fn test_reject_custom_support_level_3() {
        let toml = minimal_valid_toml()
            .replace(r#"source = "screen""#, r#"source = "structured""#)
            .replace(r#"support_level = 0"#, r#"support_level = 3"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::Custom);
        assert!(matches!(
            result,
            Err(DefinitionError::CustomSupportLevelTooHigh)
        ));
    }

    #[test]
    fn test_reject_custom_structured_lifecycle() {
        let toml = minimal_valid_toml().replace(r#"source = "screen""#, r#"source = "structured""#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::Custom);
        assert!(matches!(
            result,
            Err(DefinitionError::CustomLifecycleStructured)
        ));
    }

    #[test]
    fn test_reject_placeholder_dollar_brace_syntax() {
        let toml = minimal_valid_toml().replace(r#"args = []"#, r#"args = ["$({session})"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::InvalidPlaceholder(_))
        ));
    }

    #[test]
    fn test_reject_placeholder_fused_session_model() {
        let toml = minimal_valid_toml().replace(r#"args = []"#, r#"args = ["{session}{model}"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::InvalidPlaceholder(_))
        ));
    }

    #[test]
    fn test_reject_placeholder_unknown() {
        let toml = minimal_valid_toml().replace(r#"args = []"#, r#"args = ["{unknown}"]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::InvalidPlaceholder(_))
        ));
    }

    #[test]
    fn test_reject_shell_string_with_fused_placeholder() {
        let toml = minimal_valid_toml()
            .replace(r#"prompt_mode = "unknown""#, r#"prompt_mode = "flag""#)
            .replace(r#"support_level = 0"#, r#"support_level = 1"#)
            .replace(r#"args = []"#, r#"args = ["sh -c \"{prompt}\""]"#);
        let result = HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn);
        assert!(matches!(
            result,
            Err(DefinitionError::InvalidPlaceholder(_))
        ));
    }

    #[test]
    fn test_accept_resume_argv_with_flag_syntax() {
        let toml = r#"schema = 1
id = "test"
label = "Test Harness"
executables = ["test"]

[provenance]
source = "herdr"
commit = "abc123"
refs = []
license = "Apache-2.0"

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "id"
resume_argv = ["--resume={session}"]

[lifecycle]
source = "screen"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = true
model_selection = false
native_subagents = false
structured_events = false
support_level = 2
"#;
        let result = HarnessDefinition::from_toml_str(toml, Origin::BuiltIn);
        assert!(result.is_ok());
        if let Ok(def) = result {
            assert_eq!(
                def.session.resume_argv,
                Some(vec!["--resume={session}".to_string()])
            );
        }
    }

    #[test]
    fn test_all_definition_files_parse() {
        // This test verifies all 24 existing definition files can be parsed
        // (they may not all validate yet)
        use std::fs;
        use std::path::Path;

        let definitions_dir = "/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-harness-catalog/definitions";
        if Path::new(definitions_dir).exists() {
            for entry in fs::read_dir(definitions_dir).unwrap().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "toml")
                    && let Ok(content) = fs::read_to_string(&path)
                {
                    let parser = crate::subset_toml::Parser::new(&content);
                    assert!(parser.is_ok(), "Failed to create parser for {:?}", path);
                    if let Ok(parser) = parser {
                        let result = parser.parse();
                        assert!(result.is_ok(), "Failed to parse {:?}", path);
                    }
                }
            }
        }
    }
}
