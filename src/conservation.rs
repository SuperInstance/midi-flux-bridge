use crate::network::FluxNetwork;

/// Tracks conservation of flux energy across the network.
///
/// A conservation law records the initial total energy and can detect when
/// energy is created or destroyed (i.e., when the total diverges from expected).
#[derive(Debug, Clone)]
pub struct ConservationLaw {
    /// Expected total energy.
    pub expected: f64,
    /// Tolerance for violation detection.
    pub tolerance: f64,
}

impl ConservationLaw {
    /// Create a new conservation law with the given expected energy and tolerance.
    pub fn new(expected: f64, tolerance: f64) -> Self {
        ConservationLaw { expected, tolerance }
    }

    /// Initialize from the current state of a network.
    pub fn from_network(network: &FluxNetwork, tolerance: f64) -> Self {
        ConservationLaw {
            expected: network.total_energy(),
            tolerance,
        }
    }

    /// Check whether the network satisfies the conservation law.
    pub fn check(&self, network: &FluxNetwork) -> ConservationResult {
        let actual = network.total_energy();
        let delta = (actual - self.expected).abs();
        ConservationResult {
            expected: self.expected,
            actual,
            delta,
            violated: delta > self.tolerance,
        }
    }

    /// Update the expected energy after an injection (e.g., MIDI event bringing new energy).
    pub fn inject(&mut self, amount: f64) {
        self.expected += amount.abs();
    }

    /// Update the expected energy after an extraction (e.g., note-off removing energy).
    pub fn extract(&mut self, amount: f64) {
        self.expected -= amount.abs();
        if self.expected < 0.0 {
            self.expected = 0.0;
        }
    }
}

/// Result of a conservation check.
#[derive(Debug, Clone, PartialEq)]
pub struct ConservationResult {
    pub expected: f64,
    pub actual: f64,
    pub delta: f64,
    pub violated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flux_node::FluxNode;

    fn make_network() -> FluxNetwork {
        let mut net = FluxNetwork::new();
        let mut n = FluxNode::new(0, "a", 2);
        n.state = vec![2.0, 3.0];
        net.add_node(n);
        net
    }

    #[test]
    fn conservation_satisfied() {
        let net = make_network();
        let law = ConservationLaw::from_network(&net, 0.01);
        let result = law.check(&net);
        assert!(!result.violated);
        assert!((result.expected - 5.0).abs() < 1e-10);
    }

    #[test]
    fn conservation_violated() {
        let mut net = make_network();
        let law = ConservationLaw::from_network(&net, 0.01);
        // Secretly add energy
        net.nodes[0].state[0] += 10.0;
        let result = law.check(&net);
        assert!(result.violated);
    }

    #[test]
    fn inject_updates_expected() {
        let net = make_network();
        let mut law = ConservationLaw::from_network(&net, 0.01);
        law.inject(3.0);
        assert!((law.expected - 8.0).abs() < 1e-10);
    }

    #[test]
    fn extract_updates_expected() {
        let net = make_network();
        let mut law = ConservationLaw::from_network(&net, 0.01);
        law.extract(2.0);
        assert!((law.expected - 3.0).abs() < 1e-10);
    }

    #[test]
    fn extract_clamps_to_zero() {
        let net = make_network();
        let mut law = ConservationLaw::from_network(&net, 0.01);
        law.extract(100.0);
        assert!((law.expected).abs() < 1e-10);
    }

    #[test]
    fn conservation_result_fields() {
        let result = ConservationResult {
            expected: 5.0,
            actual: 5.1,
            delta: 0.1,
            violated: false,
        };
        assert!(!result.violated);
        assert!((result.delta - 0.1).abs() < 1e-10);
    }
}
