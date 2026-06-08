//! FLUX conductor: executes FLUX timing bytecode.
//!
//! Advances time, fires signals, handles sync barriers, and processes
//! tempo changes in order.

use crate::{Cadence, ConductorState, FluxOp, FluxSchedule};

/// A recorded event from conductor execution.
/// A recorded event from conductor execution with timestamp.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConductedEvent {
    pub time_ms: f64,
    pub event: ConductedEventType,
}

/// The type of event that occurred during execution.
/// Types of events that occur during FLUX bytecode execution.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConductedEventType {
    Wait(f64),
    SignalFired(String),
    BarrierReached { required: usize, achieved: usize },
    TempoChanged { from: f64, to: f64 },
    CadenceSet { agent_id: String },
    Halted,
}

/// Result of executing a FLUX schedule.
/// Result of executing a complete FLUX schedule.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConductedResult {
    pub events: Vec<ConductedEvent>,
    pub final_state: ConductorState,
}

/// Execute a complete FLUX schedule and return all events.
pub fn execute_schedule(schedule: &FluxSchedule, initial_bpm: f64) -> ConductedResult {
    let mut state = ConductorState::new(initial_bpm);
    let mut events = Vec::new();
    let mut barrier_signaled: Vec<String> = Vec::new();

    for op in &schedule.ops {
        let event = match op {
            FluxOp::Wait(ms) => {
                state.time_ms += ms;
                ConductedEvent {
                    time_ms: state.time_ms,
                    event: ConductedEventType::Wait(*ms),
                }
            }
            FluxOp::Signal(agent_id) => {
                if !state.active_agents.contains(agent_id) {
                    state.active_agents.push(agent_id.clone());
                }
                barrier_signaled.push(agent_id.clone());
                ConductedEvent {
                    time_ms: state.time_ms,
                    event: ConductedEventType::SignalFired(agent_id.clone()),
                }
            }
            FluxOp::SyncBarrier(n) => {
                let achieved = barrier_signaled.len();
                let reached = achieved >= *n;
                if reached {
                    barrier_signaled.clear();
                    state.pending_barriers = 0;
                } else {
                    state.pending_barriers = *n - achieved;
                }
                ConductedEvent {
                    time_ms: state.time_ms,
                    event: ConductedEventType::BarrierReached {
                        required: *n,
                        achieved,
                    },
                }
            }
            FluxOp::TempoChange(bpm) => {
                let from = state.current_bpm;
                state.current_bpm = *bpm;
                ConductedEvent {
                    time_ms: state.time_ms,
                    event: ConductedEventType::TempoChanged { from, to: *bpm },
                }
            }
            FluxOp::CadenceSet(agent_id, _cadence) => {
                ConductedEvent {
                    time_ms: state.time_ms,
                    event: ConductedEventType::CadenceSet {
                        agent_id: agent_id.clone(),
                    },
                }
            }
            FluxOp::Halt => {
                ConductedEvent {
                    time_ms: state.time_ms,
                    event: ConductedEventType::Halted,
                }
            }
        };
        events.push(event);
    }

    ConductedResult {
        events,
        final_state: state,
    }
}

/// Step through a single FluxOp, updating the conductor state.
pub fn step(state: &mut ConductorState, op: &FluxOp) -> ConductedEvent {
    match op {
        FluxOp::Wait(ms) => {
            state.time_ms += ms;
            ConductedEvent {
                time_ms: state.time_ms,
                event: ConductedEventType::Wait(*ms),
            }
        }
        FluxOp::Signal(agent_id) => {
            if !state.active_agents.contains(agent_id) {
                state.active_agents.push(agent_id.clone());
            }
            ConductedEvent {
                time_ms: state.time_ms,
                event: ConductedEventType::SignalFired(agent_id.clone()),
            }
        }
        FluxOp::SyncBarrier(n) => {
            state.pending_barriers = *n;
            ConductedEvent {
                time_ms: state.time_ms,
                event: ConductedEventType::BarrierReached {
                    required: *n,
                    achieved: 0,
                },
            }
        }
        FluxOp::TempoChange(bpm) => {
            let from = state.current_bpm;
            state.current_bpm = *bpm;
            ConductedEvent {
                time_ms: state.time_ms,
                event: ConductedEventType::TempoChanged { from, to: *bpm },
            }
        }
        FluxOp::CadenceSet(agent_id, _) => {
            ConductedEvent {
                time_ms: state.time_ms,
                event: ConductedEventType::CadenceSet {
                    agent_id: agent_id.clone(),
                },
            }
        }
        FluxOp::Halt => {
            ConductedEvent {
                time_ms: state.time_ms,
                event: ConductedEventType::Halted,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FluxOp;

    #[test]
    fn test_conductor_wait_advances_time() {
        let schedule = FluxSchedule {
            ops: vec![FluxOp::Wait(250.0)],
            total_duration_ms: 250.0,
            agent_count: 0,
        };
        let result = execute_schedule(&schedule, 120.0);
        assert_eq!(result.final_state.time_ms, 250.0);
    }

    #[test]
    fn test_conductor_signal_fires() {
        let schedule = FluxSchedule {
            ops: vec![FluxOp::Signal("agent-a".into())],
            total_duration_ms: 0.0,
            agent_count: 1,
        };
        let result = execute_schedule(&schedule, 120.0);
        assert!(result.final_state.active_agents.contains(&"agent-a".into()));
        let fired = result.events.iter().any(|e| matches!(&e.event, ConductedEventType::SignalFired(id) if id == "agent-a"));
        assert!(fired);
    }

    #[test]
    fn test_conductor_tempo_change() {
        let schedule = FluxSchedule {
            ops: vec![FluxOp::TempoChange(140.0)],
            total_duration_ms: 0.0,
            agent_count: 0,
        };
        let result = execute_schedule(&schedule, 120.0);
        assert_eq!(result.final_state.current_bpm, 140.0);
    }

    #[test]
    fn test_conductor_sync_barrier() {
        let schedule = FluxSchedule {
            ops: vec![
                FluxOp::Signal("a1".into()),
                FluxOp::Signal("a2".into()),
                FluxOp::SyncBarrier(2),
            ],
            total_duration_ms: 0.0,
            agent_count: 2,
        };
        let result = execute_schedule(&schedule, 120.0);
        let barrier_event = result.events.iter().find(|e| matches!(&e.event, ConductedEventType::BarrierReached { .. }));
        assert!(barrier_event.is_some());
        if let Some(e) = barrier_event {
            if let ConductedEventType::BarrierReached { required, achieved } = &e.event {
                assert_eq!(*required, 2);
                assert_eq!(*achieved, 2);
            }
        }
    }

    #[test]
    fn test_conductor_halt() {
        let schedule = FluxSchedule {
            ops: vec![FluxOp::Halt],
            total_duration_ms: 0.0,
            agent_count: 0,
        };
        let result = execute_schedule(&schedule, 120.0);
        let halted = result.events.last().unwrap();
        assert!(matches!(halted.event, ConductedEventType::Halted));
    }

    #[test]
    fn test_conductor_full_sequence() {
        let schedule = FluxSchedule {
            ops: vec![
                FluxOp::TempoChange(120.0),
                FluxOp::Wait(500.0),
                FluxOp::Signal("a1".into()),
                FluxOp::Wait(500.0),
                FluxOp::Signal("a2".into()),
                FluxOp::SyncBarrier(2),
                FluxOp::Halt,
            ],
            total_duration_ms: 1000.0,
            agent_count: 2,
        };
        let result = execute_schedule(&schedule, 100.0);
        assert_eq!(result.final_state.time_ms, 1000.0);
        assert_eq!(result.final_state.current_bpm, 120.0);
        assert_eq!(result.final_state.active_agents.len(), 2);
        assert_eq!(result.events.len(), 7);
    }

    #[test]
    fn test_step_function() {
        let mut state = ConductorState::new(120.0);
        let e = step(&mut state, &FluxOp::Wait(100.0));
        assert_eq!(state.time_ms, 100.0);
        assert!(matches!(e.event, ConductedEventType::Wait(100.0)));
    }
}
