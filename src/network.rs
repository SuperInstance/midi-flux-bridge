use crate::connection::Connection;
use crate::error::BridgeError;
use crate::flux_node::FluxNode;
use crate::midi_event::MidiEvent;

const DEFAULT_MAX_ITERATIONS: usize = 100;

/// A graph of flux nodes and connections with a propagation engine.
#[derive(Debug, Clone)]
pub struct FluxNetwork {
    pub nodes: Vec<FluxNode>,
    pub connections: Vec<Connection>,
    pub max_iterations: usize,
}

impl FluxNetwork {
    /// Create an empty network.
    pub fn new() -> Self {
        FluxNetwork {
            nodes: Vec::new(),
            connections: Vec::new(),
            max_iterations: DEFAULT_MAX_ITERATIONS,
        }
    }

    /// Add a node, returning its assigned ID.
    pub fn add_node(&mut self, mut node: FluxNode) -> usize {
        let id = self.nodes.len();
        node.id = id;
        self.nodes.push(node);
        id
    }

    /// Add a connection between two nodes.
    pub fn connect(&mut self, conn: Connection) -> Result<(), BridgeError> {
        let source_exists = self.nodes.iter().any(|n| n.id == conn.source);
        let target_exists = self.nodes.iter().any(|n| n.id == conn.target);
        if !source_exists {
            return Err(BridgeError::NodeNotFound(conn.source));
        }
        if !target_exists {
            return Err(BridgeError::NodeNotFound(conn.target));
        }
        self.connections.push(conn);
        Ok(())
    }

    /// Get a reference to a node by ID.
    pub fn get_node(&self, id: usize) -> Option<&FluxNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get a mutable reference to a node by ID.
    pub fn get_node_mut(&mut self, id: usize) -> Option<&mut FluxNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Total energy across all nodes.
    pub fn total_energy(&self) -> f64 {
        self.nodes.iter().map(|n| n.energy()).sum()
    }

    /// Reset all node states to zero.
    pub fn reset_all(&mut self) {
        for node in &mut self.nodes {
            node.reset();
        }
    }

    /// Propagate flux through the entire network once (single pass).
    ///
    /// For each connection, read the source node's output, apply the connection
    /// transfer function, and deliver to the target node. Uses the node outputs
    /// computed at the start of this step (not updated mid-step).
    fn propagate_step(&mut self) {
        // Compute all outputs before any mutations
        let outputs: Vec<Vec<f64>> = self.nodes.iter().map(|n| n.output()).collect();

        // Collect incoming flux per node
        let mut incoming: Vec<Vec<f64>> = self.nodes.iter().map(|n| vec![0.0; n.state.len()]).collect();

        for conn in &self.connections {
            let src_out = &outputs[conn.source];
            let transferred = conn.transfer(src_out);
            let tgt = &mut incoming[conn.target];
            for (i, v) in transferred.iter().enumerate() {
                if i < tgt.len() {
                    tgt[i] += v;
                }
            }
        }

        // Apply incoming to nodes
        for node in &mut self.nodes {
            let inc = incoming[node.id].clone();
            node.receive(&inc);
        }
    }

    /// Propagate a MIDI event into the network and run propagation to convergence.
    ///
    /// The event is injected into the specified node, then flux propagates until
    /// the network converges (total energy change < epsilon) or max iterations.
    pub fn process_event(
        &mut self,
        node_id: usize,
        _event: &MidiEvent,
        values: &[f64],
    ) -> Result<f64, BridgeError> {
        let node = self
            .get_node_mut(node_id)
            .ok_or(BridgeError::NodeNotFound(node_id))?;
        node.inject(values);

        let epsilon = 1e-9;
        for iteration in 0..self.max_iterations {
            let before = self.total_energy();
            self.propagate_step();
            let after = self.total_energy();
            if (after - before).abs() < epsilon {
                return Ok(after);
            }
            if iteration == self.max_iterations - 1 {
                return Err(BridgeError::ConvergenceFailed {
                    iterations: self.max_iterations,
                });
            }
        }
        Ok(self.total_energy())
    }

    /// Propagate once without an event (useful for steady-state checks).
    pub fn propagate(&mut self) -> f64 {
        self.propagate_step();
        self.total_energy()
    }
}

impl Default for FluxNetwork {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flux_node::Transform;

    fn make_simple_network() -> FluxNetwork {
        let mut net = FluxNetwork::new();
        let n0 = FluxNode::new(0, "source", 2);
        let n1 = FluxNode::with_transform(1, "scale", 2, Transform::Scale(0.5));
        net.add_node(n0);
        net.add_node(n1);
        net.connect(Connection::linear(0, 1, 1.0)).unwrap();
        net
    }

    #[test]
    fn create_network() {
        let net = make_simple_network();
        assert_eq!(net.nodes.len(), 2);
        assert_eq!(net.connections.len(), 1);
    }

    #[test]
    fn add_nodes_sequential_ids() {
        let mut net = FluxNetwork::new();
        let id0 = net.add_node(FluxNode::new(99, "a", 1));
        let id1 = net.add_node(FluxNode::new(99, "b", 1));
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(net.nodes[0].id, 0);
        assert_eq!(net.nodes[1].id, 1);
    }

    #[test]
    fn connect_invalid_source() {
        let mut net = FluxNetwork::new();
        net.add_node(FluxNode::new(0, "a", 1));
        let err = net.connect(Connection::linear(5, 0, 1.0)).unwrap_err();
        assert!(matches!(err, BridgeError::NodeNotFound(5)));
    }

    #[test]
    fn connect_invalid_target() {
        let mut net = FluxNetwork::new();
        net.add_node(FluxNode::new(0, "a", 1));
        let err = net.connect(Connection::linear(0, 5, 1.0)).unwrap_err();
        assert!(matches!(err, BridgeError::NodeNotFound(5)));
    }

    #[test]
    fn propagate_linear() {
        let mut net = make_simple_network();
        net.nodes[0].state = vec![2.0, 4.0];
        net.propagate();
        // Node 0 output = [2.0, 4.0] (identity), transferred = [2.0, 4.0]
        // Node 1 receives [2.0, 4.0], state becomes [2.0, 4.0]
        assert_eq!(net.nodes[1].state, vec![2.0, 4.0]);
    }

    #[test]
    fn process_event_converges() {
        // Use a single node with no outgoing connections — converges immediately
        let mut net = FluxNetwork::new();
        net.add_node(FluxNode::new(0, "solo", 2));
        let event = MidiEvent {
            channel: 0,
            kind: crate::midi_event::MidiEventKind::NoteOn {
                note: 60,
                velocity: 100,
            },
            timestamp: 0.0,
        };
        let energy = net.process_event(0, &event, &[1.0, 1.0]).unwrap();
        assert!(energy > 0.0);
    }

    #[test]
    fn total_energy_calculation() {
        let mut net = FluxNetwork::new();
        net.add_node(FluxNode::new(0, "a", 2));
        net.add_node(FluxNode::new(1, "b", 2));
        net.nodes[0].state = vec![1.0, 2.0];
        net.nodes[1].state = vec![3.0, 4.0];
        assert_eq!(net.total_energy(), 10.0);
    }

    #[test]
    fn reset_all() {
        let mut net = make_simple_network();
        net.nodes[0].state = vec![5.0, 10.0];
        net.reset_all();
        assert!(net.nodes.iter().all(|n| n.state.iter().all(|&v| v == 0.0)));
    }

    #[test]
    fn process_event_invalid_node() {
        let mut net = make_simple_network();
        let event = MidiEvent {
            channel: 0,
            kind: crate::midi_event::MidiEventKind::NoteOn {
                note: 60,
                velocity: 100,
            },
            timestamp: 0.0,
        };
        let err = net.process_event(99, &event, &[1.0]).unwrap_err();
        assert!(matches!(err, BridgeError::NodeNotFound(99)));
    }

    #[test]
    fn three_node_chain() {
        let mut net = FluxNetwork::new();
        net.add_node(FluxNode::new(0, "a", 1));
        net.add_node(FluxNode::new(1, "b", 1));
        net.add_node(FluxNode::new(2, "c", 1));
        net.connect(Connection::linear(0, 1, 1.0)).unwrap();
        net.connect(Connection::linear(1, 2, 1.0)).unwrap();

        net.nodes[0].state = vec![5.0];
        // Step 1: a->b
        net.propagate_step();
        assert_eq!(net.nodes[1].state, vec![5.0]);

        // Step 2: b->c
        net.propagate_step();
        assert_eq!(net.nodes[2].state, vec![5.0]);
    }

    #[test]
    fn threshold_connection_propagation() {
        let mut net = FluxNetwork::new();
        net.add_node(FluxNode::new(0, "a", 1));
        net.add_node(FluxNode::new(1, "b", 1));
        net.connect(Connection::threshold(0, 1, 1.0, 1.0)).unwrap();

        // Below threshold - blocked
        net.nodes[0].state = vec![0.5];
        net.propagate_step();
        assert_eq!(net.nodes[1].state, vec![0.0]);

        // Above threshold - passes
        net.nodes[0].state = vec![1.5];
        net.propagate_step();
        assert_eq!(net.nodes[1].state, vec![1.5]);
    }
}
