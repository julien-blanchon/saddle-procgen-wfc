# `saddle-procgen-wfc-lab`

Crate-local lab for `saddle-procgen-wfc`.

## Purpose

- Exercise the async Bevy job flow
- Provide BRP-visible job entities and runtime diagnostics
- Give E2E scenarios a stable visual surface for normal solves, constrained solves, and contradiction debugging

## Status

Working

## How To Run

```bash
cargo run -p saddle-procgen-wfc-lab
```

With E2E:

```bash
cargo run -p saddle-procgen-wfc-lab --features e2e -- wfc_smoke
cargo run -p saddle-procgen-wfc-lab --features e2e -- wfc_async_large
```

With BRP handoff:

```bash
cargo run -p saddle-procgen-wfc-lab --features e2e -- wfc_smoke --handoff
```

## Controls

- `1`: basic tiled solve
- `2`: constrained room solve
- `3`: contradiction / debug-entropy view
- `4`: larger async solve surface
- `Space`: regenerate current view with the next seed

## BRP Workflow

```bash
cargo run -p saddle-procgen-wfc-lab --features e2e -- wfc_async_large --handoff
uv run --project .codex/skills/bevy-brp/script brp ping
uv run --project .codex/skills/bevy-brp/script brp world query bevy_ecs::name::Name
uv run --project .codex/skills/bevy-brp/script brp extras screenshot /tmp/wfc_lab.png
uv run --project .codex/skills/bevy-brp/script brp extras shutdown
```

## Findings

- The lab keeps finished `WfcJob` entities in the world so BRP can inspect requests and results after completion.
- The contradiction view intentionally submits an unsatisfiable request with debug snapshots enabled, making failure diagnostics visible without leaving the app.
- The large async view gives E2E and BRP a stable surface to verify that request submission, background solving, and final application are all visible as distinct states.
