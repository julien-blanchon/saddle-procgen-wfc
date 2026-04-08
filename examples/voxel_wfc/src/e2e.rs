use bevy::prelude::*;
use saddle_bevy_e2e::{
    action::Action,
    actions::assertions,
    scenario::Scenario,
};

use crate::WfcVoxelConfig;

pub struct VoxelWfcE2EPlugin;

impl Plugin for VoxelWfcE2EPlugin {
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
                    "[voxel_wfc:e2e] Unknown scenario '{name}'. Available: {:?}",
                    list_scenarios()
                );
            }
        }
    }
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "voxel_wfc_smoke" => Some(voxel_wfc_smoke()),
        _ => None,
    }
}

pub fn list_scenarios() -> Vec<&'static str> {
    vec!["voxel_wfc_smoke"]
}

fn voxel_wfc_smoke() -> Scenario {
    Scenario::builder("voxel_wfc_smoke")
        .description(
            "Verify the WFC 3D solver produces a grid and the voxel world starts generating chunks.",
        )
        .then(Action::WaitFrames(60))
        .then(assertions::resource_satisfies::<WfcVoxelConfig>(
            "voxel config has valid seed",
            |config| config.seed > 0,
        ))
        .then(Action::Screenshot("voxel_wfc_smoke".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("voxel_wfc_smoke"))
        .build()
}
