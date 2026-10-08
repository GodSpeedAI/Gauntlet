//! Lifecycle tracking for interactive sessions.
//!
//! Lifecycle is LIVENESS/FORENSICS ONLY and can never complete an invocation.
//! It observes the session output to infer state (Working, Blocked, Idle, Done)
//! using conservative heuristics, with stale-state protection via sequence
//! tracking.

use std::collections::HashMap;

/// The liveness state of a session.
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

/// The source of a lifecycle observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleSource {
    /// Observation from structured message.
    Structured,
    /// Observation from process state.
    Process,
    /// Observation from output screen analysis.
    Screen,
}

/// One lifecycle observation with sequence tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleObservation {
    /// The observed state.
    pub state: LifecycleState,
    /// The sequence number for stale-state protection.
    pub sequence: u64,
    /// The source of the observation.
    pub source: LifecycleSource,
}

/// The result of applying an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// The observation was applied successfully.
    Applied,
    /// The observation was ignored because it was stale.
    IgnoredStale,
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

/// Tracks lifecycle state per session, with stale-state protection.
#[derive(Debug)]
pub struct LifecycleTracker {
    /// Per-session state and sequence tracking by source.
    states: HashMap<u64, HashMap<(LifecycleSource, u64), LifecycleState>>,
    /// Per-session current state.
    current: HashMap<u64, LifecycleState>,
    /// Classifier configuration.
    config: ClassifierConfig,
}

impl LifecycleTracker {
    pub fn new(config: ClassifierConfig) -> Self {
        Self {
            states: HashMap::new(),
            current: HashMap::new(),
            config,
        }
    }

    /// Applies an observation, ignoring stale ones from the same source.
    pub fn observe(&mut self, session: u64, obs: LifecycleObservation) -> Observed {
        let states_for_session = self.states.entry(session).or_default();
        let key = (obs.source, obs.sequence);

        // Check if this sequence from this source is stale
        if let Some(existing) = states_for_session.get(&key)
            && *existing == obs.state
        {
            return Observed::IgnoredStale;
        }

        // Store and update current state
        states_for_session.insert(key, obs.state);
        self.current.insert(session, obs.state);
        Observed::Applied
    }

    /// Returns the current lifecycle state for a session.
    pub fn current(&self, session: u64) -> LifecycleState {
        self.current
            .get(&session)
            .copied()
            .unwrap_or(LifecycleState::Unknown)
    }

    /// Classifies a tail of output to a lifecycle state (conservative heuristic).
    /// Strip ANSI/OSC escapes, check for idle suffix, blocked marker, or output growth.
    pub fn classify_tail(&self, bytes: &[u8], last_size: u64) -> LifecycleState {
        if bytes.is_empty() {
            return LifecycleState::Unknown;
        }

        // Strip ANSI and OSC escape sequences
        let cleaned = strip_ansi_escapes(bytes);

        // Check for idle suffixes (quiescent tail ending in prompt marker)
        for suffix in &self.config.idle_suffixes {
            if cleaned.ends_with(suffix.as_bytes()) {
                return LifecycleState::Idle;
            }
        }

        // Check for blocked markers
        for marker in &self.config.blocked_markers {
            if cleaned
                .windows(marker.len())
                .any(|w| w == marker.as_bytes())
            {
                return LifecycleState::Blocked;
            }
        }

        // If cleaned is empty or output hasn't grown, it's unknown
        if cleaned.is_empty() || bytes.len() as u64 <= last_size {
            LifecycleState::Unknown
        } else {
            // Output grew and is non-empty after cleanup
            LifecycleState::Working
        }
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
    fn lifecycle_tracker_ignores_stale_sequence() {
        let mut tracker = LifecycleTracker::new(ClassifierConfig::default());
        let session = 1;

        let obs1 = LifecycleObservation {
            state: LifecycleState::Working,
            sequence: 1,
            source: LifecycleSource::Screen,
        };

        assert_eq!(tracker.observe(session, obs1), Observed::Applied);
        assert_eq!(tracker.current(session), LifecycleState::Working);

        // Same sequence from same source -> stale
        assert_eq!(tracker.observe(session, obs1), Observed::IgnoredStale);
        assert_eq!(tracker.current(session), LifecycleState::Working);
    }

    #[test]
    fn classify_tail_detects_idle() {
        let tracker = LifecycleTracker::new(ClassifierConfig::default());
        let bytes = b"output here> ";
        let state = tracker.classify_tail(bytes, 0);
        assert_eq!(state, LifecycleState::Idle);
    }

    #[test]
    fn classify_tail_detects_blocked() {
        let tracker = LifecycleTracker::new(ClassifierConfig::default());
        let bytes = b"currently waiting for user input";
        let state = tracker.classify_tail(bytes, 0);
        assert_eq!(state, LifecycleState::Blocked);
    }

    #[test]
    fn classify_tail_ansi_codes_with_content() {
        let tracker = LifecycleTracker::new(ClassifierConfig::default());
        let bytes = b"\x1b[31mred\x1b[0m\x1b[32mgreen\x1b[0m";
        let state = tracker.classify_tail(bytes, 0);
        // Stripped output is "redgreen" which is non-empty and grew, so Working
        assert_eq!(state, LifecycleState::Working);
    }
}
