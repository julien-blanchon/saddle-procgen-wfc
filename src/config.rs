use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Reflect,
    Serialize,
    Deserialize,
)]
pub struct WfcTileId(pub u16);

impl From<u16> for WfcTileId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<WfcTileId> for u16 {
    fn from(value: WfcTileId) -> Self {
        value.0
    }
}

impl std::fmt::Display for WfcTileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tile({})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct WfcSeed(pub u64);

impl WfcSeed {
    pub fn for_chunk(self, chunk: IVec3) -> Self {
        let mut value = self.0
            ^ ((chunk.x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            ^ ((chunk.y as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
            ^ ((chunk.z as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9));
        value ^= value >> 30;
        value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value ^= value >> 27;
        value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^= value >> 31;
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum WfcTopology {
    #[default]
    Cartesian2d,
    Cartesian3d,
    Hex2d,
}

impl std::fmt::Display for WfcTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cartesian2d => write!(f, "Cartesian2D"),
            Self::Cartesian3d => write!(f, "Cartesian3D"),
            Self::Hex2d => write!(f, "Hex2D"),
        }
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize,
)]
pub enum WfcDirection {
    XPos,
    XNeg,
    YPos,
    YNeg,
    ZPos,
    ZNeg,
    HexEast,
    HexWest,
    HexNorthEast,
    HexNorthWest,
    HexSouthEast,
    HexSouthWest,
}

impl WfcDirection {
    pub fn active(topology: WfcTopology) -> &'static [Self] {
        match topology {
            WfcTopology::Cartesian2d => &[Self::XPos, Self::XNeg, Self::YPos, Self::YNeg],
            WfcTopology::Cartesian3d => &[
                Self::XPos,
                Self::XNeg,
                Self::YPos,
                Self::YNeg,
                Self::ZPos,
                Self::ZNeg,
            ],
            WfcTopology::Hex2d => &[
                Self::HexEast,
                Self::HexWest,
                Self::HexNorthEast,
                Self::HexNorthWest,
                Self::HexSouthEast,
                Self::HexSouthWest,
            ],
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::XPos => Self::XNeg,
            Self::XNeg => Self::XPos,
            Self::YPos => Self::YNeg,
            Self::YNeg => Self::YPos,
            Self::ZPos => Self::ZNeg,
            Self::ZNeg => Self::ZPos,
            Self::HexEast => Self::HexWest,
            Self::HexWest => Self::HexEast,
            Self::HexNorthEast => Self::HexSouthWest,
            Self::HexNorthWest => Self::HexSouthEast,
            Self::HexSouthEast => Self::HexNorthWest,
            Self::HexSouthWest => Self::HexNorthEast,
        }
    }
}

impl std::fmt::Display for WfcDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::XPos => write!(f, "+X"),
            Self::XNeg => write!(f, "-X"),
            Self::YPos => write!(f, "+Y"),
            Self::YNeg => write!(f, "-Y"),
            Self::ZPos => write!(f, "+Z"),
            Self::ZNeg => write!(f, "-Z"),
            Self::HexEast => write!(f, "HexE"),
            Self::HexWest => write!(f, "HexW"),
            Self::HexNorthEast => write!(f, "HexNE"),
            Self::HexNorthWest => write!(f, "HexNW"),
            Self::HexSouthEast => write!(f, "HexSE"),
            Self::HexSouthWest => write!(f, "HexSW"),
        }
    }
}

impl WfcDirection {
    pub fn offset(self) -> IVec3 {
        match self {
            Self::XPos => IVec3::X,
            Self::XNeg => -IVec3::X,
            Self::YPos => IVec3::Y,
            Self::YNeg => -IVec3::Y,
            Self::ZPos => IVec3::Z,
            Self::ZNeg => -IVec3::Z,
            Self::HexEast => IVec3::X,
            Self::HexWest => -IVec3::X,
            Self::HexNorthEast => IVec3::new(1, -1, 0),
            Self::HexNorthWest => IVec3::new(0, -1, 0),
            Self::HexSouthEast => IVec3::new(1, 1, 0),
            Self::HexSouthWest => IVec3::new(0, 1, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum WfcBorder {
    MinX,
    MaxX,
    MinY,
    MaxY,
    MinZ,
    MaxZ,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct WfcGridSize {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

impl WfcGridSize {
    pub const fn new_2d(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
        }
    }

    pub const fn new_3d(width: u32, height: u32, depth: u32) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    pub fn total_cells(self) -> usize {
        self.width as usize * self.height as usize * self.depth as usize
    }

    pub fn as_uvec3(self) -> UVec3 {
        UVec3::new(self.width, self.height, self.depth)
    }
}

impl Default for WfcGridSize {
    fn default() -> Self {
        Self::new_2d(8, 8)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct WfcBoundaryStitching {
    pub wrap_x: bool,
    pub wrap_y: bool,
    pub wrap_z: bool,
}

impl WfcBoundaryStitching {
    pub const fn xy() -> Self {
        Self {
            wrap_x: true,
            wrap_y: true,
            wrap_z: false,
        }
    }

    pub const fn xyz() -> Self {
        Self {
            wrap_x: true,
            wrap_y: true,
            wrap_z: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcTileDefinition {
    pub id: WfcTileId,
    pub weight: f32,
    pub label: String,
    pub symmetry: WfcTileSymmetry,
}

impl WfcTileDefinition {
    pub fn new(id: impl Into<WfcTileId>, weight: f32, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            weight,
            label: label.into(),
            symmetry: WfcTileSymmetry::default(),
        }
    }

    pub fn with_symmetry(mut self, symmetry: WfcTileSymmetry) -> Self {
        self.symmetry = symmetry;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum WfcTileSymmetry {
    #[default]
    Fixed,
    Rotate2,
    Rotate4,
}

impl WfcTileSymmetry {
    pub const fn unique_rotations(self, topology: WfcTopology) -> u8 {
        match topology {
            WfcTopology::Cartesian2d => match self {
                Self::Fixed => 1,
                Self::Rotate2 => 2,
                Self::Rotate4 => 4,
            },
            WfcTopology::Cartesian3d | WfcTopology::Hex2d => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcAdjacencyRule {
    pub tile: WfcTileId,
    pub direction: WfcDirection,
    pub allowed_tiles: Vec<WfcTileId>,
}

impl WfcAdjacencyRule {
    pub fn new(
        tile: impl Into<WfcTileId>,
        direction: WfcDirection,
        allowed_tiles: impl IntoIterator<Item = WfcTileId>,
    ) -> Self {
        Self {
            tile: tile.into(),
            direction,
            allowed_tiles: allowed_tiles.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcRuleset {
    pub topology: WfcTopology,
    pub tiles: Vec<WfcTileDefinition>,
    pub adjacency: Vec<WfcAdjacencyRule>,
}

impl WfcRuleset {
    pub fn new(topology: WfcTopology, tiles: Vec<WfcTileDefinition>) -> Self {
        Self {
            topology,
            tiles,
            adjacency: Vec::new(),
        }
    }

    pub fn with_rule(
        mut self,
        tile: impl Into<WfcTileId>,
        direction: WfcDirection,
        allowed_tiles: impl IntoIterator<Item = WfcTileId>,
    ) -> Self {
        self.adjacency
            .push(WfcAdjacencyRule::new(tile, direction, allowed_tiles));
        self
    }

    pub fn add_rule(
        &mut self,
        tile: impl Into<WfcTileId>,
        direction: WfcDirection,
        allowed_tiles: impl IntoIterator<Item = WfcTileId>,
    ) {
        self.adjacency
            .push(WfcAdjacencyRule::new(tile, direction, allowed_tiles));
    }

    /// Add a symmetric rule: if `a` allows `b` in `direction`, then `b` allows `a`
    /// in the opposite direction.
    pub fn with_symmetric_rule(
        mut self,
        tile_a: impl Into<WfcTileId>,
        direction: WfcDirection,
        tile_b: impl Into<WfcTileId>,
    ) -> Self {
        let a = tile_a.into();
        let b = tile_b.into();
        self.adjacency
            .push(WfcAdjacencyRule::new(a, direction, [b]));
        self.adjacency
            .push(WfcAdjacencyRule::new(b, direction.opposite(), [a]));
        self
    }

    /// Add rules for all active directions at once: the given tile allows
    /// the listed neighbors in every direction for the ruleset's topology.
    pub fn with_all_direction_rules(
        mut self,
        tile: impl Into<WfcTileId>,
        allowed_tiles: impl IntoIterator<Item = WfcTileId> + Clone,
    ) -> Self {
        let tile_id = tile.into();
        for &direction in WfcDirection::active(self.topology) {
            self.adjacency.push(WfcAdjacencyRule::new(
                tile_id,
                direction,
                allowed_tiles.clone(),
            ));
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcFixedCell {
    pub position: UVec3,
    pub tile: WfcTileId,
}

impl WfcFixedCell {
    pub fn new(position: UVec3, tile: impl Into<WfcTileId>) -> Self {
        Self {
            position,
            tile: tile.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcCellBans {
    pub position: UVec3,
    pub banned_tiles: Vec<WfcTileId>,
}

impl WfcCellBans {
    pub fn new(position: UVec3, banned_tiles: impl IntoIterator<Item = WfcTileId>) -> Self {
        Self {
            position,
            banned_tiles: banned_tiles.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcBorderConstraint {
    pub border: WfcBorder,
    pub allowed_tiles: Vec<WfcTileId>,
}

impl WfcBorderConstraint {
    pub fn new(border: WfcBorder, allowed_tiles: impl IntoIterator<Item = WfcTileId>) -> Self {
        Self {
            border,
            allowed_tiles: allowed_tiles.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcTileCountConstraint {
    pub tile: WfcTileId,
    pub min_count: Option<u32>,
    pub max_count: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub enum WfcGlobalConstraint {
    TileCount(WfcTileCountConstraint),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum WfcObservationHeuristic {
    MinimumRemainingValues,
    #[default]
    MinimumEntropy,
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcSettings {
    pub observation_heuristic: WfcObservationHeuristic,
    pub max_backtracks: u32,
    pub capture_debug_snapshot: bool,
}

impl Default for WfcSettings {
    fn default() -> Self {
        Self {
            observation_heuristic: WfcObservationHeuristic::MinimumEntropy,
            max_backtracks: 256,
            capture_debug_snapshot: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct WfcRequest {
    pub grid_size: WfcGridSize,
    pub ruleset: WfcRuleset,
    pub seed: WfcSeed,
    pub settings: WfcSettings,
    pub boundary_stitching: WfcBoundaryStitching,
    pub fixed_cells: Vec<WfcFixedCell>,
    pub banned_cells: Vec<WfcCellBans>,
    pub border_constraints: Vec<WfcBorderConstraint>,
    pub global_constraints: Vec<WfcGlobalConstraint>,
}

impl WfcRequest {
    pub fn new(grid_size: WfcGridSize, ruleset: WfcRuleset, seed: WfcSeed) -> Self {
        Self {
            grid_size,
            ruleset,
            seed,
            settings: WfcSettings::default(),
            boundary_stitching: WfcBoundaryStitching::default(),
            fixed_cells: Vec::new(),
            banned_cells: Vec::new(),
            border_constraints: Vec::new(),
            global_constraints: Vec::new(),
        }
    }
}
