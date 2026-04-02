# Saddle Procgen WFC

Reusable Wave Function Collapse / Model Synthesis toolkit for deterministic 2D and 3D cartesian grid solving in Bevy.

`saddle-procgen-wfc` keeps the solver output generic: it produces tile identifiers, solve stats, contradiction diagnostics, and optional debug snapshots. Spawning sprites, meshes, colliders, or gameplay entities is left to consuming crates and examples.

## Installation

```toml
saddle-procgen-wfc = { git = "https://github.com/julien-blanchon/saddle-procgen-wfc" }
```

## Quick Start

Pure solver usage:

```rust
use saddle_procgen_wfc::{
    WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcTileDefinition, WfcTileId,
    WfcTopology, solve_wfc,
};

let grass = WfcTileId(0);
let water = WfcTileId(1);
let all = [grass, water];

let ruleset = WfcRuleset::new(
    WfcTopology::Cartesian2d,
    vec![
        WfcTileDefinition::new(grass, 3.0, "Grass"),
        WfcTileDefinition::new(water, 1.0, "Water"),
    ],
)
.with_rule(grass, WfcDirection::XPos, all)
.with_rule(grass, WfcDirection::XNeg, all)
.with_rule(grass, WfcDirection::YPos, all)
.with_rule(grass, WfcDirection::YNeg, all)
.with_rule(water, WfcDirection::XPos, [grass, water])
.with_rule(water, WfcDirection::XNeg, [grass, water])
.with_rule(water, WfcDirection::YPos, [grass, water])
.with_rule(water, WfcDirection::YNeg, [grass, water]);

let request = WfcRequest::new(WfcGridSize::new_2d(16, 16), ruleset, WfcSeed(7));
let solution = solve_wfc(&request)?;
println!("signature={}", solution.signature);
# Ok::<(), saddle_procgen_wfc::WfcFailure>(())
```

Runtime Bevy integration:

```rust,no_run
use bevy::prelude::*;
use saddle_procgen_wfc::{GenerateWfc, WfcPlugin};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(WfcPlugin::default());
    app.add_systems(Startup, request_generation);
    app.run();
}

fn request_generation(mut writer: MessageWriter<GenerateWfc>) {
    # use saddle_procgen_wfc::{WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcTileDefinition, WfcTileId, WfcTopology};
    # let a = WfcTileId(0);
    # let ruleset = WfcRuleset::new(
    #     WfcTopology::Cartesian2d,
    #     vec![WfcTileDefinition::new(a, 1.0, "A")],
    # )
    # .with_rule(a, WfcDirection::XPos, [a])
    # .with_rule(a, WfcDirection::XNeg, [a])
    # .with_rule(a, WfcDirection::YPos, [a])
    # .with_rule(a, WfcDirection::YNeg, [a]);
    writer.write(GenerateWfc {
        request: WfcRequest::new(WfcGridSize::new_2d(8, 8), ruleset, WfcSeed(1)),
        label: Some("startup solve".into()),
    });
}
```

Use `WfcPlugin::new(activate, deactivate, update)` when the runtime job layer should follow your own schedules or state machine.

## Public API

| API | Purpose |
| --- | --- |
| `WfcPlugin` | Thin async Bevy runtime for background solves and job/result messages |
| `WfcSystems` | Public ordering hooks: `Request`, `PollJobs`, `ApplyResults`, `Debug` |
| `WfcRequest`, `WfcSettings`, `WfcSeed`, `WfcGridSize` | Core request/config surface |
| `WfcRuleset`, `WfcTileDefinition`, `WfcAdjacencyRule` | Explicit tiled-model rules |
| `WfcFixedCell`, `WfcCellBans`, `WfcBorderConstraint` | Local hard constraints |
| `WfcGlobalConstraint::TileCount` | Generic min/max tile count pruning |
| `GenerateWfc`, `WfcSolved`, `WfcFailed`, `WfcProgress` | Runtime request/result messages |
| `WfcJob`, `WfcJobResult`, `WfcRuntimeDiagnostics` | ECS-visible runtime inspection surface |
| `solve_wfc(&WfcRequest)` | Synchronous pure-Rust solve for tools and tests |
| `WfcSolution`, `WfcFailure`, `WfcSolveStats`, `WfcDebugSnapshot` | Output and diagnostics |

## What Ships In v1

- Explicit adjacency-rule solving for 2D and 3D cartesian grids
- Deterministic seeded observation and weighted sampling
- AC-3-style propagation over dense compatibility masks
- Explicit backtracking with rollback trail
- Fixed cells, per-cell bans, border restrictions, and tile-count constraints
- Async Bevy job workflow using `AsyncComputeTaskPool`
- Structured contradiction and debug snapshots for bad tilesets

## Design Notes

- The crate starts with the simple tiled model from the original `mxgmn` and Merrell model-synthesis lineage because that keeps the public API explicit and output-agnostic.
- Propagation is documented and implemented as an AC-3-style queue over dense compatibility masks, following the practical CSP framing from BorisTheBrave’s WFC and arc-consistency writeups.
- Backtracking is explicit, journaled, and inspectable instead of cloning whole solver states, which keeps failure analysis readable and leaves room for future support-count backends.
- `WfcSeed::for_chunk` exists so chunked or streaming consumers can derive deterministic local seeds without baking host-game concepts into the crate.

## Examples

| Example | Description | Run |
| --- | --- | --- |
| `basic_2d` | Minimal 2D tiled solve with colored tile preview | `cargo run -p saddle-procgen-wfc-example-basic-2d` |
| `constrained_room` | Forced entrance/exit and border-constrained room layout | `cargo run -p saddle-procgen-wfc-example-constrained-room` |
| `voxel_3d` | 3D voxel solve with stacked cube preview | `cargo run -p saddle-procgen-wfc-example-voxel-3d` |
| `async_runtime` | Message-driven async solve and result application | `cargo run -p saddle-procgen-wfc-example-async-runtime` |
| `debug_entropy` | Entropy and remaining-domain-count heatmap | `cargo run -p saddle-procgen-wfc-example-debug-entropy` |
| `saddle-procgen-wfc-lab` | Crate-local BRP/E2E verification app | `cargo run -p saddle-procgen-wfc-lab` |

Interactive examples can auto-exit for scripted verification with `WFC_EXAMPLE_EXIT_AFTER_SECONDS=<seconds>`.

## Scope Notes

- v1 focuses on the simple tiled model with explicit adjacency rules.
- Overlapping WFC, sample-derived rule extraction, and richer non-local constraints are intentionally left as future extensions.
- The runtime plugin solves in the background, but the pure solver is synchronous by design so tools and tests can use it directly.

See [docs/architecture.md](docs/architecture.md) for solver internals and [docs/configuration.md](docs/configuration.md) for every public knob.
