//! # midi-flux-bridge
//!
//! Bridge between MIDI events and flux-based state propagation in musical agent networks.
//!
//! In the SuperInstance ecosystem, MIDI events are state changes that propagate through
//! a network of connected modules (instruments, effects, analyzers). This crate bridges
//! raw MIDI messages to a flux-based state propagation system: each MIDI event becomes
//! a flux node, connections propagate state changes, and conservation laws constrain
//! the total energy flowing through the network.
//!
//! ## Quick Start
//!
//! ```
//! use midi_flux_bridge::*;
//!
//! // Parse a MIDI event from raw bytes
//! let event = MidiEvent::from_bytes(&[0x90, 0x3C, 0x64], 0.0).unwrap();
//! assert!(event.is_note_on());
//!
//! // Build a flux network
//! let mut network = FluxNetwork::new();
//! network.add_node(FluxNode::new(0, "input", 2));
//! network.add_node(FluxNode::with_transform(1, "output", 2, Transform::Scale(0.5)));
//! network.connect(Connection::linear(0, 1, 1.0)).unwrap();
//!
//! // Track conservation
//! let mut law = ConservationLaw::from_network(&network, 0.001);
//!
//! // Set up routing
//! let mut router = MidiRouter::new(4, 2);
//! router.allocate_channel(0, &mut network, Transform::Identity);
//!
//! // Route an event
//! let affected = router.route_event(&event, &mut network).unwrap();
//! ```

pub mod connection;
pub mod conservation;
pub mod error;
pub mod flux_node;
pub mod midi_event;
pub mod network;
pub mod routing;

pub use connection::{Connection, ConnectionKind};
pub use conservation::{ConservationLaw, ConservationResult};
pub use error::BridgeError;
pub use flux_node::{FluxNode, Transform};
pub use midi_event::{MidiEvent, MidiEventKind};
pub use network::FluxNetwork;
pub use routing::MidiRouter;
