use bevy::prelude::*;

use crate::{WfcBorder, WfcDirection, WfcGridSize, WfcTopology};

#[derive(Clone, Debug)]
pub struct CompiledGrid {
    topology: WfcTopology,
    size: WfcGridSize,
}

impl CompiledGrid {
    pub fn new(topology: WfcTopology, size: WfcGridSize) -> Result<Self, String> {
        if size.width == 0 || size.height == 0 || size.depth == 0 {
            return Err("grid dimensions must be greater than zero".to_string());
        }
        if matches!(topology, WfcTopology::Cartesian2d) && size.depth != 1 {
            return Err("2D topology requires depth = 1".to_string());
        }
        Ok(Self { topology, size })
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
        let next = current + direction.offset();
        if next.x < 0 || next.y < 0 || next.z < 0 {
            return None;
        }
        self.index_of(next.as_uvec3())
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
