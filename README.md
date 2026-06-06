# midi-flux-bridge

Bridges **tensor-midi timing schedules** to **FLUX coordination bytecode**. Agent dialogue timing computed as tensor contractions becomes executable coordination instructions.

## Overview

`midi-flux-bridge` converts multi-agent timing schedules (derived from tensor contractions over agent × time_slot × params) into a concrete FLUX bytecode that a conductor can execute. This enables precise coordination of multiple agents with independent BPM, swing, offset, and cadence parameters.

## Modules

| Module | Description |
|---|---|
| [`tensor_schedule`] | Import tensor-midi-style timing: agent × time_slot × params → schedule. Each agent gets BPM, swing, offset, cadence. |
| [`flux_bytecode`] | Generate FLUX timing instructions: `WAIT(ms)`, `SIGNAL(agent_id)`, `SYNC_BARRIER(n_agents)`, `TEMPO_CHANGE(bpm)`, `CADENCE_SET(pattern)`. |
| [`bridge`] | Core bridge: `TensorSchedule → Vec<FluxOp>`. Tensor contraction results mapped to concrete timing bytecode. |
| [`conductor`] | Execute FLUX timing bytecode: advance time, fire signals, handle sync barriers, tempo changes. |
| [`swing`] | Swing timing computation: off-beat delay ratio, groove templates (straight, triplet, dotted, custom). |
| [`alignment`] | Agent alignment verification: check that all agents respect timing constraints. Detect drift beyond tolerance. |

## Core Types

```rust
struct AgentTiming {
    agent_id: String,
    bpm: f64,
    swing: f64,
    offset_ms: f64,
    cadence: Cadence,
}

enum Cadence {
    Regular { interval_ms: f64 },
    Pattern { beats: Vec<f64> },
    Reactive { min_gap_ms: f64 },
}

enum FluxOp {
    Wait(f64),
    Signal(String),
    SyncBarrier(usize),
    TempoChange(f64),
    CadenceSet(String, Cadence),
    Halt,
}

struct FluxSchedule {
    ops: Vec<FluxOp>,
    total_duration_ms: f64,
    agent_count: usize,
}

struct ConductorState {
    time_ms: f64,
    active_agents: Vec<String>,
    pending_barriers: usize,
    current_bpm: f64,
}
```

## Quick Start

### Create a Tensor Schedule

```rust
use midi_flux_bridge::{AgentTiming, Cadence, TensorSchedule};

let schedule = TensorSchedule::from_agents(vec![
    AgentTiming::new("voice-1", 120.0, 0.0, 0.0, Cadence::Regular { interval_ms: 500.0 }),
    AgentTiming::new("voice-2", 120.0, 0.3, 250.0, Cadence::Regular { interval_ms: 500.0 }),
]);
```

### Bridge to FLUX Bytecode

```rust
use midi_flux_bridge::bridge::bridge_schedule;

let flux = bridge_schedule(&schedule);
println!("Generated {} ops for {} agents", flux.ops.len(), flux.agent_count);
```

### Execute with the Conductor

```rust
use midi_flux_bridge::conductor::execute_schedule;

let result = execute_schedule(&flux, 120.0);
for event in &result.events {
    println!("{:?} @ {}ms", event.event, event.time_ms);
}
```

### Build FLUX Bytecode Manually

```rust
use midi_flux_bridge::flux_bytecode::FluxBuilder;
use midi_flux_bridge::{Cadence, FluxOp};

let mut builder = FluxBuilder::new();
builder.tempo_change(140.0)
    .cadence_set("drums", Cadence::Regular { interval_ms: 428.57 })
    .sync_barrier(1)
    .wait(428.57)
    .signal("drums")
    .halt();
let schedule = builder.build();
```

### Swing and Groove Templates

```rust
use midi_flux_bridge::swing::{straight_template, triplet_template, apply_swing};

// Straight: all beats equal
let straight = straight_template();
let beats = straight.apply(500.0, 4); // [0.0, 500.0, 1000.0, 1500.0]

// Triplet: 2:1 ratio
let triplet = triplet_template();

// Apply swing to a sequence
let swung = apply_swing(500.0, 0.3, 8, 0.0); // 30% swing on off-beats
```

### Alignment Checking

```rust
use midi_flux_bridge::alignment::{check_alignment, detect_cumulative_drift};

let expected = vec![("agent-1".into(), 500.0)];
let actual = vec![("agent-1".into(), 505.0)];
let result = check_alignment(&expected, &actual, 10.0);
assert!(result.aligned); // 5ms drift within 10ms tolerance
```

## Pipeline

```
TensorSchedule (agent × time_slot × params)
        │
        ▼
    bridge_schedule()
        │
        ▼
    FluxSchedule (Vec<FluxOp>)
        │
        ▼
    execute_schedule()
        │
        ▼
    ConductedResult (Vec<ConductedEvent> + final state)
```

## Serde Support

All public types derive `Serialize` and `Deserialize`:

```rust
let json = serde_json::to_string(&schedule).unwrap();
let parsed: FluxSchedule = serde_json::from_str(&json).unwrap();
```

## Testing

67 tests covering:
- Agent timing construction and beat timestamp computation
- WAIT instruction advancing conductor time
- SIGNAL firing for correct agents
- SyncBarrier blocking until N agents signal
- TempoChange updating BPM
- Tensor schedule → flux bytecode conversion
- Swing timing: off-beats delayed correctly
- Groove templates (straight, triplet, dotted, custom)
- Alignment checking: passes for on-time, detects drift
- Cumulative drift detection
- Full pipeline: tensor → bytecode → conductor execution
- Multi-agent interleaving with different cadences
- Serde roundtrip serialization

```bash
cargo test
```

## License

MIT
