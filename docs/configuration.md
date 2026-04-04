# Configuration

## `WfcGridSize`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `width` | `u32` | `8` | Number of cells on X |
| `height` | `u32` | `8` | Number of cells on Y |
| `depth` | `u32` | `1` | Number of cells on Z; must stay `1` for `Cartesian2d` |

Helpers:

- `WfcGridSize::new_2d(width, height)`
- `WfcGridSize::new_3d(width, height, depth)`

## `WfcSeed`

`WfcSeed(u64)` is the only randomness input.

- Same seed + same request => same result or same failure path
- Different seeds can explore different weighted branches
- `WfcSeed::for_chunk(IVec3)` mixes a base seed with a chunk coordinate for deterministic streaming workflows

## `WfcRequest`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `grid_size` | `WfcGridSize` | `8x8x1` | Cell dimensions for the solve |
| `ruleset` | `WfcRuleset` | required | Tile definitions plus explicit adjacency |
| `seed` | `WfcSeed` | required | Deterministic branch order and weighted sampling |
| `settings` | `WfcSettings` | `WfcSettings::default()` | Solver heuristics and debug capture |
| `fixed_cells` | `Vec<WfcFixedCell>` | empty | Pre-collapsed cells applied before propagation |
| `banned_cells` | `Vec<WfcCellBans>` | empty | Per-cell domain removals applied before propagation |
| `border_constraints` | `Vec<WfcBorderConstraint>` | empty | Allowed tiles for border planes |
| `global_constraints` | `Vec<WfcGlobalConstraint>` | empty | Generic non-local feasibility bounds |

`WfcRequest::new(grid_size, ruleset, seed)` builds the default empty-constraints form and is the intended entry point for most callers.

## `WfcSettings`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `observation_heuristic` | `WfcObservationHeuristic` | `MinimumEntropy` | Chooses which unresolved cell gets observed next |
| `max_backtracks` | `u32` | `256` | Hard cap on branch rewinds before the solver reports failure |
| `capture_debug_snapshot` | `bool` | `false` | Stores per-cell domains, entropy, and contradiction data in outputs |

Tradeoffs:

- `MinimumEntropy` usually produces better solve stability on weighted tilesets.
- `MinimumRemainingValues` is cheaper and can be easier to reason about in tests.
- Higher `max_backtracks` improves robustness on contradiction-heavy rulesets but raises worst-case solve time.
- `capture_debug_snapshot` is useful for authoring and diagnostics, but it increases allocation and output size.

## `WfcRuleset`

| Field | Type | Effect |
| --- | --- | --- |
| `topology` | `WfcTopology` | Selects 2D or 3D active directions |
| `tiles` | `Vec<WfcTileDefinition>` | Declares all valid tile ids, family weights, and optional rotation symmetry |
| `adjacency` | `Vec<WfcAdjacencyRule>` | Explicit allowed-neighbor table |

Ruleset authoring requirements:

- tile ids must be unique
- weights must be finite and positive
- every tile must provide adjacency for every active direction
- each adjacency rule may reference only declared tiles

## `WfcTileDefinition`

| Field | Type | Effect |
| --- | --- | --- |
| `id` | `WfcTileId` | Stable generic identifier returned by the solver |
| `weight` | `f32` | Relative sampling probability for the whole tile family during observation |
| `label` | `String` | Human-readable authoring/debug name |
| `symmetry` | `WfcTileSymmetry` | Optional 2D auto-rotation mode: `Fixed`, `Rotate2`, or `Rotate4` |

Weights are relative, not normalized. A tile with weight `4.0` is sampled roughly four times as often as a tile with weight `1.0` when both are currently allowed. When auto-rotation is enabled, that family weight is divided across the generated rotated variants so the overall family frequency stays stable.

Helpers:

- `WfcTileDefinition::new(id, weight, label)`
- `WfcTileDefinition::with_symmetry(WfcTileSymmetry::Rotate2 | WfcTileSymmetry::Rotate4)`

## `WfcTileSymmetry`

Use symmetry only for `Cartesian2d` rulesets.

- `Fixed`: one authored orientation, no automatic rotation
- `Rotate2`: two unique quarter-turn states, useful for straight corridors or roads
- `Rotate4`: four unique quarter-turn states, useful for elbows, tees, or directional props

## `WfcAdjacencyRule`

| Field | Type | Effect |
| --- | --- | --- |
| `tile` | `WfcTileId` | Source tile |
| `direction` | `WfcDirection` | Neighbor direction from the source tile |
| `allowed_tiles` | `Vec<WfcTileId>` | Tiles permitted in that direction |

These rules are explicit. Missing tile-direction pairs are treated as authoring errors.

When a tile uses `WfcTileSymmetry`, author the adjacency rules only for its canonical orientation. The solver rotates those rules automatically for the other generated variants.

## Local Constraints

### `WfcFixedCell`

Pins a position to a single tile before propagation.

### `WfcCellBans`

Removes a set of tiles from one cell before propagation.

### `WfcBorderConstraint`

Restricts every cell on one border plane to the listed tiles.

This is the main v1 tool for chunk seams and fixed-boundary solves.
For auto-rotated tiles, border constraints apply to the full tile family, not just one specific quarter-turn.

## Global Constraints

### `WfcGlobalConstraint::TileCount`

| Field | Type | Effect |
| --- | --- | --- |
| `tile` | `WfcTileId` | Tile being counted |
| `min_count` | `Option<u32>` | Required lower bound |
| `max_count` | `Option<u32>` | Required upper bound |

These are checked as feasibility bounds throughout the solve, not just at the end.

## Runtime Resources and Components

### `WfcPlugin`

`WfcPlugin::new(activate, deactivate, update)` accepts injectable schedules so host apps can align the runtime job flow with their own state machine.

Public system sets:

- `WfcSystems::Request`
- `WfcSystems::PollJobs`
- `WfcSystems::ApplyResults`
- `WfcSystems::Debug`

### `WfcJob`

Stores:

- job id
- human-readable label
- current status
- original request

### `WfcJobResult`

Stores the final `WfcSolution` or `WfcFailure` on the job entity for BRP and debug tools.

### `WfcRuntimeDiagnostics`

Aggregate runtime counters:

- submitted jobs
- running jobs
- completed jobs
- failed jobs
- cancelled jobs
- last successful signature
- last failure reason

## Output Rotation Metadata

### `WfcTileGrid`

`WfcTileGrid::tile_at(position)` still returns the logical `WfcTileId`.

New helpers:

- `WfcTileGrid::rotation_at(position)` returns the chosen quarter-turn index for that cell
- `WfcTileGrid::variant_at(position)` returns a `WfcTileVariant { tile, rotation_steps }`

### `WfcDebugSnapshot`

Each `WfcCellDebug` now exposes both:

- `possible_tiles`: deduplicated logical tile ids
- `possible_variants`: logical tile id plus rotation for each remaining candidate

## Scaling Notes

Memory and runtime scale with:

- cell count
- tile count
- direction count
- contradiction frequency

Practical implications:

- 3D grids are much more expensive than 2D because both cell count and neighborhood pressure grow quickly.
- Large weighted tilesets make entropy and compatibility unions more expensive.
- Heavy contradiction rates raise backtracking costs more than raw grid size alone.

Suggested v1 expectations:

- `32x32` 2D with a modest tileset should be comfortably interactive.
- `64x64` 2D is reasonable for background generation.
- `16x16x16` 3D is a practical upper bound for real-time debug usage unless the ruleset is simple.
