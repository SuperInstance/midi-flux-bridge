//! Agent alignment verification and drift detection.
//!
//! Checks that all agents respect timing constraints and detects drift
//! beyond a configurable tolerance.

use crate::AgentTiming;
use crate::tensor_schedule::TensorSchedule;

/// Result of an alignment check.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AlignmentResult {
    pub aligned: bool,
    pub max_drift_ms: f64,
    pub drifts: Vec<AgentDrift>,
}

/// Drift information for a single agent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AgentDrift {
    pub agent_id: String,
    pub expected_time_ms: f64,
    pub actual_time_ms: f64,
    pub drift_ms: f64,
    pub within_tolerance: bool,
}

/// Check alignment of actual event times against expected schedule.
///
/// Given expected timestamps and actual timestamps (both sorted by agent),
/// compute the drift for each agent and report whether all are within tolerance.
pub fn check_alignment(
    expected: &[(String, f64)],
    actual: &[(String, f64)],
    tolerance_ms: f64,
) -> AlignmentResult {
    let mut drifts = Vec::new();
    let mut max_drift = 0.0;

    for (exp_agent, exp_time) in expected {
        // Find matching actual time
        let act_time = actual
            .iter()
            .find(|(a, _)| a == exp_agent)
            .map(|(_, t)| *t)
            .unwrap_or(*exp_time);

        let drift = (act_time - exp_time).abs();
        if drift > max_drift {
            max_drift = drift;
        }

        drifts.push(AgentDrift {
            agent_id: exp_agent.clone(),
            expected_time_ms: *exp_time,
            actual_time_ms: act_time,
            drift_ms: drift,
            within_tolerance: drift <= tolerance_ms,
        });
    }

    let aligned = drifts.iter().all(|d| d.within_tolerance);

    AlignmentResult {
        aligned,
        max_drift_ms: max_drift,
        drifts,
    }
}

/// Check that all agents in a TensorSchedule have compatible BPM ranges.
///
/// Two agents are "BPM-compatible" if their beat intervals don't cause
/// drift beyond tolerance over the given duration.
pub fn check_bpm_compatibility(
    schedule: &TensorSchedule,
    duration_ms: f64,
    tolerance_ms: f64,
) -> AlignmentResult {
    if schedule.agents.len() < 2 {
        return AlignmentResult {
            aligned: true,
            max_drift_ms: 0.0,
            drifts: vec![],
        };
    }

    let reference = &schedule.agents[0];
    let ref_beat_ms = 60_000.0 / reference.bpm;

    let mut drifts = Vec::new();
    let mut max_drift = 0.0;

    for agent in &schedule.agents {
        let agent_beat_ms = 60_000.0 / agent.bpm;
        // Number of beats in duration
        let ref_beats = (duration_ms / ref_beat_ms).floor();
        let agent_beats = (duration_ms / agent_beat_ms).floor();

        // Drift = difference in total beats * reference beat interval
        let beat_diff = (ref_beats - agent_beats).abs();
        let drift = beat_diff * ref_beat_ms;

        if drift > max_drift {
            max_drift = drift;
        }

        drifts.push(AgentDrift {
            agent_id: agent.agent_id.clone(),
            expected_time_ms: duration_ms,
            actual_time_ms: duration_ms - drift,
            drift_ms: drift,
            within_tolerance: drift <= tolerance_ms,
        });
    }

    let aligned = drifts.iter().all(|d| d.within_tolerance);

    AlignmentResult {
        aligned,
        max_drift_ms: max_drift,
        drifts,
    }
}

/// Detect cumulative drift over multiple beats for a single agent.
///
/// Returns the list of drifts at each beat boundary.
pub fn detect_cumulative_drift(
    expected_interval_ms: f64,
    actual_interval_ms: f64,
    beat_count: usize,
) -> Vec<f64> {
    let mut drifts = Vec::with_capacity(beat_count);
    let mut cumulative = 0.0;
    for i in 1..=beat_count {
        let expected = expected_interval_ms * i as f64;
        let actual = actual_interval_ms * i as f64;
        cumulative = (actual - expected).abs();
        drifts.push(cumulative);
    }
    drifts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cadence;

    #[test]
    fn test_alignment_passes_for_on_time_agents() {
        let expected = vec![
            ("a1".into(), 0.0),
            ("a2".into(), 500.0),
        ];
        let actual = vec![
            ("a1".into(), 0.0),
            ("a2".into(), 500.0),
        ];
        let result = check_alignment(&expected, &actual, 10.0);
        assert!(result.aligned);
        assert_eq!(result.max_drift_ms, 0.0);
    }

    #[test]
    fn test_alignment_detects_drift() {
        let expected = vec![
            ("a1".into(), 0.0),
            ("a2".into(), 500.0),
        ];
        let actual = vec![
            ("a1".into(), 0.0),
            ("a2".into(), 520.0),
        ];
        let result = check_alignment(&expected, &actual, 10.0);
        assert!(!result.aligned);
        assert_eq!(result.max_drift_ms, 20.0);
    }

    #[test]
    fn test_alignment_within_tolerance() {
        let expected = vec![
            ("a1".into(), 0.0),
            ("a2".into(), 500.0),
        ];
        let actual = vec![
            ("a1".into(), 2.0),
            ("a2".into(), 498.0),
        ];
        let result = check_alignment(&expected, &actual, 10.0);
        assert!(result.aligned);
        assert!(result.max_drift_ms <= 10.0);
    }

    #[test]
    fn test_bpm_compatibility_same_bpm() {
        let schedule = TensorSchedule::from_agents(vec![
            AgentTiming::new("a1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
            AgentTiming::new("a2", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
        ]);
        let result = check_bpm_compatibility(&schedule, 5000.0, 50.0);
        assert!(result.aligned);
    }

    #[test]
    fn test_cumulative_drift() {
        let drifts = detect_cumulative_drift(500.0, 510.0, 5);
        assert!((drifts[0] - 10.0).abs() < 0.01);
        assert!((drifts[4] - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_alignment_single_agent() {
        let schedule = TensorSchedule::from_agents(vec![
            AgentTiming::new("solo", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
        ]);
        let result = check_bpm_compatibility(&schedule, 5000.0, 10.0);
        assert!(result.aligned);
    }
}
