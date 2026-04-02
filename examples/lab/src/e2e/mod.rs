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
        "wfc_regeneration" => Some(wfc_regeneration()),
        "wfc_async_large" => Some(wfc_async_large()),
        _ => None,
    }
}

pub fn list_scenarios() -> Vec<&'static str> {
    vec![
        "wfc_smoke",
        "wfc_views",
        "wfc_regeneration",
        "wfc_async_large",
    ]
}

fn wfc_smoke() -> Scenario {
    Scenario::builder("wfc_smoke")
        .description(
            "Verify the default basic view solves and publishes diagnostics plus a visible grid.",
        )
        .then(Action::WaitFrames(10))
        .then(wait_for_view(LabView::Basic, LabSolveState::Solved))
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
        .then(wait_for_view(LabView::Contradiction, LabSolveState::Failed))
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
        .then(wait_for_view(LabView::Basic, LabSolveState::Solved))
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
        .then(wait_for_view(LabView::Large, LabSolveState::Solved))
        .then(assertions::resource_satisfies::<LabDiagnostics>(
            "large view eventually solves",
            |diagnostics| {
                diagnostics.active_view == LabView::Large
                    && diagnostics.signature != 0
                    && diagnostics.visible_cells == 64 * 48
                    && diagnostics.running_jobs == 0
            },
        ))
        .then(inspect::log_resource::<LabDiagnostics>("large solved diagnostics"))
        .then(Action::Screenshot("wfc_large_solved".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("wfc_async_large"))
        .build()
}
