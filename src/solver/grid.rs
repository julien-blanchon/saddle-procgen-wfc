use bevy::prelude::*;

use crate::{WfcBorder, WfcBoundaryStitching, WfcDirection, WfcGridSize, WfcTopology};

#[derive(Clone, Debug)]
pub struct CompiledGrid {
    topology: WfcTopology,
    size: WfcGridSize,
    boundary_stitching: WfcBoundaryStitching,
}

impl CompiledGrid {
    pub fn new(
        topology: WfcTopology,
        size: WfcGridSize,
        boundary_stitching: WfcBoundaryStitching,
    ) -> Result<Self, String> {
        if size.width == 0 || size.height == 0 || size.depth == 0 {
            return Err("grid dimensions must be greater than zero".to_string());
        }
        if matches!(topology, WfcTopology::Cartesian2d) && size.depth != 1 {
            return Err("2D topology requires depth = 1".to_string());
        }
        if matches!(topology, WfcTopology::Hex2d) && size.depth != 1 {
            return Err("hex topology requires depth = 1".to_string());
        }
        Ok(Self {
            topology,
            size,
            boundary_stitching,
        })
    }

    pub fn topology(&self) -> WfcTopology {
        self.topology
    }

    pub fn size(&self) -> WfcGridSize {
        self.size
    }

    pub fn total_cells(&self) -> usize {
        self.size.total_cells()
    }

    pub fn directions(&self) -> &'static [WfcDirection] {
        WfcDirection::active(self.topology)
    }

    pub fn index_of(&self, position: UVec3) -> Option<usize> {
        if position.x >= self.size.width
            || position.y >= self.size.height
            || position.z >= self.size.depth
        {
            return None;
        }
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        Some(
            position.z as usize * width * height
                + position.y as usize * width
                + position.x as usize,
        )
    }

    pub fn position_of(&self, index: usize) -> UVec3 {
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        let plane = width * height;
        let z = index / plane;
        let remainder = index % plane;
        let y = remainder / width;
        let x = remainder % width;
        UVec3::new(x as u32, y as u32, z as u32)
    }

    pub fn neighbor(&self, index: usize, direction: WfcDirection) -> Option<usize> {
        let current = self.position_of(index).as_ivec3();
        let next = match self.topology {
            WfcTopology::Cartesian2d | WfcTopology::Cartesian3d => current + direction.offset(),
            WfcTopology::Hex2d => hex_neighbor(current, direction),
        };
        self.wrap_or_reject(next)
    }

    pub fn is_on_border(&self, index: usize, border: WfcBorder) -> bool {
        let position = self.position_of(index);
        match border {
            WfcBorder::MinX => position.x == 0,
            WfcBorder::MaxX => position.x + 1 == self.size.width,
            WfcBorder::MinY => position.y == 0,
            WfcBorder::MaxY => position.y + 1 == self.size.height,
            WfcBorder::MinZ => position.z == 0,
            WfcBorder::MaxZ => position.z + 1 == self.size.depth,
        }
    }
}

impl CompiledGrid {
    fn wrap_or_reject(&self, next: IVec3) -> Option<usize> {
        let wrapped = IVec3::new(
            wrap_axis(next.x, self.size.width, self.boundary_stitching.wrap_x)?,
            wrap_axis(next.y, self.size.height, self.boundary_stitching.wrap_y)?,
            wrap_axis(next.z, self.size.depth, self.boundary_stitching.wrap_z)?,
        );
        self.index_of(wrapped.as_uvec3())
    }
}

fn wrap_axis(value: i32, extent: u32, wrap: bool) -> Option<i32> {
    let extent = extent as i32;
    if (0..extent).contains(&value) {
        return Some(value);
    }
    if !wrap || extent <= 0 {
        return None;
    }
    Some(value.rem_euclid(extent))
}

fn hex_neighbor(current: IVec3, direction: WfcDirection) -> IVec3 {
    let odd_row = current.y.rem_euclid(2) != 0;
    let delta = match direction {
        WfcDirection::HexEast => IVec3::new(1, 0, 0),
        WfcDirection::HexWest => IVec3::new(-1, 0, 0),
        WfcDirection::HexNorthEast => {
            if odd_row {
                IVec3::new(1, -1, 0)
            } else {
                IVec3::new(0, -1, 0)
            }
        }
        WfcDirection::HexNorthWest => {
            if odd_row {
                IVec3::new(0, -1, 0)
            } else {
                IVec3::new(-1, -1, 0)
            }
        }
        WfcDirection::HexSouthEast => {
            if odd_row {
                IVec3::new(1, 1, 0)
            } else {
                IVec3::new(0, 1, 0)
            }
        }
        WfcDirection::HexSouthWest => {
            if odd_row {
                IVec3::new(0, 1, 0)
            } else {
                IVec3::new(-1, 1, 0)
            }
        }
        _ => IVec3::ZERO,
    };
    current + delta
}
