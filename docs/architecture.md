# Architecture

`saddle-procgen-wfc` is split into two layers:

1. Pure solver core
2. Thin Bevy runtime integration

The solver owns domains, propagation, entropy scoring, and rollback. The Bevy layer owns requests, job entities, task polling, and result messages.

## Core Model

This v1 architecture intentionally follows a few concrete lessons from the WFC / model-synthesis references:

- `mxgmn` and Merrell both point toward the simple tiled model as the cleanest reusable base when the consumer needs explicit authored adjacency instead of opaque sample-derived patterns.
- BorisTheBrave’s CSP framing is why the solver treats WFC as ordinary constraint propagation plus search, not as a special procgen-only algorithm.
- DeBroglie and `ghx_proc_gen` show the value of dense tile indices, explicit compatibility tables, and reusable output-agnostic solver surfaces, which is why the ECS integration stays thin.

- The public ruleset uses explicit `WfcTileId` values.
- Compilation maps those ids to dense internal indices.
- When `WfcTileSymmetry` is enabled in 2D, compilation expands one logical tile family into two or four rotated internal variants.
- Each direction and tile gets a precomputed compatibility bitset.
- Each cell stores its current domain as a compact bitset plus a cached domain-size count.

That gives a direct representation of the simple tiled model:

```text
cell domain = { possible tile ids at this position }
adjacency[tile][direction] = bitset of compatible neighbor tiles
```

For auto-rotated families the internal model becomes:

```text
logical tile id + quarter-turn -> internal variant index
adjacency[variant][direction] = rotated compatibility bitset
```

The public output still reports the logical tile id, plus a separate chosen rotation per collapsed cell.

## Boundary Stitching

`WfcBoundaryStitching` is folded into `CompiledGrid`, not layered on top of propagation later.

- neighbor lookup stays the single source of truth for ordinary and wrapped grids
- border constraints still apply to the rectangular bounding box, even when neighbor lookup wraps
- overlap-model periodic output reuses the same wrapped-neighbor path through `WfcBoundaryStitching::xy()`

This keeps chunk-seam and toroidal behavior inside the grid topology layer instead of duplicating it in the solver.

## Hex Topology

`WfcTopology::Hex2d` keeps the rest of the solver unchanged by changing only:

- the active direction set
- neighbor lookup in `CompiledGrid`
- authoring expectations for adjacency rules

Hex grids currently use fixed tile families rather than auto-rotated symmetry expansion, which keeps the public ruleset explicit while avoiding a second rotation system on top of the cartesian quarter-turn path.

## Overlap-model Helper

`solve_overlap_wfc_2d(&WfcOverlapRequest)` is implemented as a thin builder over the same tiled-model core:

1. read overlapping windows from a sample `WfcTileGrid`
2. deduplicate those windows into transient pattern tiles
3. derive compatibility from overlap equality
4. solve through the existing tiled-model solver
5. map the chosen pattern anchors back into the returned `WfcSolution`

This means overlap output still benefits from the same seeded branching, backtracking, diagnostics, and runtime-free pure solve path as explicit authored rulesets.

## Propagation Strategy

The crate uses an AC-3-style propagation loop over precomputed compatibility masks.

When a cell domain changes:

1. Push the changed cell into a queue.
2. Revisit each neighbor.
3. Union the compatibility masks for every tile still allowed in the source cell.
4. Intersect the neighbor domain with that union.
5. If the neighbor domain shrinks, queue it too.

This keeps the implementation understandable while still using dense masks and cheap intersections.

Why this choice:

- It matches the core WFC framing from the original tiled model references and BorisTheBrave’s CSP explanations.
- It keeps rollback simple because the solver only needs to restore prior domains.
- It leaves room for a future AC-4-style support-count backend without changing the public API.

## Observation Heuristic

The solver supports:

- `MinimumEntropy`
- `MinimumRemainingValues`

`MinimumEntropy` computes Shannon entropy from the remaining weighted tiles in a cell. Ties are broken deterministically with the seeded RNG.

Weighted tile choice is sampled without replacement to build a deterministic per-decision choice order. That order becomes the backtracking branch stack for the chosen cell.

## Backtracking

Backtracking uses two explicit stacks:

- `trail`: every domain mutation stores the prior domain and prior cached count
- `decisions`: each observed cell stores the rollback point and remaining alternatives

On contradiction:

1. Roll back the trail to the last decision marker.
2. Try the next remaining tile for that decision.
3. If no alternatives remain, continue unwinding.

This is intentionally explicit and inspectable. The crate reports observation count, propagation count, contradiction count, backtrack count, and elapsed time for every solve.

## Constraints

Local constraints in v1:

- fixed cells
- per-cell bans
- border restrictions

Generic global constraint in v1:

- per-tile min/max count

Tile-count constraints are checked as feasibility bounds:

- contradiction if guaranteed placements already exceed `max`
- contradiction if remaining possible placements already fall below `min`

This keeps the constraint generic and useful without turning v1 into a full non-local-constraint framework.

## Failure Reporting

Bad tilesets are treated as normal input, not exceptional misuse.

Failures report:

- reason
- seed
- topology and grid size
- solve stats
- last contradiction, when known
- optional per-cell debug snapshot

The debug snapshot contains the remaining domain, entropy, and collapsed tile per cell. It is disabled by default because it can be large on big grids.
When rotated families are in play, the snapshot also carries the surviving rotated variants so authoring mistakes are easier to diagnose.

## Bevy Runtime Layer

The plugin is deliberately small:

1. `GenerateWfc` message arrives.
2. A named `WfcJob` entity is spawned.
3. A task is started on `AsyncComputeTaskPool`.
4. Polling checks `check_ready` without blocking the main thread.
5. `WfcSolved` or `WfcFailed` is emitted and the job entity keeps the final result for inspection.

`WfcRuntimeDiagnostics` tracks aggregate job counts and the last observed outcome. This is useful for BRP and crate-local E2E verification.

## Step-by-Step Solver

`WfcStepSolver` wraps the internal `Solver` and exposes a single-observation-per-call interface. Each `step()` call performs one observe-then-propagate cycle and returns a `WfcStepSnapshot` containing the full grid state, the last observed position, and whether the solve is finished or failed.

This is intentionally a thin wrapper — it reuses the same propagation, backtracking, and entropy heuristics as the batch solver. The snapshot copies cell state from the internal solver, so there is no coupling between visualization code and solver internals.

The step solver borrows `&WfcRequest` for its lifetime, which means callers that need to persist it across frames (like Bevy systems) must manage the borrow appropriately. The `step_visualizer` example demonstrates a heap-pinned self-referential pattern for this.

## Planned Extension Points

- rotated or mirrored overlap-pattern augmentation
- partial sub-block repair APIs
- alternate propagation backends such as support-count AC-4
- multi-tile object placement on top of the current request/result shell

The current API is kept intentionally narrow so those extensions can add new builders or helper layers without forcing a rewrite of `WfcRequest`, `WfcRuleset`, `WfcSolution`, or the async Bevy job shell.
