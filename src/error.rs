use std::fmt;

/// Errors that can occur during MIDI-flux bridge operations.
#[derive(Debug, Clone)]
pub enum BridgeError {
    /// MIDI byte sequence is invalid or too short.
    InvalidMidiBytes(String),
    /// A referenced node ID does not exist in the network.
    NodeNotFound(usize),
    /// A referenced connection is invalid.
    InvalidConnection { source: usize, target: usize },
    /// Conservation law violation detected.
    ConservationViolation { expected: f64, actual: f64 },
    /// Propagation did not converge within the iteration limit.
    ConvergenceFailed { iterations: usize },
    /// Routing error (e.g., no available voice for polyphony).
    RoutingError(String),
    /// Generic error.
    Other(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMidiBytes(msg) => write!(f, "invalid MIDI bytes: {msg}"),
            Self::NodeNotFound(id) => write!(f, "node not found: {id}"),
            Self::InvalidConnection { source, target } => {
                write!(f, "invalid connection: {source} -> {target}")
            }
            Self::ConservationViolation { expected, actual } => {
                write!(
                    f,
                    "conservation violation: expected {expected:.6}, got {actual:.6}"
                )
            }
            Self::ConvergenceFailed { iterations } => {
                write!(f, "propagation failed to converge after {iterations} iterations")
            }
            Self::RoutingError(msg) => write!(f, "routing error: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for BridgeError {}
