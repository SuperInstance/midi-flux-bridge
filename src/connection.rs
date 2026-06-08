use serde::{Deserialize, Serialize};

/// The kind of transfer function a connection uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionKind {
    /// Output = input * weight.
    Linear,
    /// Output = weight * exp(input).
    Exponential,
    /// Output = input * weight if input.abs() >= threshold, else 0.
    Threshold { value: f64 },
}

/// A weighted directed connection between two flux nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub source: usize,
    pub target: usize,
    pub weight: f64,
    pub kind: ConnectionKind,
}

impl Connection {
    /// Create a new linear connection.
    pub fn linear(source: usize, target: usize, weight: f64) -> Self {
        Connection {
            source,
            target,
            weight,
            kind: ConnectionKind::Linear,
        }
    }

    /// Create a new exponential connection.
    pub fn exponential(source: usize, target: usize, weight: f64) -> Self {
        Connection {
            source,
            target,
            weight,
            kind: ConnectionKind::Exponential,
        }
    }

    /// Create a new threshold connection.
    pub fn threshold(source: usize, target: usize, weight: f64, threshold: f64) -> Self {
        Connection {
            source,
            target,
            weight,
            kind: ConnectionKind::Threshold { value: threshold },
        }
    }

    /// Apply the connection's transfer function to an input vector.
    pub fn transfer(&self, input: &[f64]) -> Vec<f64> {
        match &self.kind {
            ConnectionKind::Linear => input.iter().map(|x| x * self.weight).collect(),
            ConnectionKind::Exponential => input
                .iter()
                .map(|x| self.weight * x.exp())
                .collect(),
            ConnectionKind::Threshold { value } => input
                .iter()
                .map(|x| {
                    if x.abs() >= *value {
                        x * self.weight
                    } else {
                        0.0
                    }
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_transfer() {
        let conn = Connection::linear(0, 1, 0.5);
        assert_eq!(conn.transfer(&[2.0, 4.0]), vec![1.0, 2.0]);
    }

    #[test]
    fn exponential_transfer() {
        let conn = Connection::exponential(0, 1, 1.0);
        let out = conn.transfer(&[0.0]);
        assert!((out[0] - 1.0).abs() < 1e-10); // e^0 = 1
    }

    #[test]
    fn threshold_transfer_passes() {
        let conn = Connection::threshold(0, 1, 1.0, 0.5);
        assert_eq!(conn.transfer(&[0.6]), vec![0.6]);
    }

    #[test]
    fn threshold_transfer_blocks() {
        let conn = Connection::threshold(0, 1, 1.0, 0.5);
        assert_eq!(conn.transfer(&[0.3]), vec![0.0]);
    }

    #[test]
    fn connection_accessors() {
        let conn = Connection::linear(2, 5, 0.75);
        assert_eq!(conn.source, 2);
        assert_eq!(conn.target, 5);
        assert!((conn.weight - 0.75).abs() < f64::EPSILON);
    }
}
