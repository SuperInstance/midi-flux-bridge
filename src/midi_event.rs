use crate::error::BridgeError;
use serde::{Deserialize, Serialize};

/// The specific kind of a MIDI event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MidiEventKind {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8, velocity: u8 },
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
    PitchBend { value: i16 },
}

/// A MIDI event with channel, kind, and timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiEvent {
    pub channel: u8,
    pub kind: MidiEventKind,
    pub timestamp: f64,
}

impl MidiEvent {
    /// Parse a MIDI event from raw bytes.
    ///
    /// Expects at least one byte where the high nibble indicates the status
    /// and the low nibble indicates the channel. Returns an error for
    /// unsupported status bytes or incorrect payload lengths.
    pub fn from_bytes(bytes: &[u8], timestamp: f64) -> Result<Self, BridgeError> {
        if bytes.is_empty() {
            return Err(BridgeError::InvalidMidiBytes("empty byte slice".into()));
        }
        let status = bytes[0];
        let command = status & 0xF0;
        let channel = status & 0x0F;

        let kind = match command {
            0x80 => {
                if bytes.len() < 3 {
                    return Err(BridgeError::InvalidMidiBytes(
                        "NoteOff requires 2 data bytes".into(),
                    ));
                }
                MidiEventKind::NoteOff {
                    note: bytes[1] & 0x7F,
                    velocity: bytes[2] & 0x7F,
                }
            }
            0x90 => {
                if bytes.len() < 3 {
                    return Err(BridgeError::InvalidMidiBytes(
                        "NoteOn requires 2 data bytes".into(),
                    ));
                }
                let note = bytes[1] & 0x7F;
                let velocity = bytes[2] & 0x7F;
                // velocity 0 means note-off
                if velocity == 0 {
                    MidiEventKind::NoteOff { note, velocity: 0 }
                } else {
                    MidiEventKind::NoteOn { note, velocity }
                }
            }
            0xB0 => {
                if bytes.len() < 3 {
                    return Err(BridgeError::InvalidMidiBytes(
                        "ControlChange requires 2 data bytes".into(),
                    ));
                }
                MidiEventKind::ControlChange {
                    controller: bytes[1] & 0x7F,
                    value: bytes[2] & 0x7F,
                }
            }
            0xC0 => {
                if bytes.len() < 2 {
                    return Err(BridgeError::InvalidMidiBytes(
                        "ProgramChange requires 1 data byte".into(),
                    ));
                }
                MidiEventKind::ProgramChange {
                    program: bytes[1] & 0x7F,
                }
            }
            0xE0 => {
                if bytes.len() < 3 {
                    return Err(BridgeError::InvalidMidiBytes(
                        "PitchBend requires 2 data bytes".into(),
                    ));
                }
                let lsb = (bytes[1] & 0x7F) as i16;
                let msb = (bytes[2] & 0x7F) as i16;
                let value = (msb << 7) | lsb;
                // Center is 0x2000 = 8192
                let value = value - 8192;
                MidiEventKind::PitchBend { value }
            }
            _ => {
                return Err(BridgeError::InvalidMidiBytes(format!(
                    "unsupported status byte: 0x{:02X}",
                    status
                )));
            }
        };

        Ok(MidiEvent {
            channel,
            kind,
            timestamp,
        })
    }

    /// Serialize the MIDI event back to raw bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let channel = self.channel & 0x0F;
        match &self.kind {
            MidiEventKind::NoteOn { note, velocity } => {
                vec![0x90 | channel, note & 0x7F, velocity & 0x7F]
            }
            MidiEventKind::NoteOff { note, velocity } => {
                vec![0x80 | channel, note & 0x7F, velocity & 0x7F]
            }
            MidiEventKind::ControlChange { controller, value } => {
                vec![0xB0 | channel, controller & 0x7F, value & 0x7F]
            }
            MidiEventKind::ProgramChange { program } => {
                vec![0xC0 | channel, program & 0x7F]
            }
            MidiEventKind::PitchBend { value } => {
                let v = (*value + 8192) as u16;
                let lsb = (v & 0x7F) as u8;
                let msb = ((v >> 7) & 0x7F) as u8;
                vec![0xE0 | channel, lsb, msb]
            }
        }
    }

    /// Returns the note number if this is a NoteOn or NoteOff event.
    pub fn note(&self) -> Option<u8> {
        match &self.kind {
            MidiEventKind::NoteOn { note, .. } | MidiEventKind::NoteOff { note, .. } => {
                Some(*note)
            }
            _ => None,
        }
    }

    /// Returns true if this is a NoteOn event with non-zero velocity.
    pub fn is_note_on(&self) -> bool {
        matches!(&self.kind, MidiEventKind::NoteOn { velocity, .. } if *velocity > 0)
    }

    /// Returns true if this is a NoteOff event or a NoteOn with velocity 0.
    pub fn is_note_off(&self) -> bool {
        matches!(
            &self.kind,
            MidiEventKind::NoteOff { .. }
        ) || matches!(&self.kind, MidiEventKind::NoteOn { velocity: 0, .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn parse_note_on() {
        let ev = MidiEvent::from_bytes(&[0x90, 0x3C, 0x64], 0.0).unwrap();
        assert_eq!(ev.channel, 0);
        assert_eq!(
            ev.kind,
            MidiEventKind::NoteOn {
                note: 60,
                velocity: 100
            }
        );
    }

    #[test]
    fn parse_note_off() {
        let ev = MidiEvent::from_bytes(&[0x81, 0x40, 0x00], 1.0).unwrap();
        assert_eq!(ev.channel, 1);
        assert_eq!(
            ev.kind,
            MidiEventKind::NoteOff {
                note: 64,
                velocity: 0
            }
        );
    }

    #[test]
    fn note_on_velocity_zero_becomes_note_off() {
        let ev = MidiEvent::from_bytes(&[0x90, 0x3C, 0x00], 0.0).unwrap();
        assert_eq!(
            ev.kind,
            MidiEventKind::NoteOff {
                note: 60,
                velocity: 0
            }
        );
    }

    #[test]
    fn parse_control_change() {
        let ev = MidiEvent::from_bytes(&[0xB2, 0x07, 0x7F], 2.0).unwrap();
        assert_eq!(ev.channel, 2);
        assert_eq!(
            ev.kind,
            MidiEventKind::ControlChange {
                controller: 7,
                value: 127
            }
        );
    }

    #[test]
    fn parse_program_change() {
        let ev = MidiEvent::from_bytes(&[0xC0, 0x05], 3.0).unwrap();
        assert_eq!(ev.channel, 0);
        assert_eq!(ev.kind, MidiEventKind::ProgramChange { program: 5 });
    }

    #[test]
    fn parse_pitch_bend() {
        // Center position: LSB=0, MSB=64 => value = 8192 - 8192 = 0
        let ev = MidiEvent::from_bytes(&[0xE0, 0x00, 0x40], 4.0).unwrap();
        assert_eq!(ev.channel, 0);
        assert_eq!(ev.kind, MidiEventKind::PitchBend { value: 0 });
    }

    #[test]
    fn parse_pitch_bend_max() {
        // Max: LSB=127, MSB=127 => (127<<7)|127 = 16383 => 16383-8192 = 8191
        let ev = MidiEvent::from_bytes(&[0xE0, 0x7F, 0x7F], 0.0).unwrap();
        assert_eq!(ev.kind, MidiEventKind::PitchBend { value: 8191 });
    }

    #[test]
    fn roundtrip_note_on() {
        let original = MidiEvent {
            channel: 3,
            kind: MidiEventKind::NoteOn {
                note: 72,
                velocity: 96,
            },
            timestamp: PI,
        };
        let bytes = original.to_bytes();
        let parsed = MidiEvent::from_bytes(&bytes, PI).unwrap();
        assert_eq!(original.channel, parsed.channel);
        assert_eq!(original.kind, parsed.kind);
    }

    #[test]
    fn roundtrip_pitch_bend() {
        let original = MidiEvent {
            channel: 5,
            kind: MidiEventKind::PitchBend { value: -1000 },
            timestamp: 0.0,
        };
        let bytes = original.to_bytes();
        let parsed = MidiEvent::from_bytes(&bytes, 0.0).unwrap();
        assert_eq!(original.kind, parsed.kind);
    }

    #[test]
    fn empty_bytes_error() {
        let err = MidiEvent::from_bytes(&[], 0.0).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidMidiBytes(_)));
    }

    #[test]
    fn unsupported_status() {
        let err = MidiEvent::from_bytes(&[0xF0, 0x00], 0.0).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidMidiBytes(_)));
    }

    #[test]
    fn note_helpers() {
        let on = MidiEvent {
            channel: 0,
            kind: MidiEventKind::NoteOn {
                note: 60,
                velocity: 100,
            },
            timestamp: 0.0,
        };
        assert!(on.is_note_on());
        assert!(!on.is_note_off());
        assert_eq!(on.note(), Some(60));

        let off = MidiEvent {
            channel: 0,
            kind: MidiEventKind::NoteOff {
                note: 60,
                velocity: 0,
            },
            timestamp: 0.0,
        };
        assert!(off.is_note_off());
        assert!(!off.is_note_on());

        let cc = MidiEvent {
            channel: 0,
            kind: MidiEventKind::ControlChange {
                controller: 1,
                value: 64,
            },
            timestamp: 0.0,
        };
        assert!(!cc.is_note_on());
        assert!(!cc.is_note_off());
        assert_eq!(cc.note(), None);
    }
}
