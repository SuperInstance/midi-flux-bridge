//! # midi-flux-bridge
//!
//! Bridges tensor-midi timing schedules to FLUX coordination bytecode.
//! Agent dialogue timing computed as tensor contractions becomes executable
//! coordination instructions.
//!
//! ## Modules
//!
//! - [`tensor_schedule`] — Import tensor-midi-style timing schedules
//! - [`flux_bytecode`] — Generate FLUX timing instructions
//! - [`bridge`] — Core bridge: TensorSchedule → Vec<FluxOp>
//! - [`conductor`] — Execute FLUX timing bytecode
//! - [`swing`] — Swing timing computation and groove templates
//! - [`alignment`] — Agent alignment verification and drift detection

pub mod alignment;
pub mod bridge;
pub mod conductor;
pub mod flux_bytecode;
pub mod swing;
pub mod tensor_schedule;

use serde::{Deserialize, Serialize};

/// Timing parameters for a single agent in the coordination schedule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTiming {
    pub agent_id: String,
    pub bpm: f64,
    pub swing: f64,
    pub offset_ms: f64,
    pub cadence: Cadence,
}

/// Cadence pattern for an agent's timing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Cadence {
    /// Regular fixed interval in milliseconds.
    Regular { interval_ms: f64 },
    /// Custom beat pattern where each value is a duration in ms.
    Pattern { beats: Vec<f64> },
    /// Reactive timing with a minimum gap between responses.
    Reactive { min_gap_ms: f64 },
}

/// A single FLUX timing instruction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FluxOp {
    /// Wait for the specified duration in milliseconds.
    Wait(f64),
    /// Signal a specific agent to act.
    Signal(String),
    /// Synchronization barrier; block until `n` agents have signaled.
    SyncBarrier(usize),
    /// Change the tempo to a new BPM.
    TempoChange(f64),
    /// Set the cadence for a named agent.
    CadenceSet(String, Cadence),
    /// Halt execution.
    Halt,
}

/// A complete FLUX schedule with metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FluxSchedule {
    pub ops: Vec<FluxOp>,
    pub total_duration_ms: f64,
    pub agent_count: usize,
}

/// Mutable state of the FLUX conductor during execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConductorState {
    pub time_ms: f64,
    pub active_agents: Vec<String>,
    pub pending_barriers: usize,
    pub current_bpm: f64,
}

impl ConductorState {
    /// Create a new conductor state at time zero with the given BPM.
    pub fn new(bpm: f64) -> Self {
        Self {
            time_ms: 0.0,
            active_agents: Vec::new(),
            pending_barriers: 0,
            current_bpm: bpm,
        }
    }
}

impl FluxSchedule {
    /// Create an empty schedule.
    pub fn empty() -> Self {
        Self {
            ops: Vec::new(),
            total_duration_ms: 0.0,
            agent_count: 0,
        }
    }
}

impl Default for FluxSchedule {
    fn default() -> Self {
        Self::empty()
    }
}

impl Default for ConductorState {
    fn default() -> Self {
        Self::new(120.0)
    }
}
