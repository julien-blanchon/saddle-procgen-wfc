use bevy::prelude::*;
use saddle_bevy_e2e::{
    action::Action,
    actions::assertions,
    scenario::Scenario,
};

use crate::{DungeonConfig, DungeonState};

pub struct NavmeshDungeonE2EPlugin;

impl Plugin for NavmeshDungeonE2EPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(saddle_bevy_e2e::E2EPlugin);
        let args: Vec<String> = std::env::args().collect();
        let scenario_name = args.iter().skip(1).find(|a| !a.starts_with('-')).cloned();
        if let Some(name) = scenario_name {
            if let Some(mut scenario) = scenario_by_name(&name) {
                if args.iter().any(|a| a == "--handoff")
                    || std::env::var("E2E_HANDOFF").is_ok_and(|v| v == "1" || v == "true")
                {
                    scenario.actions.push(Action::Handoff);
                }
                saddle_bevy_e2e::init_scenario(app, scenario);
            } else {
                error!(
                    "[navmesh_dungeon:e2e] Unknown scenario '{name}'. Available: {:?}",
                    list_scenarios()
                );
            }
        }
    }
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "navmesh_dungeon_smoke" => Some(navmesh_dungeon_smoke()),
        "navmesh_dungeon_agent" => Some(navmesh_dungeon_agent()),
        _ => None,
    }
}

pub fn list_scenarios() -> Vec<&'static str> {
    vec!["navmesh_dungeon_smoke", "navmesh_dungeon_agent"]
}

fn navmesh_dungeon_smoke() -> Scenario {
    Scenario::builder("navmesh_dungeon_smoke")
        .description(
            "Verify the WFC dungeon generates and navmesh surface + agent are spawned.",
        )
        .then(Action::WaitFrames(30))
        .then(assertions::resource_satisfies::<DungeonState>(
            "dungeon is built with surface and agent",
            |state| state.built && state.surface_entity.is_some() && state.agent_entity.is_some(),
        ))
        .then(Action::Screenshot("navmesh_dungeon_smoke".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("navmesh_dungeon_smoke"))
        .build()
}

fn navmesh_dungeon_agent() -> Scenario {
    Scenario::builder("navmesh_dungeon_agent")
        .description(
            "Wait for the agent to start moving toward the exit and verify progress.",
        )
        .then(Action::WaitFrames(60))
        .then(assertions::resource_satisfies::<DungeonState>(
            "dungeon built and entrance/exit positions are distinct",
            |state| {
                state.built
                    && state.entrance_pos.distance(state.exit_pos) > 1.0
            },
        ))
        .then(Action::Screenshot("navmesh_dungeon_agent_moving".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("navmesh_dungeon_agent"))
        .build()
}
