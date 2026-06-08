//! Tensor-midi-style timing schedule import.
//!
//! Models agent × time_slot × params as a timing schedule where each agent
//! receives BPM, swing, offset, and cadence parameters derived from tensor
//! contraction results.

use crate::{AgentTiming, Cadence};

/// A complete timing schedule for multiple agents, derived from tensor contractions.
/// A complete timing schedule for multiple agents derived from tensor contractions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TensorSchedule {
    pub agents: Vec<AgentTiming>,
}

impl TensorSchedule {
    /// Create an empty schedule.
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
        }
    }

    /// Create a schedule from a list of agent timings.
    pub fn from_agents(agents: Vec<AgentTiming>) -> Self {
        Self { agents }
    }

    /// Add an agent timing entry.
    pub fn add_agent(&mut self, timing: AgentTiming) {
        self.agents.push(timing);
    }

    /// Get the number of agents in the schedule.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Look up an agent by ID.
    pub fn get_agent(&self, agent_id: &str) -> Option<&AgentTiming> {
        self.agents.iter().find(|a| a.agent_id == agent_id)
    }

    /// Get the maximum BPM across all agents.
    pub fn max_bpm(&self) -> f64 {
        self.agents
            .iter()
            .map(|a| a.bpm)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Get the minimum BPM across all agents.
    pub fn min_bpm(&self) -> f64 {
        self.agents
            .iter()
            .map(|a| a.bpm)
            .fold(f64::INFINITY, f64::min)
    }

    /// Total duration spanned by all agent cadences (max end time).
    pub fn total_duration_ms(&self) -> f64 {
        self.agents
            .iter()
            .map(|a| a.offset_ms + a.cadence_duration_ms())
            .fold(0.0, f64::max)
    }
}

impl Default for TensorSchedule {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentTiming {
    /// Create a new agent timing with the given parameters.
    pub fn new(
        agent_id: impl Into<String>,
        bpm: f64,
        swing: f64,
        offset_ms: f64,
        cadence: Cadence,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            bpm,
            swing,
            offset_ms,
            cadence,
        }
    }

    /// Compute the total duration of this agent's cadence pattern.
    pub fn cadence_duration_ms(&self) -> f64 {
        match &self.cadence {
            Cadence::Regular { interval_ms } => *interval_ms,
            Cadence::Pattern { beats } => beats.iter().sum(),
            Cadence::Reactive { min_gap_ms } => *min_gap_ms,
        }
    }

    /// Compute the beat interval in ms for this agent at its BPM.
    pub fn beat_interval_ms(&self) -> f64 {
        60_000.0 / self.bpm
    }

    /// Generate beat timestamps for this agent over a given duration.
    /// Applies swing and offset.
    pub fn beat_timestamps(&self, duration_ms: f64) -> Vec<f64> {
        match &self.cadence {
            Cadence::Regular { interval_ms } => {
                let mut beats = Vec::new();
                let mut t = self.offset_ms;
                let mut beat_index = 0;
                while t < duration_ms {
                    beats.push(t);
                    let swing_offset = if beat_index % 2 == 1 {
                        self.swing * interval_ms
                    } else {
                        0.0
                    };
                    t += interval_ms + swing_offset;
                    beat_index += 1;
                }
                beats
            }
            Cadence::Pattern { beats: pattern } => {
                let mut result = Vec::new();
                let mut t = self.offset_ms;
                let mut pattern_idx = 0;
                while t < duration_ms {
                    result.push(t);
                    let base = pattern[pattern_idx % pattern.len()];
                    let swing_offset = if pattern_idx % 2 == 1 {
                        self.swing * base
                    } else {
                        0.0
                    };
                    t += base + swing_offset;
                    pattern_idx += 1;
                }
                result
            }
            Cadence::Reactive { min_gap_ms } => {
                // Reactive cadence: only emit the start
                if self.offset_ms < duration_ms {
                    vec![self.offset_ms]
                } else {
                    vec![]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cadence;

    #[test]
    fn test_tensor_schedule_new() {
        let schedule = TensorSchedule::new();
        assert_eq!(schedule.agent_count(), 0);
    }

    #[test]
    fn test_tensor_schedule_add_agent() {
        let mut schedule = TensorSchedule::new();
        schedule.add_agent(AgentTiming::new("a1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }));
        assert_eq!(schedule.agent_count(), 1);
    }

    #[test]
    fn test_tensor_schedule_get_agent() {
        let schedule = TensorSchedule::from_agents(vec![
            AgentTiming::new("alpha", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
            AgentTiming::new("beta", 140.0, 0.3, 50.0, Cadence::Regular { interval_ms: 428.57 }),
        ]);
        let a = schedule.get_agent("beta").unwrap();
        assert_eq!(a.bpm, 140.0);
        assert_eq!(a.swing, 0.3);
        assert!(schedule.get_agent("gamma").is_none());
    }

    #[test]
    fn test_agent_timing_beat_interval() {
        let agent = AgentTiming::new("test", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 });
        assert!((agent.beat_interval_ms() - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_agent_timing_cadence_duration_regular() {
        let agent = AgentTiming::new("test", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 });
        assert_eq!(agent.cadence_duration_ms(), 500.0);
    }

    #[test]
    fn test_agent_timing_cadence_duration_pattern() {
        let agent = AgentTiming::new("test", 120.0, 0.0, 0.0, Cadence::Pattern { beats: vec![100.0, 200.0, 300.0] });
        assert_eq!(agent.cadence_duration_ms(), 600.0);
    }

    #[test]
    fn test_tensor_schedule_max_min_bpm() {
        let schedule = TensorSchedule::from_agents(vec![
            AgentTiming::new("a", 100.0, 0.0, 0.0, Cadence::Regular { interval_ms: 600.0 }),
            AgentTiming::new("b", 140.0, 0.0, 0.0, Cadence::Regular { interval_ms: 428.57 }),
            AgentTiming::new("c", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
        ]);
        assert_eq!(schedule.max_bpm(), 140.0);
        assert_eq!(schedule.min_bpm(), 100.0);
    }

    #[test]
    fn test_agent_beat_timestamps_regular_no_swing() {
        let agent = AgentTiming::new("test", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 });
        let beats = agent.beat_timestamps(1500.0);
        assert_eq!(beats, vec![0.0, 500.0, 1000.0]);
    }

    #[test]
    fn test_agent_beat_timestamps_with_offset() {
        let agent = AgentTiming::new("test", 120.0, 0.0, 100.0, Cadence::Regular { interval_ms: 500.0 });
        let beats = agent.beat_timestamps(1600.0);
        assert_eq!(beats, vec![100.0, 600.0, 1100.0]);
    }

    #[test]
    fn test_agent_beat_timestamps_pattern() {
        let agent = AgentTiming::new("test", 120.0, 0.0, 0.0, Cadence::Pattern { beats: vec![200.0, 300.0] });
        let beats = agent.beat_timestamps(1000.0);
        assert_eq!(beats, vec![0.0, 200.0, 500.0, 700.0]);
    }
}
