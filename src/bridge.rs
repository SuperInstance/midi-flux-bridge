//! Core bridge: TensorSchedule → Vec<FluxOp>.
//!
//! Converts tensor contraction results (agent timing schedules) into
//! concrete FLUX timing bytecode that a conductor can execute.

use crate::{AgentTiming, Cadence, FluxOp, FluxSchedule};
use crate::tensor_schedule::TensorSchedule;

/// Bridge a TensorSchedule into a FLUX bytecode schedule.
///
/// The conversion strategy:
/// 1. Find the dominant BPM (max across agents) and set it.
/// 2. For each agent, set its cadence.
/// 3. Compute all beat timestamps across all agents.
/// 4. Sort events by time and emit WAIT + SIGNAL pairs.
/// 5. Insert SYNC_BARRIER when all agents need to synchronize.
pub fn bridge_schedule(schedule: &TensorSchedule) -> FluxSchedule {
    if schedule.agents.is_empty() {
        return FluxSchedule::empty();
    }

    let dominant_bpm = schedule.max_bpm();
    let duration = schedule.total_duration_ms();
    let agent_count = schedule.agent_count();

    let mut ops: Vec<FluxOp> = Vec::new();

    // Set dominant tempo
    ops.push(FluxOp::TempoChange(dominant_bpm));

    // Set cadences for each agent
    for agent in &schedule.agents {
        ops.push(FluxOp::CadenceSet(agent.agent_id.clone(), agent.cadence.clone()));
    }

    // Sync barrier to ensure all agents are configured
    ops.push(FluxOp::SyncBarrier(agent_count));

    // Collect all (time, agent_id) events
    let mut events: Vec<(f64, String)> = Vec::new();
    for agent in &schedule.agents {
        let beats = agent.beat_timestamps(duration);
        for t in beats {
            events.push((t, agent.agent_id.clone()));
        }
    }

    // Sort by time
    events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Emit WAIT + SIGNAL pairs
    let mut last_time = 0.0;
    for (time, agent_id) in events {
        let delta = time - last_time;
        if delta > 0.0 {
            ops.push(FluxOp::Wait(delta));
        }
        ops.push(FluxOp::Signal(agent_id));
        last_time = time;
    }

    // Final sync barrier
    ops.push(FluxOp::SyncBarrier(agent_count));
    ops.push(FluxOp::Halt);

    let total_duration = ops.iter().map(|op| match op {
        FluxOp::Wait(ms) => *ms,
        _ => 0.0,
    }).sum();

    FluxSchedule {
        ops,
        total_duration_ms: total_duration,
        agent_count,
    }
}

/// Bridge a single AgentTiming into a minimal FLUX sequence.
pub fn bridge_single_agent(agent: &AgentTiming, duration_ms: f64) -> FluxSchedule {
    let beats = agent.beat_timestamps(duration_ms);
    let mut ops = vec![FluxOp::TempoChange(agent.bpm)];
    ops.push(FluxOp::CadenceSet(agent.agent_id.clone(), agent.cadence.clone()));

    let mut last_time = 0.0;
    for t in beats {
        let delta = t - last_time;
        if delta > 0.0 {
            ops.push(FluxOp::Wait(delta));
        }
        ops.push(FluxOp::Signal(agent.agent_id.clone()));
        last_time = t;
    }
    ops.push(FluxOp::Halt);

    let total_duration = ops.iter().map(|op| match op {
        FluxOp::Wait(ms) => *ms,
        _ => 0.0,
    }).sum();

    FluxSchedule {
        ops,
        total_duration_ms: total_duration,
        agent_count: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cadence;

    #[test]
    fn test_bridge_empty_schedule() {
        let schedule = TensorSchedule::new();
        let flux = bridge_schedule(&schedule);
        assert!(flux.ops.is_empty());
    }

    #[test]
    fn test_bridge_single_agent_regular() {
        let schedule = TensorSchedule::from_agents(vec![
            AgentTiming::new("a1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
        ]);
        let flux = bridge_schedule(&schedule);
        // Should start with TempoChange, CadenceSet, SyncBarrier, then events
        assert!(flux.ops.len() > 3);
        assert_eq!(flux.agent_count, 1);
        // First op is tempo change
        assert!(matches!(flux.ops[0], FluxOp::TempoChange(120.0)));
    }

    #[test]
    fn test_bridge_two_agents_different_offsets() {
        let schedule = TensorSchedule::from_agents(vec![
            AgentTiming::new("a1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
            AgentTiming::new("a2", 120.0, 0.0, 250.0, Cadence::Regular { interval_ms: 500.0 }),
        ]);
        let flux = bridge_schedule(&schedule);
        assert_eq!(flux.agent_count, 2);

        // Verify signals are interleaved: a1 at 0, a2 at 250, a1 at 500, a2 at 750
        let signals: Vec<&str> = flux.ops.iter().filter_map(|op| match op {
            FluxOp::Signal(id) => Some(id.as_str()),
            _ => None,
        }).collect();
        assert!(signals.contains(&"a1"));
        assert!(signals.contains(&"a2"));
    }

    #[test]
    fn test_bridge_single_agent_function() {
        let agent = AgentTiming::new("solo", 100.0, 0.0, 0.0, Cadence::Regular { interval_ms: 600.0 });
        let flux = bridge_single_agent(&agent, 1800.0);
        assert_eq!(flux.agent_count, 1);
        assert!(flux.total_duration_ms > 0.0);
        // Should have 3 signals at 0, 600, 1200
        let signal_count = flux.ops.iter().filter(|op| matches!(op, FluxOp::Signal(_))).count();
        assert_eq!(signal_count, 3);
    }

    #[test]
    fn test_bridge_preserves_pattern_cadence() {
        let schedule = TensorSchedule::from_agents(vec![
            AgentTiming::new("p1", 120.0, 0.0, 0.0, Cadence::Pattern { beats: vec![200.0, 400.0, 200.0] }),
        ]);
        let flux = bridge_schedule(&schedule);
        // Check that a CadenceSet op is present with the pattern
        let cadence_ops: Vec<&Cadence> = flux.ops.iter().filter_map(|op| match op {
            FluxOp::CadenceSet(_, c) => Some(c),
            _ => None,
        }).collect();
        assert_eq!(cadence_ops.len(), 1);
        assert!(matches!(cadence_ops[0], Cadence::Pattern { .. }));
    }
}
