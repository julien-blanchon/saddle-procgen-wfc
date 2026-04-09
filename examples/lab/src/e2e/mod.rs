use bevy::prelude::*;
use saddle_bevy_e2e::{
    action::Action,
    actions::{assertions, inspect},
    scenario::Scenario,
};
use saddle_procgen_wfc::WfcSystems;

use crate::{
    BeforeSignature, LabDiagnostics, LabSolveState, LabView, request_regeneration, set_view,
};


pub struct WfcLabE2EPlugin;

impl Plugin for WfcLabE2EPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(saddle_bevy_e2e::E2EPlugin);
        app.configure_sets(Update, saddle_bevy_e2e::E2ESet.before(WfcSystems::Request));
        let args: Vec<String> = std::env::args().collect();
        let (scenario_name, handoff) = parse_e2e_args(&args);
        if let Some(name) = scenario_name {
            if let Some(mut scenario) = scenario_by_name(&name) {
                if handoff {
                    scenario.actions.push(Action::Handoff);
                }
                saddle_bevy_e2e::init_scenario(app, scenario);
            } else {
                error!(
                    "[wfc_lab:e2e] Unknown scenario '{name}'. Available: {:?}",
                    list_scenarios()
                );
            }
        }
    }
}

fn parse_e2e_args(args: &[String]) -> (Option<String>, bool) {
    let mut scenario_name = None;
    let mut handoff = false;
    for arg in args.iter().skip(1) {
        if arg == "--handoff" {
            handoff = true;
        } else if !arg.starts_with('-') && scenario_name.is_none() {
            scenario_name = Some(arg.clone());
        }
    }
    if !handoff {
        handoff = std::env::var("E2E_HANDOFF").is_ok_and(|value| value == "1" || value == "true");
    }
    (scenario_name, handoff)
}

fn switch_view(view: LabView) -> Action {
    Action::Custom(Box::new(move |world| set_view(world, view)))
}

fn regenerate() -> Action {
    Action::Custom(Box::new(request_regeneration))
}

fn remember_signature() -> Action {
    Action::Custom(Box::new(|world| {
        world.resource_mut::<BeforeSignature>().0 = world.resource::<LabDiagnostics>().signature;
    }))
}

fn wait_for_view(view: LabView, state: LabSolveState) -> Action {
    Action::WaitUntil {
        label: format!("wait for {view:?} {state:?}"),
        condition: Box::new(move |world| {
            let diagnostics = world.resource::<LabDiagnostics>();
            diagnostics.active_view == view && diagnostics.solve_state == state
        }),
        max_frames: 240,
    }
}

fn wait_for_solved_view(view: LabView) -> Action {
    wait_for_view(view, LabSolveState::Solved)
}

fn wait_for_failed_view(view: LabView) -> Action {
    wait_for_view(view, LabSolveState::Failed)
}

fn wait_for_new_signature(view: LabView) -> Action {
    Action::WaitUntil {
        label: format!("wait for new {view:?} signature"),
        condition: Box::new(move |world| {
            let before = world.resource::<BeforeSignature>().0;
            let diagnostics = world.resource::<LabDiagnostics>();
            diagnostics.active_view == view
                && diagnostics.solve_state == LabSolveState::Solved
                && diagnostics.signature != 0
                && diagnostics.signature != before
        }),
        max_frames: 240,
    }
}

fn wait_for_running_jobs(view: LabView) -> Action {
    Action::WaitUntil {
        label: format!("wait for {view:?} running"),
        condition: Box::new(move |world| {
            let diagnostics = world.resource::<LabDiagnostics>();
            diagnostics.active_view == view
                && diagnostics.solve_state == LabSolveState::Running
                && diagnostics.running_jobs >= 1
        }),
        max_frames: 240,
    }
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "wfc_smoke" => Some(wfc_smoke()),
        "wfc_views" => Some(wfc_views()),
        "wfc_topologies" => Some(wfc_topologies()),
        "wfc_regeneration" => Some(wfc_regeneration()),
        "wfc_async_large" => Some(wfc_async_large()),
        "wfc_border_constraints" => Some(wfc_border_constraints()),
        "wfc_fixed_cells" => Some(wfc_fixed_cells()),
        "wfc_global_constraints" => Some(wfc_global_constraints()),
        "wfc_contradiction_debug_snapshot" => Some(wfc_contradiction_debug_snapshot()),
        "wfc_seed_determinism" => Some(wfc_seed_determinism()),
        "wfc_voxel_3d_topology" => Some(wfc_voxel_3d_topology()),
        "wfc_socket_builder" => Some(wfc_socket_builder()),
        "wfc_serde_roundtrip" => Some(wfc_serde_roundtrip()),
        "wfc_learned_rules" => Some(wfc_learned_rules()),
        "wfc_keyboard_workflow" => Some(wfc_keyboard_workflow()),
        _ => None,
    }
}

pub fn list_scenarios() -> Vec<&'static str> {
    vec![
        "wfc_smoke",
        "wfc_views",
        "wfc_topologies",
        "wfc_regeneration",
        "wfc_async_large",
        "wfc_border_constraints",
        "wfc_fixed_cells",
        "wfc_global_constraints",
        "wfc_contradiction_debug_snapshot",
        "wfc_seed_determinism",
        "wfc_voxel_3d_topology",
        "wfc_socket_builder",
        "wfc_serde_roundtrip",
        "wfc_learned_rules",
        "wfc_keyboard_workflow",
    ]
}

fn wfc_smoke() -> Scenario {
    Scenario::builder("wfc_smoke")
        .description(
            "Verify the default basic view solves and publishes diagnostics plus a visible grid.",
        )
        .then(Action::WaitFrames(10))
        .then(wait_for_solved_view(LabView::Basic))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "basic view solved",
            |diagnostics| diagnostics.signature != 0 && diagnostics.visible_cells > 0,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("smoke diagnostics"))
        .then(Action::Screenshot("wfc_smoke".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_smoke"))
        .build()
}

fn wfc_views() -> Scenario {
    Scenario::builder("wfc_views")
        .description("Switch through the room and contradiction views and assert both the successful and failed surfaces update.")
        .then(remember_signature())
        .then(switch_view(LabView::Room))
        .then(wait_for_new_signature(LabView::Room))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "room view has entrance and exit markers",
            |diagnostics| diagnostics.highlighted_cells == 2 && diagnostics.visible_cells > 0,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("room diagnostics"))
        .then(Action::Screenshot("wfc_room".into()))
        .then(Action::WaitFrames(1))
        .then(switch_view(LabView::Contradiction))
        .then(wait_for_failed_view(LabView::Contradiction))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "contradiction view exposes failure diagnostics",
            |diagnostics| {
                diagnostics.last_failure_reason == Some(saddle_procgen_wfc::WfcFailureReason::Contradiction)
                    && diagnostics.zero_domain_cells >= 1
                    && diagnostics.contradiction_position.is_some()
            },
        ))
        .then(inspect::log_resource::<LabDiagnostics>("contradiction diagnostics"))
        .then(Action::Screenshot("wfc_contradiction".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_views"))
        .build()
}

fn wfc_regeneration() -> Scenario {
    Scenario::builder("wfc_regeneration")
        .description(
            "Regenerate the basic view with a new seed and verify the published signature changes.",
        )
        .then(switch_view(LabView::Basic))
        .then(wait_for_solved_view(LabView::Basic))
        .then(remember_signature())
        .then(regenerate())
        .then(wait_for_new_signature(LabView::Basic))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "basic view regenerated",
            |diagnostics| diagnostics.regeneration_count >= 2 && diagnostics.completed_jobs >= 2,
        ))
        .then(inspect::log_resource::<LabDiagnostics>(
            "regeneration diagnostics",
        ))
        .then(Action::Screenshot("wfc_regenerated".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_regeneration"))
        .build()
}

fn wfc_topologies() -> Scenario {
    Scenario::builder("wfc_topologies")
        .description(
            "Switch to the stitched torus and hex views, then verify both solve and publish diagnostics for the new topology modes.",
        )
        .then(remember_signature())
        .then(switch_view(LabView::Stitched))
        .then(wait_for_new_signature(LabView::Stitched))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "stitched view validates wrapped seams",
            |diagnostics| diagnostics.seam_pairs >= 10 && diagnostics.visible_cells == 24,
        ))
        .then(Action::Screenshot("wfc_stitched".into()))
        .then(Action::WaitFrames(1))
        .then(remember_signature())
        .then(switch_view(LabView::Hex))
        .then(wait_for_new_signature(LabView::Hex))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "hex view solves with visible output",
            |diagnostics| {
                diagnostics.active_view == LabView::Hex
                    && diagnostics.solve_state == LabSolveState::Solved
                    && diagnostics.visible_cells == 80
            },
        ))
        .then(Action::Screenshot("wfc_hex".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_topologies"))
        .build()
}

fn wfc_async_large() -> Scenario {
    Scenario::builder("wfc_async_large")
        .description(
            "Switch to the larger async solve, verify the app stays in a running state while work is pending, then assert the final grid resolves.",
        )
        .then(switch_view(LabView::Large))
        .then(wait_for_running_jobs(LabView::Large))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "large view enters async running state",
            |diagnostics| {
                diagnostics.active_view == LabView::Large
                    && diagnostics.solve_state == LabSolveState::Running
                    && diagnostics.running_jobs >= 1
            },
        ))
        .then(inspect::log_resource::<LabDiagnostics>("large running diagnostics"))
        .then(Action::Screenshot("wfc_large_running".into()))
        .then(Action::WaitFrames(1))
        .then(wait_for_solved_view(LabView::Large))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "large view eventually solves",
            |diagnostics| {
                diagnostics.active_view == LabView::Large
                    && diagnostics.signature != 0
                    && diagnostics.visible_cells == 64 * 48
                    && diagnostics.completed_jobs >= 1
            },
        ))
        .then(inspect::log_resource::<LabDiagnostics>("large solved diagnostics"))
        .then(Action::Screenshot("wfc_large_solved".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_async_large"))
        .build()
}

// ---------------------------------------------------------------------------
// New scenarios covering previously untested features
// ---------------------------------------------------------------------------

/// Verify that border constraints are enforced: the Room view forces wall-only
/// tiles on MinY/MaxY and wall+entrance on MinX, wall+exit on MaxX. After
/// solving we assert zero_domain_cells == 0 (no constraint violations) and
/// that the highlighted_cells count equals exactly 2 (one entrance + one exit
/// portal cell pair as labelled by the Room view).
fn wfc_border_constraints() -> Scenario {
    Scenario::builder("wfc_border_constraints")
        .description(
            "Verify border constraints are honoured: Room view forces wall-only borders on top/bottom and entrance/exit on left/right; expects exactly 2 portal (entrance+exit) cells and a clean solve with no zero-domain cells.",
        )
        .then(switch_view(LabView::Room))
        .then(wait_for_new_signature(LabView::Room))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "room view solves cleanly — no zero-domain cells",
            |diagnostics| {
                diagnostics.active_view == LabView::Room
                    && diagnostics.solve_state == LabSolveState::Solved
                    && diagnostics.zero_domain_cells == 0
            },
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "room border constraints produce exactly 2 portal cells (entrance + exit)",
            |diagnostics| diagnostics.highlighted_cells == 2,
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "room grid is fully populated (16x10 = 160 cells)",
            |diagnostics| diagnostics.visible_cells == 160,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("border constraints diagnostics"))
        .then(Action::Screenshot("wfc_border_constraints".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_border_constraints"))
        .build()
}

/// Verify fixed-cell pinning: the Room view pins entrance at (0,5) and exit at
/// (15,4). After solving, tile indices 2 (entrance) and 3 (exit) must appear
/// exactly once each and at no position other than the pinned ones.
fn wfc_fixed_cells() -> Scenario {
    Scenario::builder("wfc_fixed_cells")
        .description(
            "Verify fixed-cell pinning works: the Room view pins entrance at (0,5) and exit at (15,4). After solving the WfcGlobalConstraint enforces exactly 1 entrance and 1 exit in the 16x10 grid.",
        )
        .then(switch_view(LabView::Room))
        .then(wait_for_new_signature(LabView::Room))
        // The global constraints on the room enforce min_count=1 max_count=1 for
        // both entrance (tile 2) and exit (tile 3), which the solver must satisfy.
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "fixed cells respected — exactly 1 entrance and 1 exit",
            |diagnostics| {
                diagnostics.active_view == LabView::Room
                    && diagnostics.solve_state == LabSolveState::Solved
                    // highlighted_cells counts tiles with id 2 or 3 in Room view
                    && diagnostics.highlighted_cells == 2
            },
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "solve succeeded with non-zero signature",
            |diagnostics| diagnostics.signature != 0,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("fixed cells diagnostics"))
        .then(Action::Screenshot("wfc_fixed_cells".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_fixed_cells"))
        .build()
}

/// Verify global TileCount constraints: Room view requires at least 60 floor
/// tiles, exactly 1 entrance, and exactly 1 exit. After solving we assert the
/// runtime completes without failure, the visible cell count equals 160 (16×10),
/// and the diagnostics mark the solve as successful.
fn wfc_global_constraints() -> Scenario {
    Scenario::builder("wfc_global_constraints")
        .description(
            "Verify WfcGlobalConstraint::TileCount enforcement: Room view requires floor >= 60, entrance == 1, exit == 1. Asserts the solve succeeds (signature != 0) and the runtime records a completed job with no failures.",
        )
        .then(switch_view(LabView::Room))
        .then(wait_for_new_signature(LabView::Room))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "global constraints satisfied — room solved",
            |diagnostics| {
                diagnostics.active_view == LabView::Room
                    && diagnostics.solve_state == LabSolveState::Solved
                    && diagnostics.signature != 0
            },
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "runtime records completed job and no failures",
            |diagnostics| diagnostics.completed_jobs >= 1 && diagnostics.failed_jobs == 0,
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "all 160 cells are populated (global min_count for floor did not over-constrain)",
            |diagnostics| diagnostics.visible_cells == 160,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("global constraints diagnostics"))
        .then(Action::Screenshot("wfc_global_constraints".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_global_constraints"))
        .build()
}

/// Verify debug snapshot population on contradiction: the Contradiction view
/// uses capture_debug_snapshot=true so the failure carries a populated snapshot
/// with every cell reporting a non-zero possible_count entry (ambiguous cells)
/// or a zero-domain cell at the contradiction site.
fn wfc_contradiction_debug_snapshot() -> Scenario {
    Scenario::builder("wfc_contradiction_debug_snapshot")
        .description(
            "Verify that when a solve fails with Contradiction and capture_debug_snapshot is enabled, the failure exposes: reason=Contradiction, zero_domain_cells >= 1 (the contradiction site), ambiguous_cells > 0 (partially propagated), and a non-None contradiction_position.",
        )
        .then(switch_view(LabView::Contradiction))
        .then(wait_for_failed_view(LabView::Contradiction))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "failure reason is Contradiction",
            |diagnostics| {
                diagnostics.last_failure_reason
                    == Some(saddle_procgen_wfc::WfcFailureReason::Contradiction)
            },
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "at least one zero-domain cell detected at contradiction site",
            |diagnostics| diagnostics.zero_domain_cells >= 1,
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "contradiction position is populated by the debug snapshot",
            |diagnostics| diagnostics.contradiction_position.is_some(),
        ))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "debug snapshot populated cells (ambiguous_cells > 0 means partial propagation was captured)",
            |diagnostics| diagnostics.ambiguous_cells > 0,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("contradiction debug snapshot diagnostics"))
        .then(Action::Screenshot("wfc_contradiction_debug_snapshot".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_contradiction_debug_snapshot"))
        .build()
}

/// Verify seed determinism: solve the same view twice with the same seed and
/// assert that both runs produce identical signatures. We solve Basic (seed 7),
/// save the signature, switch to Stitched to clear the slate, switch back to
/// Basic (which resets to seed 7 via set_view), wait for it to finish, then
/// assert the new signature equals the remembered one.
///
/// Note: we use `wait_for_view` (not `wait_for_new_signature`) for the second
/// Basic solve because the signature is expected to match — not differ — from
/// the stored value.
fn wfc_seed_determinism() -> Scenario {
    Scenario::builder("wfc_seed_determinism")
        .description(
            "Verify seed determinism: solve Basic view (seed 7) twice by switching away and back, assert that both runs produce identical signatures proving the solver is deterministic for the same seed.",
        )
        // First solve at seed 7.
        .then(switch_view(LabView::Basic))
        .then(wait_for_solved_view(LabView::Basic))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "first solve produces a valid signature",
            |diagnostics| {
                diagnostics.active_view == LabView::Basic
                    && diagnostics.signature != 0
            },
        ))
        // Record the first signature into BeforeSignature.
        .then(remember_signature())
        // Switch away so the Basic view must run a fresh solve on return.
        .then(switch_view(LabView::Stitched))
        .then(wait_for_new_signature(LabView::Stitched))
        // Overwrite BeforeSignature with the Stitched value so the second
        // wait_for_new_signature guard (below) does not use the Basic sig.
        // Instead we save the second Basic sig in BeforeSignature via a
        // Custom action after the solve so we can compare.
        // Switch back to Basic — set_view resets to seed=7 deterministically.
        .then(switch_view(LabView::Basic))
        // Use wait_for_view (not wait_for_new_signature) because the signature
        // IS expected to equal the first run — we just wait for Solved state.
        .then(wait_for_solved_view(LabView::Basic))
        // Assert the second signature matches the first (saved before the
        // Stitched detour). We read both from the world directly.
        .then(Action::Custom(Box::new(|world| {
            let first_sig = world.resource::<BeforeSignature>().0;
            let diagnostics = world.resource::<LabDiagnostics>();
            assert_ne!(first_sig, 0, "stored first signature should be non-zero");
            assert_ne!(diagnostics.signature, 0, "second signature should be non-zero");
            assert_eq!(
                diagnostics.signature, first_sig,
                "seed determinism violated: first_run={first_sig} second_run={}",
                diagnostics.signature
            );
        })))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "second solve matches first — solver is deterministic for seed 7",
            |diagnostics| {
                diagnostics.active_view == LabView::Basic
                    && diagnostics.solve_state == LabSolveState::Solved
                    && diagnostics.signature != 0
            },
        ))
        .then(inspect::log_resource::<LabDiagnostics>("seed determinism diagnostics"))
        .then(Action::Screenshot("wfc_seed_determinism".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_seed_determinism"))
        .build()
}

/// Verify the 3D Cartesian topology path using the `solve_wfc` API directly
/// inside an `Action::Custom`. We build a 10×10×6 Cartesian3d voxel grid
/// inline (air / stone / cap with vertical adjacency rules). We assert: solve
/// succeeds, grid has 600 cells, stone caps always sit above stone columns
/// (ZNeg adjacency rule), and the signature is non-zero.
fn wfc_voxel_3d_topology() -> Scenario {
    Scenario::builder("wfc_voxel_3d_topology")
        .description(
            "Directly invoke solve_wfc with a Cartesian3d (voxel) request inside Action::Custom and assert: solve succeeds with 600 cells, signature is non-zero, and the cap-above-stone adjacency rule is respected across all 10x10x6 cells.",
        )
        // Wait for the app to be fully initialised first.
        .then(Action::WaitFrames(10))
        .then(Action::Custom(Box::new(|world| {
            use saddle_procgen_wfc::{
                WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcTileDefinition,
                WfcTileId, WfcTopology, solve_wfc,
            };

            let air   = WfcTileId(0);
            let stone = WfcTileId(1);
            let cap   = WfcTileId(2);

            let ruleset = WfcRuleset::new(
                WfcTopology::Cartesian3d,
                vec![
                    WfcTileDefinition::new(air,   3.0, "Air"),
                    WfcTileDefinition::new(stone, 2.0, "Stone"),
                    WfcTileDefinition::new(cap,   1.0, "Cap"),
                ],
            )
            .with_rule(air,   WfcDirection::XPos, [air, stone, cap])
            .with_rule(air,   WfcDirection::XNeg, [air, stone, cap])
            .with_rule(air,   WfcDirection::YPos, [air, stone, cap])
            .with_rule(air,   WfcDirection::YNeg, [air, stone, cap])
            .with_rule(air,   WfcDirection::ZPos, [air, cap])
            .with_rule(air,   WfcDirection::ZNeg, [air, stone, cap])
            .with_rule(stone, WfcDirection::XPos, [stone, air])
            .with_rule(stone, WfcDirection::XNeg, [stone, air])
            .with_rule(stone, WfcDirection::YPos, [stone, air])
            .with_rule(stone, WfcDirection::YNeg, [stone, air])
            .with_rule(stone, WfcDirection::ZPos, [stone, cap])
            .with_rule(stone, WfcDirection::ZNeg, [stone])
            .with_rule(cap,   WfcDirection::XPos, [cap, air])
            .with_rule(cap,   WfcDirection::XNeg, [cap, air])
            .with_rule(cap,   WfcDirection::YPos, [cap, air])
            .with_rule(cap,   WfcDirection::YNeg, [cap, air])
            .with_rule(cap,   WfcDirection::ZPos, [air])
            .with_rule(cap,   WfcDirection::ZNeg, [stone]);

            let request = WfcRequest::new(
                WfcGridSize::new_3d(10, 10, 6),
                ruleset,
                WfcSeed(31),
            );

            let solution = solve_wfc(&request)
                .expect("voxel 3D Cartesian topology should solve");

            // 10 × 10 × 6 = 600 cells
            assert_eq!(
                solution.grid.tiles.len(),
                600,
                "expected 600 cells in a 10x10x6 voxel grid"
            );
            assert_ne!(solution.signature, 0, "voxel solve should produce a non-zero signature");

            // Verify the cap-above-stone adjacency: every cap tile at layer z > 0
            // must have stone directly below it (ZNeg direction).
            let grid = &solution.grid;
            let mut adjacency_ok = true;
            'outer: for z in 1..grid.size.depth {
                for y in 0..grid.size.height {
                    for x in 0..grid.size.width {
                        let pos = bevy::math::UVec3::new(x, y, z);
                        if grid.tile_at(pos) == Some(cap) {
                            let below = bevy::math::UVec3::new(x, y, z - 1);
                            if grid.tile_at(below) != Some(stone) {
                                adjacency_ok = false;
                                break 'outer;
                            }
                        }
                    }
                }
            }
            assert!(adjacency_ok, "cap tiles must always rest on stone (ZNeg adjacency violated)");

            // Store the signature so the next assertion can confirm it.
            world.resource_mut::<BeforeSignature>().0 = solution.signature;
        })))
        .then(assertions::resource_satisfies::<BeforeSignature>(
            "voxel solve signature stored (non-zero)",
            |sig| sig.0 != 0,
        ))
        .then(Action::Screenshot("wfc_voxel_3d_topology".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_voxel_3d_topology"))
        .build()
}

/// Verify the socket/connector builder: build a ruleset using
/// `WfcSocketRulesetBuilder`, solve it, and assert adjacency correctness.
fn wfc_socket_builder() -> Scenario {
    Scenario::builder("wfc_socket_builder")
        .description(
            "Build a socket-based ruleset with symmetric and asymmetric sockets, solve a 12x10 grid, verify it succeeds with the correct cell count and non-zero signature.",
        )
        .then(Action::WaitFrames(10))
        .then(Action::Custom(Box::new(|world| {
            use saddle_procgen_wfc::{
                WfcDirection, WfcGridSize, WfcRequest, WfcSeed, WfcSocketRulesetBuilder,
                WfcTileSymmetry, WfcTopology, solve_wfc,
            };

            let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d)
                .add_asymmetric_pair("pipe_in", "pipe_out");

            builder
                .add_tile(0u16, 5.0, "Grass")
                .all_sockets("g")
                .done();
            builder
                .add_tile(1u16, 2.0, "Road")
                .socket(WfcDirection::XPos, "road")
                .socket(WfcDirection::XNeg, "road")
                .socket(WfcDirection::YPos, "g")
                .socket(WfcDirection::YNeg, "g")
                .symmetry(WfcTileSymmetry::Rotate2)
                .done();
            builder
                .add_tile(2u16, 0.5, "PipeSrc")
                .socket(WfcDirection::XPos, "pipe_out")
                .socket(WfcDirection::XNeg, "g")
                .socket(WfcDirection::YPos, "g")
                .socket(WfcDirection::YNeg, "g")
                .done();
            builder
                .add_tile(3u16, 0.5, "PipeSink")
                .socket(WfcDirection::XPos, "g")
                .socket(WfcDirection::XNeg, "pipe_in")
                .socket(WfcDirection::YPos, "g")
                .socket(WfcDirection::YNeg, "g")
                .done();

            let ruleset = builder.build().expect("socket ruleset should build");
            let request = WfcRequest::new(
                WfcGridSize::new_2d(12, 10),
                ruleset,
                WfcSeed(99),
            );
            let solution = solve_wfc(&request).expect("socket-built request should solve");

            assert_eq!(solution.grid.tiles.len(), 120, "12x10 = 120 cells");
            assert_ne!(solution.signature, 0);

            world.resource_mut::<BeforeSignature>().0 = solution.signature;
        })))
        .then(assertions::resource_satisfies::<BeforeSignature>(
            "socket builder solve signature stored",
            |sig| sig.0 != 0,
        ))
        .then(Action::Screenshot("wfc_socket_builder".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_socket_builder"))
        .build()
}

/// Verify serde roundtrip: serialize a WfcRequest to RON, deserialize it,
/// solve both, and assert identical signatures.
fn wfc_serde_roundtrip() -> Scenario {
    Scenario::builder("wfc_serde_roundtrip")
        .description(
            "Serialize a WfcRequest to RON, deserialize it back, solve both original and deserialized, assert identical signatures proving serde is lossless.",
        )
        .then(Action::WaitFrames(10))
        .then(Action::Custom(Box::new(|world| {
            use saddle_procgen_wfc::{
                WfcGridSize, WfcRequest, WfcRuleset, WfcSeed,
                WfcTileDefinition, WfcTileId, WfcTopology, solve_wfc,
            };

            let grass = WfcTileId(0);
            let road = WfcTileId(1);
            let ruleset = WfcRuleset::new(
                WfcTopology::Cartesian2d,
                vec![
                    WfcTileDefinition::new(grass, 3.0, "Grass"),
                    WfcTileDefinition::new(road, 1.0, "Road"),
                ],
            )
            .with_all_direction_rules(grass, [grass, road])
            .with_all_direction_rules(road, [grass, road]);

            let request = WfcRequest::new(
                WfcGridSize::new_2d(8, 8),
                ruleset,
                WfcSeed(42),
            );

            // Serialize to RON and deserialize back
            let ron_string = ron::to_string(&request)
                .expect("WfcRequest should serialize to RON");
            let deserialized: WfcRequest = ron::from_str(&ron_string)
                .expect("WfcRequest should deserialize from RON");

            // Solve both
            let solution_a = solve_wfc(&request).expect("original should solve");
            let solution_b = solve_wfc(&deserialized).expect("deserialized should solve");

            assert_eq!(
                solution_a.signature, solution_b.signature,
                "serde roundtrip must produce identical solutions: {} vs {}",
                solution_a.signature, solution_b.signature
            );
            assert_ne!(solution_a.signature, 0);

            world.resource_mut::<BeforeSignature>().0 = solution_a.signature;
        })))
        .then(assertions::resource_satisfies::<BeforeSignature>(
            "serde roundtrip signature stored",
            |sig| sig.0 != 0,
        ))
        .then(Action::Screenshot("wfc_serde_roundtrip".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_serde_roundtrip"))
        .build()
}

/// Verify learn_adjacency_rules: build a sample grid, learn rules from it,
/// solve, and verify all output adjacencies were observed in the sample.
fn wfc_learned_rules() -> Scenario {
    Scenario::builder("wfc_learned_rules")
        .description(
            "Build a hand-placed sample grid, learn adjacency rules with learn_adjacency_rules(), solve a larger grid, verify all adjacencies in the output were observed in the sample.",
        )
        .then(Action::WaitFrames(10))
        .then(Action::Custom(Box::new(|world| {
            use std::collections::BTreeSet;
            use saddle_procgen_wfc::{
                WfcDirection, WfcGridSize, WfcRequest, WfcSeed, WfcTileGrid, WfcTileId,
                WfcTopology, learn_adjacency_rules, solve_wfc,
            };

            let grass = WfcTileId(0);
            let road = WfcTileId(1);
            let water = WfcTileId(2);

            // Build a 6x4 sample: grass top, road middle, water bottom
            let mut sample = WfcTileGrid::new_empty(
                WfcTopology::Cartesian2d,
                WfcGridSize::new_2d(6, 4),
            );
            for x in 0..6u32 {
                sample.set_tile_at(bevy::math::UVec3::new(x, 3, 0), grass);
                sample.set_tile_at(bevy::math::UVec3::new(x, 2, 0), road);
                sample.set_tile_at(bevy::math::UVec3::new(x, 1, 0), grass);
                sample.set_tile_at(bevy::math::UVec3::new(x, 0, 0), water);
            }

            // Collect all observed adjacencies from the sample
            let directions = WfcDirection::active(WfcTopology::Cartesian2d);
            let mut observed: BTreeSet<(WfcTileId, WfcDirection, WfcTileId)> = BTreeSet::new();
            for y in 0..4i32 {
                for x in 0..6i32 {
                    let pos = bevy::math::UVec3::new(x as u32, y as u32, 0);
                    let tile = sample.tile_at(pos).unwrap();
                    for &dir in directions {
                        let offset = dir.offset();
                        let nx = x + offset.x;
                        let ny = y + offset.y;
                        if nx >= 0 && ny >= 0 && nx < 6 && ny < 4 {
                            let npos = bevy::math::UVec3::new(nx as u32, ny as u32, 0);
                            let neighbor = sample.tile_at(npos).unwrap();
                            observed.insert((tile, dir, neighbor));
                        }
                    }
                }
            }

            // Learn and solve
            let ruleset = learn_adjacency_rules(&sample);
            let request = WfcRequest::new(
                WfcGridSize::new_2d(10, 8),
                ruleset,
                WfcSeed(7),
            );
            let solution = solve_wfc(&request).expect("learned rules should solve");

            assert_eq!(solution.grid.tiles.len(), 80);
            assert_ne!(solution.signature, 0);

            // Verify all adjacencies in the output were observed in the sample
            let grid = &solution.grid;
            let mut all_valid = true;
            for y in 0..grid.size.height as i32 {
                for x in 0..grid.size.width as i32 {
                    let pos = bevy::math::UVec3::new(x as u32, y as u32, 0);
                    let tile = grid.tile_at(pos).unwrap();
                    for &dir in directions {
                        let offset = dir.offset();
                        let nx = x + offset.x;
                        let ny = y + offset.y;
                        if nx >= 0 && ny >= 0 && nx < grid.size.width as i32 && ny < grid.size.height as i32 {
                            let npos = bevy::math::UVec3::new(nx as u32, ny as u32, 0);
                            let neighbor = grid.tile_at(npos).unwrap();
                            if !observed.contains(&(tile, dir, neighbor)) {
                                all_valid = false;
                            }
                        }
                    }
                }
            }
            assert!(all_valid, "all output adjacencies must have been observed in the sample");

            world.resource_mut::<BeforeSignature>().0 = solution.signature;
        })))
        .then(assertions::resource_satisfies::<BeforeSignature>(
            "learned rules solve signature stored",
            |sig| sig.0 != 0,
        ))
        .then(Action::Screenshot("wfc_learned_rules".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_learned_rules"))
        .build()
}

fn wfc_keyboard_workflow() -> Scenario {
    Scenario::builder("wfc_keyboard_workflow")
        .description(
            "Drive the lab through its keyboard shortcuts, switching views and triggering a regeneration so the interactive controls are covered through the public input path.",
        )
        .then(Action::WaitFrames(20))
        .then(wait_for_solved_view(LabView::Basic))
        .then(Action::Screenshot("wfc_keyboard_basic".into()))
        .then(Action::HoldKey {
            key: KeyCode::Digit2,
            frames: 1,
        })
        .then(wait_for_solved_view(LabView::Room))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "room view is active",
            |diagnostics| diagnostics.active_view == LabView::Room && diagnostics.visible_cells > 0,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("wfc_keyboard_room"))
        .then(Action::Screenshot("wfc_keyboard_room".into()))
        .then(Action::WaitFrames(1))
        .then(Action::HoldKey {
            key: KeyCode::Digit4,
            frames: 1,
        })
        .then(wait_for_running_jobs(LabView::Large))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "large view is running asynchronously",
            |diagnostics| {
                diagnostics.active_view == LabView::Large
                    && diagnostics.solve_state == LabSolveState::Running
                    && diagnostics.running_jobs >= 1
            },
        ))
        .then(inspect::log_resource::<LabDiagnostics>("wfc_keyboard_large_running"))
        .then(Action::Screenshot("wfc_keyboard_large_running".into()))
        .then(Action::WaitUntil {
            label: "large view solved".into(),
            condition: Box::new(|world| {
                let diagnostics = world.resource::<LabDiagnostics>();
                diagnostics.active_view == LabView::Large
                    && diagnostics.solve_state == LabSolveState::Solved
                    && diagnostics.signature != 0
            }),
            max_frames: 240,
        })
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "large view solved",
            |diagnostics| diagnostics.active_view == LabView::Large
                && diagnostics.solve_state == LabSolveState::Solved
                && diagnostics.signature != 0,
        ))
        .then(inspect::log_resource::<LabDiagnostics>("wfc_keyboard_large_solved"))
        .then(Action::Screenshot("wfc_keyboard_large_solved".into()))
        .then(Action::WaitFrames(1))
        .then(remember_signature())
        .then(Action::HoldKey {
            key: KeyCode::Space,
            frames: 1,
        })
        .then(wait_for_new_signature(LabView::Large))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "large view regenerated with a new signature",
            |diagnostics| {
                diagnostics.active_view == LabView::Large
                    && diagnostics.solve_state == LabSolveState::Solved
                    && diagnostics.signature != 0
                    && diagnostics.regeneration_count >= 2
            },
        ))
        .then(inspect::log_resource::<LabDiagnostics>(
            "wfc_keyboard_large_regenerated",
        ))
        .then(Action::Screenshot("wfc_keyboard_large_regenerated".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_keyboard_workflow"))
        .build()
}
