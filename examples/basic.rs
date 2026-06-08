//! Basic example: Bridge a tensor-midi schedule to FLUX bytecode and execute it.
//!
//! Run with: cargo run --example basic

use midi_flux_bridge::{
    AgentTiming, Cadence, TensorSchedule,
    FluxBuilder, FluxSchedule,
    bridge_schedule,
};
use midi_flux_bridge::conductor;
use midi_flux_bridge::swing::{straight_template, triplet_template, apply_swing};
use midi_flux_bridge::alignment;

fn main() {
    // === Create a tensor-midi timing schedule ===
    println!("=== Tensor Schedule ===");
    let schedule = TensorSchedule::from_agents(vec![
        AgentTiming::new("voice-1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
        AgentTiming::new("voice-2", 120.0, 0.0, 250.0, Cadence::Regular { interval_ms: 500.0 }),
        AgentTiming::new("drums",  120.0, 0.3, 0.0, Cadence::Pattern {
            beats: vec![250.0, 250.0, 500.0],
        }),
    ]);

    println!("Agents: {}", schedule.agent_count());
    println!("BPM range: {} - {}", schedule.min_bpm(), schedule.max_bpm());
    println!("Duration: {:.1} ms", schedule.total_duration_ms());

    // Generate beat timestamps for each agent
    for agent in &schedule.agents {
        let beats = agent.beat_timestamps(2000.0);
        println!("  {}: {} beats — {:?}", agent.agent_id, beats.len(), beats);
    }

    // === Bridge to FLUX bytecode ===
    println!("\n=== FLUX Bridge ===");
    let flux = bridge_schedule(&schedule);
    println!("Generated {} FLUX ops ({} agents, {:.0} ms)",
        flux.ops.len(), flux.agent_count, flux.total_duration_ms);
    for (i, op) in flux.ops.iter().enumerate() {
        println!("  [{:2}] {:?}", i, op);
    }

    // === Execute with conductor ===
    println!("\n=== Conductor Execution ===");
    let result = conductor::execute_schedule(&flux, 120.0);
    println!("Final time: {:.1} ms", result.final_state.time_ms);
    println!("Active agents: {:?}", result.final_state.active_agents);
    println!("Events: {}", result.events.len());

    // === Swing timing ===
    println!("\n=== Swing Templates ===");
    let straight = straight_template();
    println!("Straight: {:?}", straight.apply(500.0, 4));

    let triplet = triplet_template();
    println!("Triplet:  {:?}", triplet.apply(600.0, 4));

    let swung = apply_swing(500.0, 0.5, 4, 0.0);
    println!("Swing:    {:?}", swung);

    // === Alignment check ===
    println!("\n=== Alignment ===");
    let expected = vec![
        ("voice-1".into(), 0.0),
        ("voice-2".into(), 250.0),
    ];
    let actual = vec![
        ("voice-1".into(), 2.0),   // 2ms early
        ("voice-2".into(), 253.0), // 3ms late
    ];
    let result = alignment::check_alignment(&expected, &actual, 10.0);
    println!("Aligned: {} (max drift: {:.1} ms)", result.aligned, result.max_drift_ms);
}
