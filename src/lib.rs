#![doc = include_str!("../README.md")]

mod components;
mod config;
mod debug;
mod learn;
mod messages;
mod overlap;
mod sockets;
mod solver;
mod systems;

pub use components::{WfcJob, WfcJobId, WfcJobResult, WfcJobStatus, WfcRuntimeDiagnostics};
pub use config::{
    WfcAdjacencyRule, WfcBorder, WfcBorderConstraint, WfcBoundaryStitching, WfcCellBans,
    WfcDirection, WfcFixedCell, WfcGlobalConstraint, WfcGridSize, WfcObservationHeuristic,
    WfcRequest, WfcRuleset, WfcSeed, WfcSettings, WfcTileCountConstraint, WfcTileDefinition,
    WfcTileId, WfcTileSymmetry, WfcTopology,
};
pub use debug::{
    WfcCellDebug, WfcContradiction, WfcDebugSnapshot, WfcFailure, WfcFailureReason, WfcSolution,
    WfcSolveStats, WfcTileGrid, WfcTileVariant,
};
pub use messages::{GenerateWfc, WfcFailed, WfcProgress, WfcSolved};
pub use learn::learn_adjacency_rules;
pub use overlap::{WfcOverlapOptions, WfcOverlapRequest, solve_overlap_wfc_2d};
pub use sockets::{SocketTileBuilder, WfcSocketId, WfcSocketRulesetBuilder};
pub use solver::{WfcStepCell, WfcStepSnapshot, WfcStepSolver, solve_wfc};

use bevy::{
    app::PostStartup,
    ecs::{intern::Interned, schedule::ScheduleLabel},
    prelude::*,
};

#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum WfcSystems {
    Request,
    PollJobs,
    ApplyResults,
    Debug,
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct NeverDeactivateSchedule;

pub struct WfcPlugin {
    pub activate_schedule: Interned<dyn ScheduleLabel>,
    pub deactivate_schedule: Interned<dyn ScheduleLabel>,
    pub update_schedule: Interned<dyn ScheduleLabel>,
}

impl WfcPlugin {
    pub fn new(
        activate_schedule: impl ScheduleLabel,
        deactivate_schedule: impl ScheduleLabel,
        update_schedule: impl ScheduleLabel,
    ) -> Self {
        Self {
            activate_schedule: activate_schedule.intern(),
            deactivate_schedule: deactivate_schedule.intern(),
            update_schedule: update_schedule.intern(),
        }
    }

    pub fn always_on(update_schedule: impl ScheduleLabel) -> Self {
        Self::new(PostStartup, NeverDeactivateSchedule, update_schedule)
    }
}

impl Default for WfcPlugin {
    fn default() -> Self {
        Self::always_on(Update)
    }
}

impl Plugin for WfcPlugin {
    fn build(&self, app: &mut App) {
        if self.deactivate_schedule == NeverDeactivateSchedule.intern() {
            app.init_schedule(NeverDeactivateSchedule);
        }

        app.init_resource::<systems::WfcRuntimeState>();
        if !app.world().contains_resource::<WfcRuntimeDiagnostics>() {
            app.insert_resource(WfcRuntimeDiagnostics::default());
        }

        app.add_message::<GenerateWfc>()
            .add_message::<WfcProgress>()
            .add_message::<WfcSolved>()
            .add_message::<WfcFailed>()
            .register_type::<GenerateWfc>()
            .register_type::<WfcAdjacencyRule>()
            .register_type::<WfcBorder>()
            .register_type::<WfcBorderConstraint>()
            .register_type::<WfcBoundaryStitching>()
            .register_type::<WfcCellBans>()
            .register_type::<WfcCellDebug>()
            .register_type::<WfcContradiction>()
            .register_type::<WfcDebugSnapshot>()
            .register_type::<WfcDirection>()
            .register_type::<WfcFailure>()
            .register_type::<WfcFailureReason>()
            .register_type::<WfcFixedCell>()
            .register_type::<WfcGlobalConstraint>()
            .register_type::<WfcGridSize>()
            .register_type::<WfcJob>()
            .register_type::<WfcJobId>()
            .register_type::<WfcJobResult>()
            .register_type::<WfcJobStatus>()
            .register_type::<WfcObservationHeuristic>()
            .register_type::<WfcOverlapOptions>()
            .register_type::<WfcOverlapRequest>()
            .register_type::<WfcProgress>()
            .register_type::<WfcRequest>()
            .register_type::<WfcRuleset>()
            .register_type::<WfcRuntimeDiagnostics>()
            .register_type::<WfcSeed>()
            .register_type::<WfcSettings>()
            .register_type::<WfcSolveStats>()
            .register_type::<WfcSolution>()
            .register_type::<WfcSolved>()
            .register_type::<WfcFailed>()
            .register_type::<WfcTileCountConstraint>()
            .register_type::<WfcTileDefinition>()
            .register_type::<WfcTileGrid>()
            .register_type::<WfcTileId>()
            .register_type::<WfcTileSymmetry>()
            .register_type::<WfcTileVariant>()
            .register_type::<WfcSocketId>()
            .register_type::<WfcTopology>()
            .add_systems(self.activate_schedule, systems::activate_runtime)
            .add_systems(self.deactivate_schedule, systems::deactivate_runtime)
            .configure_sets(
                self.update_schedule,
                (
                    WfcSystems::Request,
                    WfcSystems::PollJobs,
                    WfcSystems::ApplyResults,
                    WfcSystems::Debug,
                )
                    .chain(),
            )
            .add_systems(
                self.update_schedule,
                systems::request_jobs
                    .in_set(WfcSystems::Request)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                systems::poll_jobs
                    .in_set(WfcSystems::PollJobs)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                systems::apply_results
                    .in_set(WfcSystems::ApplyResults)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                systems::refresh_runtime_diagnostics
                    .in_set(WfcSystems::Debug)
                    .run_if(systems::runtime_is_active),
            );
    }
}

#[cfg(test)]
#[path = "solver_tests.rs"]
mod solver_tests;

#[cfg(test)]
#[path = "systems_tests.rs"]
mod systems_tests;

#[cfg(test)]
#[path = "overlap_tests.rs"]
mod overlap_tests;
