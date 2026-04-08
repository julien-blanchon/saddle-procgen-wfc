# Saddle Procgen WFC

Reusable Wave Function Collapse / Model Synthesis toolkit for deterministic cartesian, stitched, overlap-derived, and hex-grid solving in Bevy.

`saddle-procgen-wfc` keeps the solver output generic: it produces tile identifiers, chosen quarter-turn rotations for auto-rotated families, solve stats, contradiction diagnostics, and optional debug snapshots. Spawning sprites, meshes, colliders, or gameplay entities is left to consuming crates and examples.

## When to Use WFC

WFC is the right choice when your content generation problem has **discrete tiles with local adjacency constraints** — meaning "what can sit next to what" is well-defined and the output must be locally coherent.

**Good fits:**

- **Dungeon and room layouts** — walls, floors, doors, corridors with strict adjacency rules
- **City blocks and street grids** — road tiles, intersections, dead-ends with connectivity rules
- **Tilemap terrain** — biome transitions (grass→sand→water) where every neighbor pair must make visual sense
- **Pipe/circuit networks** — connected pieces with fixed connector positions
- **Interior decoration** — furniture placement where adjacency rules prevent clashing objects
- **Voxel structures** — 3D buildings, caves, or underground layouts with block-type constraints
- **Hex-grid strategy maps** — terrain tiles on hexagonal grids with per-edge compatibility
- **Pattern-by-example** — learning tile patterns from a small authored sample via the overlap model

**Indicators WFC is right:**

- You can enumerate a small-to-medium tileset (tens to low hundreds of tile types)
- You can author or derive "tile A can be next to tile B in direction D" rules
- Local coherence is important — every 2x2 or 3x3 neighborhood should look intentional
- You want deterministic, reproducible output from a seed

## When Not to Use WFC

WFC is a constraint solver, not a general-purpose generator. It's the wrong tool when:

| Situation | Better alternative |
| --- | --- |
| **Continuous terrain** — smooth heightmaps, rolling hills, erosion | `saddle-procgen-noise` (Perlin, Simplex, FBM) |
| **Organic shapes** — rivers, coastlines, cave wall contours | Noise + marching squares/cubes |
| **Large open areas** with minimal constraints | Random scatter or Poisson disk sampling |
| **Tree-structured content** — skill trees, dialogue, quest graphs | Graph generators or grammar systems |
| **Freeform placement** where adjacency doesn't matter | Random/weighted spawn without constraints |
| **Real-time per-frame generation** with hard latency budgets | Pre-compute + cache; WFC backtracking time is unpredictable |
| **Very large grids** (256x256+) with complex rulesets | Chunk the problem with `WfcSeed::for_chunk` + boundary stitching, or use a hierarchical approach |

**Hybrid approaches** often work best. Use noise for macro-level structure (elevation, moisture, temperature), then WFC for the tile-level detail that needs local coherence. See the integration patterns below.

## Installation

```toml
saddle-procgen-wfc = { git = "https://github.com/julien-blanchon/saddle-procgen-wfc" }
```

## Quick Start

Pure solver usage:

```rust
use bevy::prelude::UVec3;
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

### Socket Builder (Ergonomic Ruleset Authoring)

Instead of manually specifying every `(tile, direction, allowed_tiles)` triple, label each tile face with a named socket. Adjacency is derived automatically — tiles connect if their facing sockets match:

```rust,ignore
use saddle_procgen_wfc::{
    WfcDirection, WfcGridSize, WfcRequest, WfcSeed, WfcSocketRulesetBuilder,
    WfcTileSymmetry, WfcTopology, solve_wfc,
};

let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d)
    .add_asymmetric_pair("pipe_in", "pipe_out"); // directional sockets

builder.add_tile(0u16, 5.0, "Grass").all_sockets("g").done();
builder.add_tile(1u16, 2.0, "Road")
    .socket(WfcDirection::XPos, "road")
    .socket(WfcDirection::XNeg, "road")
    .socket(WfcDirection::YPos, "g")
    .socket(WfcDirection::YNeg, "g")
    .symmetry(WfcTileSymmetry::Rotate2)
    .done();

let ruleset = builder.build().expect("should build");
let request = WfcRequest::new(WfcGridSize::new_2d(16, 16), ruleset, WfcSeed(7));
let solution = solve_wfc(&request).expect("should solve");
```

### Learn Adjacency from Example

Scan a hand-placed sample grid and extract adjacency rules automatically:

```rust,ignore
use saddle_procgen_wfc::{WfcTileGrid, WfcGridSize, WfcTopology, WfcTileId, learn_adjacency_rules};

let mut sample = WfcTileGrid::new_empty(WfcTopology::Cartesian2d, WfcGridSize::new_2d(8, 6));
// ... hand-place tiles with set_tile_at() ...
let ruleset = learn_adjacency_rules(&sample);
// ruleset is a standard WfcRuleset ready for solve_wfc()
```

### Serde Support

All public types implement `Serialize` and `Deserialize`. Load rulesets from RON, JSON, TOML, or any serde-compatible format:

```rust,ignore
let ruleset: WfcRuleset = ron::from_str(include_str!("rules.ron")).unwrap();
```

## Public API

| API | Purpose |
| --- | --- |
| `WfcPlugin` | Thin async Bevy runtime for background solves and job/result messages |
| `WfcSystems` | Public ordering hooks: `Request`, `PollJobs`, `ApplyResults`, `Debug` |
| `WfcRequest`, `WfcSettings`, `WfcSeed`, `WfcGridSize`, `WfcBoundaryStitching` | Core request/config surface |
| `WfcRuleset`, `WfcTileDefinition`, `WfcAdjacencyRule`, `WfcTileSymmetry` | Explicit tiled-model rules plus optional 2D auto-rotation |
| `WfcSocketRulesetBuilder`, `SocketTileBuilder`, `WfcSocketId` | Socket-based ruleset authoring — label faces, adjacency derived automatically |
| `learn_adjacency_rules(&WfcTileGrid)` | Learn tiled-model adjacency rules from a hand-placed sample grid |
| `WfcOverlapRequest`, `WfcOverlapOptions`, `solve_overlap_wfc_2d` | Sample-derived overlap-model solve path for 2D patch libraries |
| `WfcFixedCell`, `WfcCellBans`, `WfcBorderConstraint` | Local hard constraints |
| `WfcGlobalConstraint::TileCount` | Generic min/max tile count pruning |
| `GenerateWfc`, `WfcSolved`, `WfcFailed`, `WfcProgress` | Runtime request/result messages |
| `WfcJob`, `WfcJobResult`, `WfcRuntimeDiagnostics` | ECS-visible runtime inspection surface |
| `solve_wfc(&WfcRequest)` | Synchronous pure-Rust solve for tools and tests |
| `WfcStepSolver`, `WfcStepSnapshot`, `WfcStepCell` | Frame-by-frame stepping API for visualizations |
| `WfcSolution`, `WfcFailure`, `WfcSolveStats`, `WfcDebugSnapshot`, `WfcTileVariant` | Output, chosen rotations, and diagnostics |

## What Ships In v1

- Explicit adjacency-rule solving for 2D, 3D, and hex-grid topologies
- Optional 2D tile-family auto-rotation with `Fixed`, `Rotate2`, and `Rotate4` symmetry
- Boundary stitching through wrapped X/Y/Z neighbor evaluation for chunk seams and toroidal worlds
- Overlap-model solving from 2D sample patches via `solve_overlap_wfc_2d`
- Deterministic seeded observation and weighted sampling
- AC-3-style propagation over dense compatibility masks
- Explicit backtracking with rollback trail
- Fixed cells, per-cell bans, border restrictions, and tile-count constraints
- Async Bevy job workflow using `AsyncComputeTaskPool`
- Structured contradiction and debug snapshots for bad tilesets
- Step-by-step solver API (`WfcStepSolver`) for frame-by-frame visualizations
- Builder helpers: `with_symmetric_rule`, `with_all_direction_rules`, `From<u16>` for `WfcTileId`
- Socket-based ruleset authoring with `WfcSocketRulesetBuilder` — label faces, adjacency derived automatically
- `learn_adjacency_rules` — extract tiled-model rules from a hand-placed sample grid
- Full `serde` support — `Serialize`/`Deserialize` on all public types for data-driven workflows
- `WfcTileGrid` utility methods: `new_empty`, `set_tile_at`, `iter`, `iter_variants`
- `Display` impls for `WfcTileId`, `WfcDirection`, `WfcTopology`, `WfcFailureReason`
- Stable FNV-1a signature hash (deterministic across Rust versions)

## Design Notes

- The crate starts with the simple tiled model from the original `mxgmn` and Merrell model-synthesis lineage because that keeps the public API explicit and output-agnostic.
- Propagation is documented and implemented as an AC-3-style queue over dense compatibility masks, following the practical CSP framing from BorisTheBrave’s WFC and arc-consistency writeups.
- Backtracking is explicit, journaled, and inspectable instead of cloning whole solver states, which keeps failure analysis readable and leaves room for future support-count backends.
- `WfcSeed::for_chunk` exists so chunked or streaming consumers can derive deterministic local seeds without baking host-game concepts into the crate.
- Rotated tile families stay authored as one logical `WfcTileId`; the solver expands them internally and reports the chosen rotation per cell in `WfcTileGrid`.

## Integration with Other Saddles

WFC output is intentionally generic — a grid of `WfcTileId` values with optional rotations. The real power comes from feeding that output into other crates. Here are the tested integration patterns:

### WFC → Tilemap (`saddle-world-tilemap`)

Map `WfcTileId` to `TileKindId`, populate a `Tilemap` with `set_tile`:

```rust,ignore
use saddle_procgen_wfc::{WfcSolution, WfcTileId};
use saddle_world_tilemap::{Tilemap, TileCell, TileCoord, TileKindId};

fn populate_tilemap(map: &mut Tilemap, layer: TileLayerId, solution: &WfcSolution) {
    for y in 0..solution.grid.size.height {
        for x in 0..solution.grid.size.width {
            let wfc_tile = solution.grid.tile_at(UVec3::new(x, y, 0)).unwrap();
            let kind = match wfc_tile.0 {
                0 => FLOOR_KIND,
                1 => WALL_KIND,
                _ => FLOOR_KIND,
            };
            map.set_tile(layer, TileCoord::new(x as i32, y as i32), TileCell::new(kind));
        }
    }
}
```

The tilemap crate handles rendering, collision descriptors, autotiling, and pathfinding automatically. See the `tilemap_overlap` and `tilemap_dungeon` examples.

### WFC → FOV (`saddle-ai-fov`)

Build a `GridOpacityMap` from WFC output to drive grid-based field-of-view:

```rust,ignore
use saddle_ai_fov::{GridMapSpec, GridOpacityMap};

fn build_opacity_grid(solution: &WfcSolution, cell_size: f32) -> GridOpacityMap {
    let spec = GridMapSpec {
        origin: Vec2::ZERO,
        dimensions: UVec2::new(solution.grid.size.width, solution.grid.size.height),
        cell_size: Vec2::splat(cell_size),
    };
    GridOpacityMap::from_fn(spec, |cell| {
        let tile = solution.grid.tile_at(UVec3::new(cell.x as u32, cell.y as u32, 0));
        tile.map_or(true, |t| t.0 == WALL_TILE_ID)
    })
}
```

Wall tiles block vision, floor tiles are transparent. Spawn `GridFov` viewers on entities and read `GridFovState::visible_now` for visibility. See the `tilemap_dungeon` example.

### WFC → Navmesh (`saddle-ai-navmesh`)

Generate floor geometry from WFC tiles, spawn as navmesh sources:

```rust,ignore
// For each WFC floor tile, spawn a quad at the tile's world position
for y in 0..solution.grid.size.height {
    for x in 0..solution.grid.size.width {
        if solution.grid.tile_at(UVec3::new(x, y, 0)).unwrap().0 == FLOOR_TILE_ID {
            commands.spawn((
                NavmeshSource::new(surface, NavmeshSourceKind::Walkable),
                NavmeshPrimitiveSource::new(NavmeshPrimitive::Quad { size: Vec2::splat(tile_size) }),
                Transform::from_xyz(x as f32 * tile_size, 0.0, y as f32 * tile_size),
            ));
        }
    }
}
```

The navmesh plugin auto-bakes from source geometry, and agents pathfind across the procedural layout. See the `navmesh_dungeon` example.

### WFC → Voxel World (`saddle-world-voxel-world`)

Implement `VoxelBlockSampler` backed by a pre-solved WFC 3D grid:

```rust,ignore
use saddle_world_voxel_world::{BlockId, VoxelBlockSampler, VoxelWorldConfig};

struct WfcBlockSampler { grid: WfcTileGrid }

impl VoxelBlockSampler for WfcBlockSampler {
    fn sample_block(&self, world_pos: IVec3, _config: &VoxelWorldConfig) -> BlockId {
        let tile = self.grid.tile_at(UVec3::new(
            world_pos.x as u32, world_pos.y as u32, world_pos.z as u32,
        ));
        match tile.map(|t| t.0) {
            Some(0) => BlockId::AIR,
            Some(1) => BlockId::SOLID,
            _ => BlockId::AIR,
        }
    }
}
```

The voxel world handles chunk streaming, meshing, and rendering. See the `voxel_wfc` example.

### Combining Multiple Crates

The most compelling use case is layering several crates together:

1. **WFC** generates the dungeon layout (wall/floor/door tiles)
2. **Tilemap** renders it with autotiling and collision descriptors
3. **FOV** reads the wall grid for visibility computation
4. **Navmesh** bakes walkable floor geometry for agent pathfinding

See the `tilemap_dungeon` example for this full pipeline in action.

## Examples

| Example | Description | Run |
| --- | --- | --- |
| `basic_2d` | Pane-driven tiled solve with live seed and grid controls | `cargo run -p saddle-procgen-wfc-example-basic-2d` |
| `auto_rotation` | Pane-driven quarter-turn road showcase using `WfcTileSymmetry` | `cargo run -p saddle-procgen-wfc-example-auto-rotation` |
| `constrained_room` | Pane-driven forced entrance/exit and border-constrained room layout | `cargo run -p saddle-procgen-wfc-example-constrained-room` |
| `voxel_3d` | Pane-driven 3D voxel solve with live volume controls | `cargo run -p saddle-procgen-wfc-example-voxel-3d` |
| `async_runtime` | Message-driven async solve with live request controls | `cargo run -p saddle-procgen-wfc-example-async-runtime` |
| `debug_entropy` | Pane-driven contradiction heatmap with live seed control | `cargo run -p saddle-procgen-wfc-example-debug-entropy` |
| `tilemap_overlap` | Tilemap-facing overlap-model showcase driven from a learned patch sample | `cargo run -p saddle-procgen-wfc-example-tilemap-overlap` |
| `step_visualizer` | Watch WFC solve one cell at a time with entropy display and pause/resume | `cargo run -p saddle-procgen-wfc-example-step-visualizer` |
| `interactive` | Click to pin tiles, WFC fills the rest around your placements | `cargo run -p saddle-procgen-wfc-example-interactive` |
| `socket_builder` | Socket-based ruleset authoring with symmetric/asymmetric sockets and rotation | `cargo run -p saddle-procgen-wfc-example-socket-builder` |
| `data_driven` | Load a ruleset from a RON file — demonstrates serde integration | `cargo run -p saddle-procgen-wfc-example-data-driven` |
| `learned_rules` | Hand-place a sample, learn rules, generate from learned rules (side-by-side) | `cargo run -p saddle-procgen-wfc-example-learned-rules` |
| `tilemap_dungeon` | WFC dungeon → tilemap rendering + FOV visibility + pathfinding | `cargo run -p saddle-procgen-wfc-example-tilemap-dungeon` |
| `navmesh_dungeon` | WFC floor layout → 3D navmesh bake + agent pathfinding | `cargo run -p saddle-procgen-wfc-example-navmesh-dungeon` |
| `voxel_wfc` | WFC 3D solver → voxel world block sampler with chunk streaming | `cargo run -p saddle-procgen-wfc-example-voxel-wfc` |
| `saddle-procgen-wfc-lab` | Crate-local BRP/E2E verification app | `cargo run -p saddle-procgen-wfc-lab` |

Interactive examples can auto-exit for scripted verification with `WFC_EXAMPLE_EXIT_AFTER_SECONDS=<seconds>`.

## Scope Notes

- v1 now ships both the explicit tiled model and a sample-derived 2D overlap-model helper.
- Richer overlap features such as rotated pattern augmentation, multi-tile objects, and larger non-local authoring constraints are still future extensions.
- The runtime plugin solves in the background, but the pure solver is synchronous by design so tools and tests can use it directly.

See [docs/architecture.md](docs/architecture.md) for solver internals and [docs/configuration.md](docs/configuration.md) for every public knob.
