# Saddle Procgen WFC

Reusable Wave Function Collapse / Model Synthesis toolkit for deterministic 2D and 3D cartesian grid solving in Bevy.

`saddle-procgen-wfc` keeps the solver output generic: it produces tile identifiers, chosen quarter-turn rotations for auto-rotated families, solve stats, contradiction diagnostics, and optional debug snapshots. Spawning sprites, meshes, colliders, or gameplay entities is left to consuming crates and examples.

## Installation

```toml
saddle-procgen-wfc = { git = "https://github.com/julien-blanchon/saddle-procgen-wfc" }
```

## Quick Start

Pure solver usage:

```rust
use saddle_procgen_wfc::{
    WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcTileDefinition, WfcTileId,
    WfcTileSymmetry, WfcTopology, solve_wfc,
};

let meadow = WfcTileId(0);
let road = WfcTileId(1);
let water = WfcTileId(2);
let all = [meadow, road, water];

let ruleset = WfcRuleset::new(
    WfcTopology::Cartesian2d,
    vec![
        WfcTileDefinition::new(meadow, 3.0, "Meadow"),
        WfcTileDefinition::new(road, 1.0, "Road").with_symmetry(WfcTileSymmetry::Rotate2),
        WfcTileDefinition::new(water, 1.0, "Water"),
    ],
)
.with_rule(meadow, WfcDirection::XPos, all)
.with_rule(meadow, WfcDirection::XNeg, all)
.with_rule(meadow, WfcDirection::YPos, all)
.with_rule(meadow, WfcDirection::YNeg, all)
.with_rule(road, WfcDirection::XPos, [meadow])
.with_rule(road, WfcDirection::XNeg, [meadow])
.with_rule(road, WfcDirection::YPos, [meadow, road])
.with_rule(road, WfcDirection::YNeg, [meadow, road])
.with_rule(water, WfcDirection::XPos, [meadow, water])
.with_rule(water, WfcDirection::XNeg, [meadow, water])
.with_rule(water, WfcDirection::YPos, [meadow, water])
.with_rule(water, WfcDirection::YNeg, [meadow, water]);

let request = WfcRequest::new(WfcGridSize::new_2d(16, 16), ruleset, WfcSeed(7));
let solution = solve_wfc(&request)?;
println!("signature={}", solution.signature);
println!("rotation(0,0)={:?}", solution.grid.rotation_at(UVec3::new(0, 0, 0)));
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
| `WfcRuleset`, `WfcTileDefinition`, `WfcAdjacencyRule`, `WfcTileSymmetry` | Explicit tiled-model rules plus optional 2D auto-rotation |
| `WfcFixedCell`, `WfcCellBans`, `WfcBorderConstraint` | Local hard constraints |
| `WfcGlobalConstraint::TileCount` | Generic min/max tile count pruning |
| `GenerateWfc`, `WfcSolved`, `WfcFailed`, `WfcProgress` | Runtime request/result messages |
| `WfcJob`, `WfcJobResult`, `WfcRuntimeDiagnostics` | ECS-visible runtime inspection surface |
| `solve_wfc(&WfcRequest)` | Synchronous pure-Rust solve for tools and tests |
| `WfcSolution`, `WfcFailure`, `WfcSolveStats`, `WfcDebugSnapshot`, `WfcTileVariant` | Output, chosen rotations, and diagnostics |

## What Ships In v1

- Explicit adjacency-rule solving for 2D and 3D cartesian grids
- Optional 2D tile-family auto-rotation with `Fixed`, `Rotate2`, and `Rotate4` symmetry
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
- Rotated tile families stay authored as one logical `WfcTileId`; the solver expands them internally and reports the chosen rotation per cell in `WfcTileGrid`.

## Examples

| Example | Description | Run |
| --- | --- | --- |
| `basic_2d` | Pane-driven tiled solve with live seed and grid controls | `cargo run -p saddle-procgen-wfc-example-basic-2d` |
| `auto_rotation` | Pane-driven quarter-turn road showcase using `WfcTileSymmetry` | `cargo run -p saddle-procgen-wfc-example-auto-rotation` |
| `constrained_room` | Pane-driven forced entrance/exit and border-constrained room layout | `cargo run -p saddle-procgen-wfc-example-constrained-room` |
| `voxel_3d` | Pane-driven 3D voxel solve with live volume controls | `cargo run -p saddle-procgen-wfc-example-voxel-3d` |
| `async_runtime` | Message-driven async solve with live request controls | `cargo run -p saddle-procgen-wfc-example-async-runtime` |
| `debug_entropy` | Pane-driven contradiction heatmap with live seed control | `cargo run -p saddle-procgen-wfc-example-debug-entropy` |
| `saddle-procgen-wfc-lab` | Crate-local BRP/E2E verification app | `cargo run -p saddle-procgen-wfc-lab` |

Interactive examples can auto-exit for scripted verification with `WFC_EXAMPLE_EXIT_AFTER_SECONDS=<seconds>`.

## Scope Notes

- v1 focuses on the simple tiled model with explicit adjacency rules.
- Overlapping WFC, sample-derived rule extraction, and richer non-local constraints are intentionally left as future extensions.
- The runtime plugin solves in the background, but the pure solver is synchronous by design so tools and tests can use it directly.

See [docs/architecture.md](docs/architecture.md) for solver internals and [docs/configuration.md](docs/configuration.md) for every public knob.
