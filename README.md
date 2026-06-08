# midi-flux-bridge

[![crates.io](https://img.shields.io/crates/v/midi-flux-bridge.svg)](https://crates.io/crates/midi-flux-bridge)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Converts multi-agent timing schedules into executable coordination bytecode. Agent dialogue timing — computed as tensor contractions over agent × time_slot × params — becomes a concrete sequence of FLUX instructions that a conductor steps through.

## The Problem

Coordinating multiple agents with independent tempos, swing ratios, offsets, and cadence patterns is a scheduling problem. Each agent has its own clock. The clocks drift. You need to know *when* each agent acts, detect when they've drifted too far, and produce a single linear schedule that interleaves them correctly.

Raw timing parameters (BPM, swing, offset, cadence pattern) aren't directly executable. You need to expand them into concrete timestamps, merge the timelines, and emit a sequential instruction stream.

## The Insight

The problem is structurally identical to MIDI sequencing, where multiple tracks with different quantizations and swing settings are merged into a single playback schedule. The "tensor" framing (agent × time_slot × params → schedule) is just a fancy way of saying "multi-track quantization." The solution is the same: expand each track to timestamps, interleave by time, emit wait/signal pairs.

The FLUX bytecode model (WAIT → SIGNAL → SYNC_BARRIER → HALT) is a minimal instruction set for temporal coordination. It's not a general-purpose VM — it only advances time, fires signals, and synchronizes. That's enough to express any interleaved multi-agent schedule.

## How It Works

The pipeline has four stages:

1. **TensorSchedule** — N agents, each with BPM, swing (0.0–1.0), offset_ms, and a `Cadence` (regular interval, custom beat pattern, or reactive with minimum gap). Beat timestamps are computed per-agent with swing applied to odd-indexed beats.

2. **bridge_schedule** — Merges all agents' timestamps into a single timeline. Strategy: find dominant BPM (max across agents), emit a `TempoChange`, set each agent's cadence, insert a `SyncBarrier`, then sort all (time, agent) events and emit `Wait(delta)` + `Signal(agent_id)` pairs.

3. **FluxBuilder** — Manual construction API for when you don't have a tensor schedule. Chain `.wait().signal().sync_barrier().halt()` calls.

4. **Conductor** — Executes a `FluxSchedule` by stepping through ops, advancing time on `Wait`, tracking active agents on `Signal`, clearing barriers when enough agents have signaled, and recording every event with its timestamp.

Alignment checking (`alignment` module) compares expected vs. actual timestamps and reports per-agent drift against a tolerance. BPM compatibility checking determines whether two agents will drift apart over a given duration. Swing computation provides groove templates (straight, triplet, dotted, custom) that apply timing ratios to a base interval.

## Code Example

```rust
use midi_flux_bridge::*;
use midi_flux_bridge::bridge::bridge_schedule;
use midi_flux_bridge::conductor::execute_schedule;
use midi_flux_bridge::alignment::{check_alignment, check_bpm_compatibility};

// Define a multi-agent schedule
let schedule = TensorSchedule::from_agents(vec![
    AgentTiming::new("lead", 120.0, 0.0, 0.0,
        Cadence::Regular { interval_ms: 500.0 }),
    AgentTiming::new("harmony", 120.0, 0.3, 250.0,
        Cadence::Regular { interval_ms: 500.0 }),
    AgentTiming::new("bass", 100.0, 0.0, 0.0,
        Cadence::Pattern { beats: vec![300.0, 300.0, 400.0] }),
]);

// Bridge to FLUX bytecode
let flux = bridge_schedule(&schedule);
assert!(flux.ops.len() > 3);
assert_eq!(flux.agent_count, 3);

// Execute and inspect events
let result = execute_schedule(&flux, 120.0);
for event in &result.events {
    println!("{:?} @ {:.1}ms", event.event, event.time_ms);
}
println!("Final time: {:.1}ms, BPM: {}", result.final_state.time_ms, result.final_state.current_bpm);

// Check BPM compatibility over 10 seconds
let compat = check_bpm_compatibility(&schedule, 10_000.0, 50.0);
if !compat.aligned {
    println!("Agents will drift: max_drift={:.1}ms", compat.max_drift_ms);
}

// Manual bytecode construction
let mut builder = midi_flux_bridge::flux_bytecode::FluxBuilder::new();
let schedule = builder
    .tempo_change(140.0)
    .cadence_set("drums", Cadence::Regular { interval_ms: 428.57 })
    .sync_barrier(1)
    .wait(428.57)
    .signal("drums")
    .halt()
    .build();
```

## Module Map

```
midi-flux-bridge
├── lib               Core types: AgentTiming, Cadence, FluxOp, FluxSchedule, ConductorState
├── tensor_schedule   TensorSchedule — multi-agent timing container, beat timestamp computation
├── bridge            bridge_schedule / bridge_single_agent — TensorSchedule → FluxSchedule
├── flux_bytecode     FluxBuilder — manual instruction construction, duration/agent extraction
├── conductor         execute_schedule / step — FLUX bytecode interpreter with event log
├── swing             GrooveTemplate, apply_swing — straight/triplet/dotted/custom groove patterns
└── alignment         check_alignment, check_bpm_compatibility, detect_cumulative_drift
```

## Design Decisions

**Dominant BPM is max, not median.** `bridge_schedule` picks the highest BPM as the dominant tempo. This means the fastest agent drives the clock. It's a conservative choice — slower agents get more `Wait` time between signals. An alternative (median BPM, or LCM of beat intervals) would reduce total wait time but complicate the sync model.

**Swing delays odd-indexed beats.** `apply_swing` adds `swing * beat_interval_ms` to every odd-indexed beat. This is the standard "shuffle" model — downbeats stay fixed, upbeats get pushed later. The `swing` field on `AgentTiming` is 0.0–1.0, not a ratio.

**`Cadence::Reactive` emits a single timestamp.** Reactive cadence is "fire once, then wait for external stimulus." The bridge emits one signal at the agent's offset, then nothing. This models event-driven agents that don't follow a beat.

**Sync barriers count signals, not agents.** The conductor tracks how many `Signal` ops have fired since the last barrier. When `achieved >= required`, the barrier clears. This means signals from *any* agent count toward the barrier — there's no per-agent tracking within a barrier window.

**No real-time execution.** The conductor is a pure function: `execute_schedule(&FluxSchedule, bpm) → ConductedResult`. It doesn't block, sleep, or interact with the outside world. Real-time scheduling is the caller's responsibility.

**Drift detection is linear.** `detect_cumulative_drift` computes `(actual_interval - expected_interval) * beat_number` for each beat. It doesn't model jitter, drift rate, or statistical distributions. It's a first-order check.

## Status

Early release. 57 tests passing. The full pipeline works: define agents → bridge to bytecode → execute and collect events. Alignment checking detects drift. Groove templates produce correct timing patterns.

Limitations:
- No MIDI input/output — this is a timing scheduler, not a MIDI driver
- No polyrhythm resolution — agents with incommensurate intervals will produce long schedules
- `bridge_schedule` sorts all events into a single timeline; there's no parallel execution model
- Swing is applied uniformly per-agent, not per-beat
- No serialization of `TensorSchedule` to/from MIDI files
- The conductor doesn't support branching, looping, or conditional scheduling
- `beat_timestamps` truncates to `duration_ms` but doesn't guarantee an integer number of beats

## License

MIT
