use serde::{Deserialize, Serialize};

/// Transform function applied by a flux node to its state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transform {
    /// Pass through unchanged.
    Identity,
    /// Multiply each state element by a scalar.
    Scale(f64),
    /// Apply a per-element mapping: `output[i] = state[i] * gains[i]`.
    Map { gains: Vec<f64> },
    /// Only pass elements whose absolute value exceeds a threshold.
    Filter { threshold: f64 },
    /// Accumulate incoming flux into state (sum).
    Accumulate,
}

impl Transform {
    /// Apply the transform to an input state vector, producing an output state vector.
    pub fn apply(&self, input: &[f64]) -> Vec<f64> {
        match self {
            Transform::Identity => input.to_vec(),
            Transform::Scale(factor) => input.iter().map(|x| x * factor).collect(),
            Transform::Map { gains } => input
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    let g = gains.get(i).copied().unwrap_or(1.0);
                    x * g
                })
                .collect(),
            Transform::Filter { threshold } => input
                .iter()
                .map(|x| {
                    if x.abs() >= *threshold {
                        *x
                    } else {
                        0.0
                    }
                })
                .collect(),
            Transform::Accumulate => input.to_vec(),
        }
    }
}

/// A node in the flux network that receives MIDI events and transforms state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluxNode {
    pub id: usize,
    pub name: String,
    pub transform: Transform,
    /// Current state vector.
    pub state: Vec<f64>,
}

impl FluxNode {
    /// Create a new flux node with an identity transform and zeroed state.
    pub fn new(id: usize, name: &str, state_dim: usize) -> Self {
        FluxNode {
            id,
            name: name.to_string(),
            transform: Transform::Identity,
            state: vec![0.0; state_dim],
        }
    }

    /// Create a node with a specific transform.
    pub fn with_transform(id: usize, name: &str, state_dim: usize, transform: Transform) -> Self {
        FluxNode {
            id,
            name: name.to_string(),
            transform,
            state: vec![0.0; state_dim],
        }
    }

    /// Apply the node's transform to its current state, returning the output.
    pub fn output(&self) -> Vec<f64> {
        self.transform.apply(&self.state)
    }

    /// Inject a state vector into this node (e.g., from a MIDI event).
    pub fn inject(&mut self, values: &[f64]) {
        for (i, v) in values.iter().enumerate() {
            if i < self.state.len() {
                self.state[i] = *v;
            }
        }
    }

    /// Add incoming flux to this node's state.
    pub fn receive(&mut self, incoming: &[f64]) {
        for (i, v) in incoming.iter().enumerate() {
            if i < self.state.len() {
                self.state[i] += v;
            }
        }
    }

    /// Total energy (sum of absolute state values).
    pub fn energy(&self) -> f64 {
        self.state.iter().map(|x| x.abs()).sum()
    }

    /// Reset state to zero.
    pub fn reset(&mut self) {
        for v in &mut self.state {
            *v = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_node_has_zero_state() {
        let node = FluxNode::new(0, "test", 3);
        assert_eq!(node.state, vec![0.0, 0.0, 0.0]);
        assert_eq!(node.id, 0);
    }

    #[test]
    fn identity_transform() {
        let node = FluxNode::new(0, "id", 2);
        assert_eq!(node.output(), vec![0.0, 0.0]);
    }

    #[test]
    fn scale_transform() {
        let mut node = FluxNode::with_transform(0, "scale", 2, Transform::Scale(2.0));
        node.state = vec![1.0, 3.0];
        assert_eq!(node.output(), vec![2.0, 6.0]);
    }

    #[test]
    fn map_transform() {
        let mut node = FluxNode::with_transform(
            0,
            "map",
            3,
            Transform::Map {
                gains: vec![0.5, 1.0, 2.0],
            },
        );
        node.state = vec![2.0, 3.0, 1.0];
        assert_eq!(node.output(), vec![1.0, 3.0, 2.0]);
    }

    #[test]
    fn filter_transform() {
        let mut node = FluxNode::with_transform(
            0,
            "filter",
            3,
            Transform::Filter { threshold: 0.5 },
        );
        node.state = vec![0.1, 0.6, -0.8];
        assert_eq!(node.output(), vec![0.0, 0.6, -0.8]);
    }

    #[test]
    fn inject_state() {
        let mut node = FluxNode::new(0, "inj", 3);
        node.inject(&[1.0, 2.0, 3.0]);
        assert_eq!(node.state, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn receive_accumulates() {
        let mut node = FluxNode::new(0, "acc", 2);
        node.state = vec![1.0, 2.0];
        node.receive(&[0.5, 1.5]);
        assert_eq!(node.state, vec![1.5, 3.5]);
    }

    #[test]
    fn energy_calculation() {
        let mut node = FluxNode::new(0, "e", 3);
        node.state = vec![1.0, -2.0, 3.0];
        assert_eq!(node.energy(), 6.0);
    }

    #[test]
    fn reset_state() {
        let mut node = FluxNode::new(0, "r", 2);
        node.state = vec![5.0, 10.0];
        node.reset();
        assert_eq!(node.state, vec![0.0, 0.0]);
    }

    #[test]
    fn inject_truncates_oversized() {
        let mut node = FluxNode::new(0, "t", 2);
        node.inject(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(node.state, vec![1.0, 2.0]);
    }
}
