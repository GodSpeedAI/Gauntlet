//! Lifecycle classification for interactive sessions.
//!
//! Lifecycle is LIVENESS/FORENSICS ONLY and can never complete an invocation.
//! This module classifies a tail of session output into a state (Working,
//! Blocked, Idle, Unknown) with conservative heuristics.
//!
//! It assigns NO sequence. A reading leaves the adapter as a
//! [`gauntlet_ports::lifecycle::HarnessLifecycleSample`]; the Gauntlet
//! supervising layer stamps its sequence at receipt and binds it to the
//! invocation's current native session (see the `InvocationGraph` port).

/// The liveness state of a session, as read from its output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    /// Session is producing output.
    Working,
    /// Session is blocked on input.
    Blocked,
    /// Session is idle (prompt suffix detected).
    Idle,
    /// Session is done (process exited).
    Done,
    /// State cannot be determined.
    Unknown,
}

impl LifecycleState {
    /// The port-level state this reading maps to. The two enums carry the
    /// same five states; the port type is the one the graph records.
    pub fn to_port(self) -> gauntlet_ports::lifecycle::LifecycleState {
        use gauntlet_ports::lifecycle::LifecycleState as Port;
        match self {
            Self::Working => Port::Working,
            Self::Blocked => Port::Blocked,
            Self::Idle => Port::Idle,
            Self::Done => Port::Done,
            Self::Unknown => Port::Unknown,
        }
    }
}

/// Configuration for lifecycle classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifierConfig {
    /// Strings that indicate an idle/prompt state (no vendor strings).
    pub idle_suffixes: Vec<String>,
    /// Strings that indicate a blocked state (no vendor strings).
    pub blocked_markers: Vec<String>,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            idle_suffixes: vec!["> ".to_string(), "$ ".to_string(), "# ".to_string()],
            blocked_markers: vec!["waiting for".to_string(), "blocked on".to_string()],
        }
    }
}

/// Classifies tail output into a [`LifecycleState`]. Stateless: the same
/// bytes always classify the same way, and no ordering is tracked here.
#[derive(Debug)]
pub struct LifecycleClassifier {
    /// Classifier configuration.
    config: ClassifierConfig,
}

impl LifecycleClassifier {
    pub fn new(config: ClassifierConfig) -> Self {
        Self { config }
    }

    /// Classifies a tail of output (conservative heuristic). Strips ANSI and
    /// OSC escapes, then checks for an idle suffix, then a blocked marker.
    /// Non-empty visible output that matches neither is Working; empty or
    /// fully-escaped output is Unknown.
    pub fn classify_tail(&self, bytes: &[u8]) -> LifecycleState {
        if bytes.is_empty() {
            return LifecycleState::Unknown;
        }
        let cleaned = strip_ansi_escapes(bytes);
        if cleaned.is_empty() {
            return LifecycleState::Unknown;
        }
        for suffix in &self.config.idle_suffixes {
            if cleaned.ends_with(suffix.as_bytes()) {
                return LifecycleState::Idle;
            }
        }
        for marker in &self.config.blocked_markers {
            if cleaned
                .windows(marker.len())
                .any(|w| w == marker.as_bytes())
            {
                return LifecycleState::Blocked;
            }
        }
        LifecycleState::Working
    }
}

/// Remove ANSI color codes and OSC sequences from output.
fn strip_ansi_escapes(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // ESC [ ... (CSI sequences)
        if i + 1 < bytes.len() && bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
            i += 2;
            // Skip until a letter
            while i < bytes.len() && !(bytes[i].is_ascii_alphabetic()) {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // Skip the letter
            }
            continue;
        }
        // ESC ] ... (OSC sequences) — skip until BEL or ST
        if i + 1 < bytes.len() && bytes[i] == b'\x1b' && bytes[i + 1] == b']' {
            i += 2;
            loop {
                if i >= bytes.len() {
                    break;
                }
                if bytes[i] == b'\x07' {
                    // BEL
                    i += 1;
                    break;
                }
                if i + 1 < bytes.len() && bytes[i] == b'\x1b' && bytes[i + 1] == b'\\' {
                    // ST
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_escapes_removes_colors() {
        let input = b"hello \x1b[31mred\x1b[0m world";
        let output = strip_ansi_escapes(input);
        assert_eq!(output, b"hello red world");
    }

    #[test]
    fn classify_tail_detects_idle() {
        let classifier = LifecycleClassifier::new(ClassifierConfig::default());
        let state = classifier.classify_tail(b"output here> ");
        assert_eq!(state, LifecycleState::Idle);
    }

    #[test]
    fn classify_tail_detects_blocked() {
        let classifier = LifecycleClassifier::new(ClassifierConfig::default());
        let state = classifier.classify_tail(b"currently waiting for user input");
        assert_eq!(state, LifecycleState::Blocked);
    }

    #[test]
    fn classify_tail_ansi_codes_with_content() {
        let classifier = LifecycleClassifier::new(ClassifierConfig::default());
        let state = classifier.classify_tail(b"\x1b[31mred\x1b[0m\x1b[32mgreen\x1b[0m");
        // Stripped output is "redgreen": visible and matching no marker.
        assert_eq!(state, LifecycleState::Working);
    }

    #[test]
    fn classify_tail_of_empty_or_escape_only_output_is_unknown() {
        let classifier = LifecycleClassifier::new(ClassifierConfig::default());
        assert_eq!(classifier.classify_tail(b""), LifecycleState::Unknown);
        assert_eq!(
            classifier.classify_tail(b"\x1b[0m\x1b[2K"),
            LifecycleState::Unknown
        );
    }

    #[test]
    fn readings_map_one_to_one_onto_the_port_states() {
        assert_eq!(
            LifecycleState::Idle.to_port(),
            gauntlet_ports::lifecycle::LifecycleState::Idle
        );
        assert_eq!(
            LifecycleState::Working.to_port(),
            gauntlet_ports::lifecycle::LifecycleState::Working
        );
    }
}
