//! Integration tests for the full midi-flux-bridge pipeline.

use midi_flux_bridge::*;
use midi_flux_bridge::alignment::*;
use midi_flux_bridge::bridge::*;
use midi_flux_bridge::conductor::*;
use midi_flux_bridge::flux_bytecode::*;
use midi_flux_bridge::swing::*;
use midi_flux_bridge::tensor_schedule::*;

// --- Agent Timing Construction ---

#[test]
fn test_agent_timing_construction() {
    let agent = AgentTiming::new("alpha", 140.0, 0.3, 50.0, Cadence::Regular { interval_ms: 428.57 });
    assert_eq!(agent.agent_id, "alpha");
    assert_eq!(agent.bpm, 140.0);
    assert_eq!(agent.swing, 0.3);
    assert_eq!(agent.offset_ms, 50.0);
}

#[test]
fn test_cadence_regular() {
    let c = Cadence::Regular { interval_ms: 500.0 };
    assert_eq!(c, Cadence::Regular { interval_ms: 500.0 });
}

#[test]
fn test_cadence_pattern() {
    let c = Cadence::Pattern { beats: vec![100.0, 200.0, 300.0] };
    if let Cadence::Pattern { beats } = c {
        assert_eq!(beats.len(), 3);
    }
}

#[test]
fn test_cadence_reactive() {
    let c = Cadence::Reactive { min_gap_ms: 250.0 };
    if let Cadence::Reactive { min_gap_ms } = c {
        assert_eq!(min_gap_ms, 250.0);
    }
}

// --- WAIT Instruction ---

#[test]
fn test_wait_advances_conductor_time() {
    let mut state = ConductorState::new(120.0);
    conductor::step(&mut state, &FluxOp::Wait(250.0));
    assert_eq!(state.time_ms, 250.0);
    conductor::step(&mut state, &FluxOp::Wait(350.0));
    assert_eq!(state.time_ms, 600.0);
}

// --- SIGNAL Instruction ---

#[test]
fn test_signal_fires_for_correct_agent() {
    let schedule = FluxSchedule {
        ops: vec![FluxOp::Signal("agent-x".into())],
        total_duration_ms: 0.0,
        agent_count: 1,
    };
    let result = conductor::execute_schedule(&schedule, 120.0);
    assert!(result.final_state.active_agents.contains(&"agent-x".into()));
    let signals: Vec<_> = result.events.iter().filter_map(|e| {
        if let ConductedEventType::SignalFired(id) = &e.event { Some(id.clone()) } else { None }
    }).collect();
    assert_eq!(signals, vec!["agent-x"]);
}

// --- SyncBarrier ---

#[test]
fn test_sync_barrier_blocks_until_n() {
    let schedule = FluxSchedule {
        ops: vec![
            FluxOp::Signal("a1".into()),
            FluxOp::Signal("a2".into()),
            FluxOp::Signal("a3".into()),
            FluxOp::SyncBarrier(3),
        ],
        total_duration_ms: 0.0,
        agent_count: 3,
    };
    let result = conductor::execute_schedule(&schedule, 120.0);
    let barrier = result.events.iter().find_map(|e| {
        if let ConductedEventType::BarrierReached { required, achieved } = &e.event {
            Some((*required, *achieved))
        } else {
            None
        }
    });
    assert_eq!(barrier, Some((3, 3)));
}

#[test]
fn test_sync_barrier_insufficient() {
    let schedule = FluxSchedule {
        ops: vec![
            FluxOp::Signal("a1".into()),
            FluxOp::SyncBarrier(3),
        ],
        total_duration_ms: 0.0,
        agent_count: 1,
    };
    let result = conductor::execute_schedule(&schedule, 120.0);
    let barrier = result.events.iter().find_map(|e| {
        if let ConductedEventType::BarrierReached { required, achieved } = &e.event {
            Some((*required, *achieved))
        } else {
            None
        }
    });
    assert_eq!(barrier, Some((3, 1)));
}

// --- TempoChange ---

#[test]
fn test_tempo_change_updates_bpm() {
    let mut state = ConductorState::new(120.0);
    conductor::step(&mut state, &FluxOp::TempoChange(160.0));
    assert_eq!(state.current_bpm, 160.0);
}

// --- Tensor → Flux Conversion ---

#[test]
fn test_tensor_to_flux_conversion() {
    let schedule = TensorSchedule::from_agents(vec![
        AgentTiming::new("a1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
    ]);
    let flux = bridge_schedule(&schedule);
    assert!(flux.ops.len() > 3);
    assert!(matches!(flux.ops[0], FluxOp::TempoChange(120.0)));
}

// --- Swing Timing ---

#[test]
fn test_swing_off_beats_delayed() {
    let beats = apply_swing(500.0, 0.5, 4, 0.0);
    // Beat 0 at 0, Beat 1 at 500 (no swing on even), Beat 2 at 500+500+250=1250
    assert!((beats[2] - 1250.0).abs() < 0.01);
}

#[test]
fn test_swing_no_swing_straight() {
    let beats = apply_swing(500.0, 0.0, 4, 0.0);
    assert_eq!(beats, vec![0.0, 500.0, 1000.0, 1500.0]);
}

// --- Groove Templates ---

#[test]
fn test_groove_straight() {
    let tmpl = straight_template();
    let beats = tmpl.apply(500.0, 4);
    assert_eq!(beats, vec![0.0, 500.0, 1000.0, 1500.0]);
}

#[test]
fn test_groove_triplet_ratio() {
    let tmpl = triplet_template();
    // 2:1 ratio means ratios are [2/3, 1/3]
    assert!((tmpl.ratios[0] - 2.0/3.0).abs() < 0.01);
    assert!((tmpl.ratios[1] - 1.0/3.0).abs() < 0.01);
}

#[test]
fn test_groove_dotted_ratio() {
    let tmpl = dotted_template();
    assert!((tmpl.ratios[0] - 0.75).abs() < 0.01);
    assert!((tmpl.ratios[1] - 0.25).abs() < 0.01);
}

// --- Alignment ---

#[test]
fn test_alignment_passes_on_time() {
    let expected = vec![("a1".into(), 100.0)];
    let actual = vec![("a1".into(), 100.0)];
    let result = check_alignment(&expected, &actual, 5.0);
    assert!(result.aligned);
}

#[test]
fn test_alignment_detects_drift() {
    let expected = vec![("a1".into(), 100.0)];
    let actual = vec![("a1".into(), 115.0)];
    let result = check_alignment(&expected, &actual, 10.0);
    assert!(!result.aligned);
    assert_eq!(result.max_drift_ms, 15.0);
}

// --- Full Pipeline ---

#[test]
fn test_full_pipeline_tensor_to_conductor() {
    // Create a tensor schedule
    let schedule = TensorSchedule::from_agents(vec![
        AgentTiming::new("voice-1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
        AgentTiming::new("voice-2", 120.0, 0.0, 250.0, Cadence::Regular { interval_ms: 500.0 }),
    ]);

    // Bridge to flux bytecode
    let flux = bridge_schedule(&schedule);
    assert!(flux.ops.len() > 5);
    assert_eq!(flux.agent_count, 2);

    // Execute with conductor
    let result = conductor::execute_schedule(&flux, 120.0);
    assert!(result.final_state.time_ms > 0.0);
    assert!(result.final_state.active_agents.contains(&"voice-1".into()));
    assert!(result.final_state.active_agents.contains(&"voice-2".into()));
}

#[test]
fn test_multi_agent_different_cadences_interleaved() {
    let schedule = TensorSchedule::from_agents(vec![
        AgentTiming::new("fast", 160.0, 0.0, 0.0, Cadence::Regular { interval_ms: 375.0 }),
        AgentTiming::new("slow", 80.0, 0.0, 0.0, Cadence::Regular { interval_ms: 750.0 }),
    ]);

    let flux = bridge_schedule(&schedule);
    let result = conductor::execute_schedule(&flux, 120.0);

    // Both agents should be active
    assert!(result.final_state.active_agents.contains(&"fast".into()));
    assert!(result.final_state.active_agents.contains(&"slow".into()));

    // Fast agent should have more signal events
    let fast_signals = result.events.iter().filter(|e| {
        matches!(&e.event, ConductedEventType::SignalFired(id) if id == "fast")
    }).count();
    let slow_signals = result.events.iter().filter(|e| {
        matches!(&e.event, ConductedEventType::SignalFired(id) if id == "slow")
    }).count();
    assert!(fast_signals > slow_signals);
}

#[test]
fn test_conductor_executes_full_schedule() {
    let schedule = FluxSchedule {
        ops: vec![
            FluxOp::TempoChange(100.0),
            FluxOp::CadenceSet("a".into(), Cadence::Regular { interval_ms: 600.0 }),
            FluxOp::Wait(600.0),
            FluxOp::Signal("a".into()),
            FluxOp::Wait(600.0),
            FluxOp::Signal("a".into()),
            FluxOp::Halt,
        ],
        total_duration_ms: 1200.0,
        agent_count: 1,
    };
    let result = conductor::execute_schedule(&schedule, 120.0);
    assert_eq!(result.final_state.time_ms, 1200.0);
    assert_eq!(result.final_state.current_bpm, 100.0);
    assert_eq!(result.events.len(), 7);
}

#[test]
fn test_serde_roundtrip_flux_schedule() {
    let schedule = FluxSchedule {
        ops: vec![
            FluxOp::Wait(100.0),
            FluxOp::Signal("agent-1".into()),
            FluxOp::SyncBarrier(1),
            FluxOp::TempoChange(140.0),
            FluxOp::Halt,
        ],
        total_duration_ms: 100.0,
        agent_count: 1,
    };
    let json = serde_json::to_string(&schedule).unwrap();
    let parsed: FluxSchedule = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, schedule);
}

#[test]
fn test_serde_roundtrip_agent_timing() {
    let agent = AgentTiming::new("test", 120.0, 0.3, 50.0, Cadence::Pattern { beats: vec![100.0, 200.0] });
    let json = serde_json::to_string(&agent).unwrap();
    let parsed: AgentTiming = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, agent);
}

#[test]
fn test_builder_pattern_chaining() {
    let mut builder = FluxBuilder::new();
    builder.tempo_change(130.0)
        .cadence_set("drums", Cadence::Regular { interval_ms: 461.54 })
        .sync_barrier(1)
        .wait(461.54)
        .signal("drums")
        .halt();
    let schedule = builder.build();
    assert_eq!(schedule.ops.len(), 6);
    assert_eq!(schedule.agent_count, 1);
}

#[test]
fn test_alignment_cumulative_drift_grows() {
    let drifts = detect_cumulative_drift(500.0, 510.0, 10);
    assert!((drifts[0] - 10.0).abs() < 0.01);
    assert!((drifts[9] - 100.0).abs() < 0.01);
    // Drift should be monotonically increasing
    for i in 1..drifts.len() {
        assert!(drifts[i] >= drifts[i-1]);
    }
}
