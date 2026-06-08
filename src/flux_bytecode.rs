//! FLUX timing bytecode generation.
//!
//! Defines the FLUX instruction set for agent coordination timing:
//! WAIT, SIGNAL, SYNC_BARRIER, TEMPO_CHANGE, CADENCE_SET, and HALT.

use crate::{Cadence, FluxOp, FluxSchedule};

/// Builder for constructing FLUX bytecode schedules.
/// Builder for constructing FLUX bytecode schedules using a fluent API.
#[derive(Debug, Clone)]
pub struct FluxBuilder {
    ops: Vec<FluxOp>,
    agents: Vec<String>,
    duration_ms: f64,
}

impl FluxBuilder {
    /// Create a new empty FLUX builder.
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            agents: Vec::new(),
            duration_ms: 0.0,
        }
    }

    /// Add a WAIT instruction.
    pub fn wait(&mut self, ms: f64) -> &mut Self {
        self.ops.push(FluxOp::Wait(ms));
        self.duration_ms += ms;
        self
    }

    /// Add a SIGNAL instruction for an agent.
    pub fn signal(&mut self, agent_id: impl Into<String>) -> &mut Self {
        let id = agent_id.into();
        if !self.agents.contains(&id) {
            self.agents.push(id.clone());
        }
        self.ops.push(FluxOp::Signal(id));
        self
    }

    /// Add a SYNC_BARRIER instruction.
    pub fn sync_barrier(&mut self, n: usize) -> &mut Self {
        self.ops.push(FluxOp::SyncBarrier(n));
        self
    }

    /// Add a TEMPO_CHANGE instruction.
    pub fn tempo_change(&mut self, bpm: f64) -> &mut Self {
        self.ops.push(FluxOp::TempoChange(bpm));
        self
    }

    /// Add a CADENCE_SET instruction.
    pub fn cadence_set(&mut self, agent_id: impl Into<String>, cadence: Cadence) -> &mut Self {
        let id = agent_id.into();
        if !self.agents.contains(&id) {
            self.agents.push(id.clone());
        }
        self.ops.push(FluxOp::CadenceSet(id, cadence));
        self
    }

    /// Add a HALT instruction.
    pub fn halt(&mut self) -> &mut Self {
        self.ops.push(FluxOp::Halt);
        self
    }

    /// Build the final FluxSchedule.
    pub fn build(self) -> FluxSchedule {
        FluxSchedule {
            ops: self.ops,
            total_duration_ms: self.duration_ms,
            agent_count: self.agents.len(),
        }
    }
}

impl Default for FluxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the total duration of a sequence of FluxOps by summing all Wait durations.
pub fn compute_duration(ops: &[FluxOp]) -> f64 {
    ops.iter()
        .map(|op| match op {
            FluxOp::Wait(ms) => *ms,
            _ => 0.0,
        })
        .sum()
}

/// Extract all unique agent IDs from a list of FluxOps.
pub fn extract_agent_ids(ops: &[FluxOp]) -> Vec<String> {
    let mut ids = Vec::new();
    for op in ops {
        match op {
            FluxOp::Signal(id) | FluxOp::CadenceSet(id, _) => {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            _ => {}
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cadence;

    #[test]
    fn test_builder_empty() {
        let schedule = FluxBuilder::new().build();
        assert!(schedule.ops.is_empty());
        assert_eq!(schedule.total_duration_ms, 0.0);
        assert_eq!(schedule.agent_count, 0);
    }

    #[test]
    fn test_builder_wait_signal() {
        let mut builder = FluxBuilder::new();
        builder.wait(100.0).signal("agent-1");
        let schedule = builder.build();
        assert_eq!(schedule.ops.len(), 2);
        assert_eq!(schedule.total_duration_ms, 100.0);
        assert_eq!(schedule.agent_count, 1);
    }

    #[test]
    fn test_builder_full_sequence() {
        let mut builder = FluxBuilder::new();
        builder.wait(500.0).signal("a1").signal("a2").sync_barrier(2)
            .tempo_change(140.0)
            .cadence_set("a1", Cadence::Regular { interval_ms: 428.0 })
            .halt();
        let schedule = builder.build();
        assert_eq!(schedule.ops.len(), 7);
        assert_eq!(schedule.total_duration_ms, 500.0);
        assert_eq!(schedule.agent_count, 2);
    }

    #[test]
    fn test_compute_duration() {
        let ops = vec![FluxOp::Wait(100.0), FluxOp::Signal("a".into()), FluxOp::Wait(200.0)];
        assert_eq!(compute_duration(&ops), 300.0);
    }

    #[test]
    fn test_extract_agent_ids() {
        let ops = vec![
            FluxOp::Signal("a1".into()),
            FluxOp::Wait(100.0),
            FluxOp::Signal("a2".into()),
            FluxOp::Signal("a1".into()),
        ];
        let ids = extract_agent_ids(&ops);
        assert_eq!(ids, vec!["a1", "a2"]);
    }
}
