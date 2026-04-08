use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{WfcGridSize, WfcSeed, WfcTileId, WfcTopology};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcSolveStats {
    pub observation_count: u32,
    pub propagation_count: u32,
    pub backtrack_count: u32,
    pub contradiction_count: u32,
    pub elapsed_ms: f32,
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcTileGrid {
    pub topology: WfcTopology,
    pub size: WfcGridSize,
    pub tiles: Vec<WfcTileId>,
    pub rotations: Vec<u8>,
}

impl WfcTileGrid {
    /// Create an empty grid filled with `WfcTileId(0)` and rotation 0.
    pub fn new_empty(topology: WfcTopology, size: WfcGridSize) -> Self {
        let total = size.total_cells();
        Self {
            topology,
            size,
            tiles: vec![WfcTileId(0); total],
            rotations: vec![0; total],
        }
    }

    pub fn width(&self) -> u32 {
        self.size.width
    }

    pub fn height(&self) -> u32 {
        self.size.height
    }

    pub fn depth(&self) -> u32 {
        self.size.depth
    }

    /// Compute a deterministic signature for this grid.
    ///
    /// Uses FNV-1a so the result is stable across Rust versions and platforms.
    pub fn signature(&self) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        let mix = |hash: &mut u64, val: u64| {
            *hash ^= val;
            *hash = hash.wrapping_mul(0x100000001b3);
        };
        mix(&mut hash, self.topology as u64);
        mix(&mut hash, self.size.width as u64);
        mix(&mut hash, self.size.height as u64);
        mix(&mut hash, self.size.depth as u64);
        for tile in &self.tiles {
            mix(&mut hash, tile.0 as u64);
        }
        for rotation in &self.rotations {
            mix(&mut hash, *rotation as u64);
        }
        hash
    }

    pub fn tile_at(&self, position: UVec3) -> Option<WfcTileId> {
        self.index_of(position).map(|i| self.tiles[i])
    }

    pub fn rotation_at(&self, position: UVec3) -> Option<u8> {
        self.index_of(position).map(|i| self.rotations[i])
    }

    pub fn variant_at(&self, position: UVec3) -> Option<WfcTileVariant> {
        let i = self.index_of(position)?;
        Some(WfcTileVariant::new(self.tiles[i], self.rotations[i]))
    }

    /// Set the tile at the given position. No-op if out of bounds.
    pub fn set_tile_at(&mut self, position: UVec3, tile: WfcTileId) {
        if let Some(i) = self.index_of(position) {
            self.tiles[i] = tile;
        }
    }

    /// Set the full variant (tile + rotation) at the given position.
    pub fn set_variant_at(&mut self, position: UVec3, variant: WfcTileVariant) {
        if let Some(i) = self.index_of(position) {
            self.tiles[i] = variant.tile;
            self.rotations[i] = variant.rotation_steps;
        }
    }

    /// Iterate over all cells yielding `(position, tile_id)`.
    pub fn iter(&self) -> impl Iterator<Item = (UVec3, WfcTileId)> + '_ {
        (0..self.size.total_cells()).map(move |i| (self.position_of(i), self.tiles[i]))
    }

    /// Iterate over all cells yielding `(position, variant)`.
    pub fn iter_variants(&self) -> impl Iterator<Item = (UVec3, WfcTileVariant)> + '_ {
        (0..self.size.total_cells())
            .map(move |i| (self.position_of(i), WfcTileVariant::new(self.tiles[i], self.rotations[i])))
    }

    fn index_of(&self, position: UVec3) -> Option<usize> {
        if position.x >= self.size.width
            || position.y >= self.size.height
            || position.z >= self.size.depth
        {
            return None;
        }
        let w = self.size.width as usize;
        let h = self.size.height as usize;
        Some(position.z as usize * w * h + position.y as usize * w + position.x as usize)
    }

    fn position_of(&self, index: usize) -> UVec3 {
        let w = self.size.width as usize;
        let h = self.size.height as usize;
        let plane = w * h;
        let z = index / plane;
        let rem = index % plane;
        UVec3::new((rem % w) as u32, (rem / w) as u32, z as u32)
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcCellDebug {
    pub position: UVec3,
    pub possible_tiles: Vec<WfcTileId>,
    pub possible_variants: Vec<WfcTileVariant>,
    pub possible_count: u32,
    pub entropy: f32,
    pub collapsed_tile: Option<WfcTileId>,
    pub collapsed_variant: Option<WfcTileVariant>,
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcContradiction {
    pub position: UVec3,
    pub last_observed_cell: Option<UVec3>,
    pub remaining_candidates: Vec<WfcTileId>,
    pub remaining_variants: Vec<WfcTileVariant>,
    pub decision_depth: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcDebugSnapshot {
    pub cells: Vec<WfcCellDebug>,
    pub last_observed_cell: Option<UVec3>,
    pub contradiction: Option<WfcContradiction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum WfcFailureReason {
    InvalidRequest,
    Contradiction,
    BacktrackLimitReached,
    UnsatisfiedGlobalConstraint,
}

impl std::fmt::Display for WfcFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest => write!(f, "InvalidRequest"),
            Self::Contradiction => write!(f, "Contradiction"),
            Self::BacktrackLimitReached => write!(f, "BacktrackLimitReached"),
            Self::UnsatisfiedGlobalConstraint => write!(f, "UnsatisfiedGlobalConstraint"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Resource, Serialize, Deserialize)]
pub struct WfcSolution {
    pub seed: WfcSeed,
    pub grid: WfcTileGrid,
    pub stats: WfcSolveStats,
    pub debug: Option<WfcDebugSnapshot>,
    pub signature: u64,
}

#[derive(Clone, Debug, PartialEq, Reflect, Resource, Serialize, Deserialize)]
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
