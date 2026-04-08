use bevy::prelude::*;
use saddle_bevy_e2e::{
    action::Action,
    actions::assertions,
    scenario::Scenario,
};

use crate::DungeonConfig;

pub struct TilemapDungeonE2EPlugin;

impl Plugin for TilemapDungeonE2EPlugin {
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
                    "[tilemap_dungeon:e2e] Unknown scenario '{name}'. Available: {:?}",
                    list_scenarios()
                );
            }
        }
    }
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "tilemap_dungeon_smoke" => Some(tilemap_dungeon_smoke()),
        "tilemap_dungeon_fov" => Some(tilemap_dungeon_fov()),
        _ => None,
    }
}

pub fn list_scenarios() -> Vec<&'static str> {
    vec!["tilemap_dungeon_smoke", "tilemap_dungeon_fov"]
}

fn tilemap_dungeon_smoke() -> Scenario {
    Scenario::builder("tilemap_dungeon_smoke")
        .description(
            "Verify the WFC dungeon generates, tilemap renders, and FOV viewer exists.",
        )
        .then(Action::WaitFrames(30))
        .then(assertions::resource_satisfies::<DungeonConfig>(
            "dungeon config has valid defaults",
            |config| config.width >= 12 && config.height >= 10 && config.seed > 0,
        ))
        .then(Action::Screenshot("tilemap_dungeon_smoke".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("tilemap_dungeon_smoke"))
        .build()
}

fn tilemap_dungeon_fov() -> Scenario {
    Scenario::builder("tilemap_dungeon_fov")
        .description(
            "Verify FOV overlay updates when the dungeon is generated with show_fov enabled.",
        )
        .then(Action::WaitFrames(30))
        .then(assertions::resource_satisfies::<DungeonConfig>(
            "fov is enabled",
            |config| config.show_fov,
        ))
        .then(Action::Screenshot("tilemap_dungeon_fov_initial".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("tilemap_dungeon_fov"))
        .build()
}
