use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use crate::{WfcGridSize, WfcSeed, WfcTileId, WfcTopology};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
pub struct WfcTileVariant {
    pub tile: WfcTileId,
    pub rotation_steps: u8,
}

impl WfcTileVariant {
    pub const fn new(tile: WfcTileId, rotation_steps: u8) -> Self {
        Self {
            tile,
            rotation_steps,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Reflect)]
pub struct WfcSolveStats {
    pub observation_count: u32,
    pub propagation_count: u32,
    pub backtrack_count: u32,
    pub contradiction_count: u32,
    pub elapsed_ms: f32,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct WfcTileGrid {
    pub topology: WfcTopology,
    pub size: WfcGridSize,
    pub tiles: Vec<WfcTileId>,
    pub rotations: Vec<u8>,
}

impl WfcTileGrid {
    pub fn signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.topology.hash(&mut hasher);
        self.size.width.hash(&mut hasher);
        self.size.height.hash(&mut hasher);
        self.size.depth.hash(&mut hasher);
        self.tiles.hash(&mut hasher);
        self.rotations.hash(&mut hasher);
        hasher.finish()
    }

    pub fn tile_at(&self, position: UVec3) -> Option<WfcTileId> {
        if position.x >= self.size.width
            || position.y >= self.size.height
            || position.z >= self.size.depth
        {
            return None;
        }
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        let index = position.z as usize * width * height
            + position.y as usize * width
            + position.x as usize;
        self.tiles.get(index).copied()
    }

    pub fn rotation_at(&self, position: UVec3) -> Option<u8> {
        if position.x >= self.size.width
            || position.y >= self.size.height
            || position.z >= self.size.depth
        {
            return None;
        }
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        let index = position.z as usize * width * height
            + position.y as usize * width
            + position.x as usize;
        self.rotations.get(index).copied()
    }

    pub fn variant_at(&self, position: UVec3) -> Option<WfcTileVariant> {
        Some(WfcTileVariant::new(
            self.tile_at(position)?,
            self.rotation_at(position)?,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct WfcCellDebug {
    pub position: UVec3,
    pub possible_tiles: Vec<WfcTileId>,
    pub possible_variants: Vec<WfcTileVariant>,
    pub possible_count: u32,
    pub entropy: f32,
    pub collapsed_tile: Option<WfcTileId>,
    pub collapsed_variant: Option<WfcTileVariant>,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct WfcContradiction {
    pub position: UVec3,
    pub last_observed_cell: Option<UVec3>,
    pub remaining_candidates: Vec<WfcTileId>,
    pub remaining_variants: Vec<WfcTileVariant>,
    pub decision_depth: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct WfcDebugSnapshot {
    pub cells: Vec<WfcCellDebug>,
    pub last_observed_cell: Option<UVec3>,
    pub contradiction: Option<WfcContradiction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Reflect)]
pub enum WfcFailureReason {
    InvalidRequest,
    Contradiction,
    BacktrackLimitReached,
    UnsatisfiedGlobalConstraint,
}

#[derive(Clone, Debug, PartialEq, Reflect, Resource)]
pub struct WfcSolution {
    pub seed: WfcSeed,
    pub grid: WfcTileGrid,
    pub stats: WfcSolveStats,
    pub debug: Option<WfcDebugSnapshot>,
    pub signature: u64,
}

#[derive(Clone, Debug, PartialEq, Reflect, Resource)]
pub struct WfcFailure {
    pub reason: WfcFailureReason,
    pub seed: WfcSeed,
    pub topology: WfcTopology,
    pub grid_size: WfcGridSize,
    pub stats: WfcSolveStats,
    pub contradiction: Option<WfcContradiction>,
    pub debug: Option<WfcDebugSnapshot>,
    pub message: String,
}
