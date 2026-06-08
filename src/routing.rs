use crate::error::BridgeError;
use crate::flux_node::{FluxNode, Transform};
use crate::midi_event::MidiEvent;
use crate::network::FluxNetwork;

/// A voice slot managed by the router.
#[derive(Debug, Clone)]
struct Voice {
    note: u8,
    node_id: usize,
    active: bool,
}

/// Maps MIDI channels to flux nodes and manages polyphony with voice stealing.
#[derive(Debug)]
pub struct MidiRouter {
    /// Max number of simultaneous voices per channel.
    pub max_voices: usize,
    /// Channel → list of node IDs allocated for that channel.
    channel_nodes: Vec<Vec<usize>>,
    /// Active voices (note, node_id, active).
    voices: Vec<Voice>,
    /// State dimension for created nodes.
    state_dim: usize,
}

impl MidiRouter {
    /// Create a new router with the given max polyphony and state dimension.
    pub fn new(max_voices: usize, state_dim: usize) -> Self {
        MidiRouter {
            max_voices,
            channel_nodes: vec![Vec::new(); 16],
            voices: Vec::new(),
            state_dim,
        }
    }

    /// Pre-allocate nodes for a specific channel in the network.
    pub fn allocate_channel(
        &mut self,
        channel: u8,
        network: &mut FluxNetwork,
        transform: Transform,
    ) {
        let ch = (channel & 0x0F) as usize;
        self.channel_nodes[ch].clear();
        for i in 0..self.max_voices {
            let name = format!("ch{}_voice{}", ch, i);
            let node = FluxNode::with_transform(0, &name, self.state_dim, transform.clone());
            let id = network.add_node(node);
            self.channel_nodes[ch].push(id);
        }
    }

    /// Route a NoteOn event: find or steal a voice and inject velocity.
    pub fn note_on(
        &mut self,
        channel: u8,
        note: u8,
        velocity: u8,
        network: &mut FluxNetwork,
    ) -> Result<usize, BridgeError> {
        let ch = (channel & 0x0F) as usize;
        let node_ids = &self.channel_nodes[ch];
        if node_ids.is_empty() {
            return Err(BridgeError::RoutingError(format!(
                "no voices allocated for channel {ch}"
            )));
        }

        // Check if this note is already playing (re-trigger)
        if let Some(voice) = self.voices.iter_mut().find(|v| v.note == note && v.active) {
            let nid = voice.node_id;
            let vel_norm = velocity as f64 / 127.0;
            let vals = vec![vel_norm; self.state_dim];
            network
                .get_node_mut(nid)
                .ok_or(BridgeError::NodeNotFound(nid))?
                .inject(&vals);
            return Ok(nid);
        }

        // Find an inactive voice slot among existing voices
        if let Some(slot) = self.voices.iter().position(|v| !v.active) {
            let nid = self.voices[slot].node_id;
            self.voices[slot] = Voice {
                note,
                node_id: nid,
                active: true,
            };
            let vel_norm = velocity as f64 / 127.0;
            let vals = vec![vel_norm; self.state_dim];
            network
                .get_node_mut(nid)
                .ok_or(BridgeError::NodeNotFound(nid))?
                .inject(&vals);
            return Ok(nid);
        }

        // No inactive voices — can we allocate a new one?
        if self.voices.len() < node_ids.len() {
            let idx = self.voices.len();
            let nid = node_ids[idx];
            self.voices.push(Voice {
                note,
                node_id: nid,
                active: true,
            });
            let vel_norm = velocity as f64 / 127.0;
            let vals = vec![vel_norm; self.state_dim];
            network
                .get_node_mut(nid)
                .ok_or(BridgeError::NodeNotFound(nid))?
                .inject(&vals);
            return Ok(nid);
        }

        // Voice stealing: steal voice 0 (oldest)
        let nid = self.voices[0].node_id;
        if let Some(n) = network.get_node_mut(nid) {
            n.reset();
        }
        self.voices[0] = Voice {
            note,
            node_id: nid,
            active: true,
        };
        let vel_norm = velocity as f64 / 127.0;
        let vals = vec![vel_norm; self.state_dim];
        network
            .get_node_mut(nid)
            .ok_or(BridgeError::NodeNotFound(nid))?
            .inject(&vals);
        Ok(nid)
    }

    /// Route a NoteOff event: deactivate the voice and zero its state.
    pub fn note_off(
        &mut self,
        _channel: u8,
        note: u8,
        network: &mut FluxNetwork,
    ) -> Result<usize, BridgeError> {
        let voice = self
            .voices
            .iter_mut()
            .find(|v| v.note == note && v.active)
            .ok_or(BridgeError::RoutingError(format!(
                "no active voice for note {note}"
            )))?;

        voice.active = false;
        let nid = voice.node_id;
        if let Some(n) = network.get_node_mut(nid) {
            n.reset();
        }
        Ok(nid)
    }

    /// Route a CC event to all active voices on a channel.
    pub fn control_change(
        &mut self,
        _channel: u8,
        controller: u8,
        value: u8,
        network: &mut FluxNetwork,
    ) -> Vec<usize> {
        let ch = (_channel & 0x0F) as usize;
        let mut affected = Vec::new();
        let val_norm = value as f64 / 127.0;

        for &nid in &self.channel_nodes[ch] {
            if let Some(node) = network.get_node_mut(nid) {
                // Inject CC into first state dimension
                node.inject(&[val_norm]);
                affected.push(nid);
            }
        }
        let _ = controller; // could use for routing logic
        affected
    }

    /// Get the number of currently active voices.
    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Route a full MIDI event, dispatching to the appropriate handler.
    pub fn route_event(
        &mut self,
        event: &MidiEvent,
        network: &mut FluxNetwork,
    ) -> Result<Vec<usize>, BridgeError> {
        match &event.kind {
            crate::midi_event::MidiEventKind::NoteOn { note, velocity } => {
                let nid = self.note_on(event.channel, *note, *velocity, network)?;
                Ok(vec![nid])
            }
            crate::midi_event::MidiEventKind::NoteOff { note, .. } => {
                let nid = self.note_off(event.channel, *note, network)?;
                Ok(vec![nid])
            }
            crate::midi_event::MidiEventKind::ControlChange {
                controller,
                value,
            } => {
                let affected = self.control_change(event.channel, *controller, *value, network);
                Ok(affected)
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Reset all voices.
    pub fn reset(&mut self, network: &mut FluxNetwork) {
        for v in &mut self.voices {
            v.active = false;
            if let Some(n) = network.get_node_mut(v.node_id) {
                n.reset();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_router_and_network() -> (MidiRouter, FluxNetwork) {
        let mut router = MidiRouter::new(4, 2);
        let mut network = FluxNetwork::new();
        router.allocate_channel(0, &mut network, Transform::Identity);
        (router, network)
    }

    #[test]
    fn allocate_channel_creates_nodes() {
        let (_router, network) = make_router_and_network();
        assert_eq!(network.nodes.len(), 4);
    }

    #[test]
    fn note_on_activates_voice() {
        let (mut router, mut network) = make_router_and_network();
        let nid = router.note_on(0, 60, 100, &mut network).unwrap();
        assert_eq!(router.active_voice_count(), 1);
        let node = network.get_node(nid).unwrap();
        let expected_vel = 100.0 / 127.0;
        assert!((node.state[0] - expected_vel).abs() < 1e-10);
    }

    #[test]
    fn note_off_deactivates_voice() {
        let (mut router, mut network) = make_router_and_network();
        router.note_on(0, 60, 100, &mut network).unwrap();
        router.note_off(0, 60, &mut network).unwrap();
        assert_eq!(router.active_voice_count(), 0);
    }

    #[test]
    fn note_off_not_found() {
        let (mut router, mut network) = make_router_and_network();
        let err = router.note_off(0, 60, &mut network).unwrap_err();
        assert!(matches!(err, BridgeError::RoutingError(_)));
    }

    #[test]
    fn polyphony_multiple_notes() {
        let (mut router, mut network) = make_router_and_network();
        router.note_on(0, 60, 100, &mut network).unwrap();
        router.note_on(0, 64, 100, &mut network).unwrap();
        router.note_on(0, 67, 100, &mut network).unwrap();
        assert_eq!(router.active_voice_count(), 3);
    }

    #[test]
    fn voice_stealing() {
        let (mut router, mut network) = make_router_and_network();
        // Fill all 4 voices
        router.note_on(0, 60, 100, &mut network).unwrap();
        router.note_on(0, 62, 100, &mut network).unwrap();
        router.note_on(0, 64, 100, &mut network).unwrap();
        router.note_on(0, 65, 100, &mut network).unwrap();
        assert_eq!(router.active_voice_count(), 4);
        // 5th note should steal voice 0 (note 60)
        let nid = router.note_on(0, 67, 100, &mut network).unwrap();
        assert_eq!(router.active_voice_count(), 4);
        // The stolen voice now plays note 67
        assert_eq!(router.voices[0].note, 67);
        assert_eq!(router.voices[0].node_id, nid);
    }

    #[test]
    fn retrigger_same_note() {
        let (mut router, mut network) = make_router_and_network();
        let nid1 = router.note_on(0, 60, 50, &mut network).unwrap();
        let nid2 = router.note_on(0, 60, 100, &mut network).unwrap();
        assert_eq!(nid1, nid2);
        assert_eq!(router.active_voice_count(), 1);
        let node = network.get_node(nid2).unwrap();
        assert!((node.state[0] - 100.0 / 127.0).abs() < 1e-10);
    }

    #[test]
    fn control_change_affects_nodes() {
        let (mut router, mut network) = make_router_and_network();
        router.note_on(0, 60, 100, &mut network).unwrap();
        let affected = router.control_change(0, 7, 64, &mut network);
        assert!(!affected.is_empty());
    }

    #[test]
    fn route_event_note_on() {
        let (mut router, mut network) = make_router_and_network();
        let event = MidiEvent {
            channel: 0,
            kind: crate::midi_event::MidiEventKind::NoteOn {
                note: 60,
                velocity: 80,
            },
            timestamp: 0.0,
        };
        let affected = router.route_event(&event, &mut network).unwrap();
        assert_eq!(affected.len(), 1);
    }

    #[test]
    fn route_event_note_off() {
        let (mut router, mut network) = make_router_and_network();
        router.note_on(0, 60, 100, &mut network).unwrap();
        let event = MidiEvent {
            channel: 0,
            kind: crate::midi_event::MidiEventKind::NoteOff {
                note: 60,
                velocity: 0,
            },
            timestamp: 0.0,
        };
        let affected = router.route_event(&event, &mut network).unwrap();
        assert_eq!(affected.len(), 1);
        assert_eq!(router.active_voice_count(), 0);
    }

    #[test]
    fn route_event_cc() {
        let (mut router, mut network) = make_router_and_network();
        let event = MidiEvent {
            channel: 0,
            kind: crate::midi_event::MidiEventKind::ControlChange {
                controller: 1,
                value: 127,
            },
            timestamp: 0.0,
        };
        let affected = router.route_event(&event, &mut network).unwrap();
        assert_eq!(affected.len(), 4); // all 4 voices
    }

    #[test]
    fn reset_clears_voices() {
        let (mut router, mut network) = make_router_and_network();
        router.note_on(0, 60, 100, &mut network).unwrap();
        router.note_on(0, 64, 100, &mut network).unwrap();
        router.reset(&mut network);
        assert_eq!(router.active_voice_count(), 0);
    }

    #[test]
    fn unallocated_channel_error() {
        let mut router = MidiRouter::new(4, 2);
        let mut network = FluxNetwork::new();
        let err = router.note_on(5, 60, 100, &mut network).unwrap_err();
        assert!(matches!(err, BridgeError::RoutingError(_)));
    }
}
