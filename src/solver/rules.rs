use std::collections::BTreeMap;

use crate::{WfcDirection, WfcRuleset, WfcTileId, WfcTopology};

use super::bitset::DomainBits;

#[derive(Clone, Debug)]
pub struct CompiledRuleset {
    topology: WfcTopology,
    tile_ids: Vec<WfcTileId>,
    tile_indices: BTreeMap<WfcTileId, usize>,
    weights: Vec<f32>,
    allowed: Vec<Vec<DomainBits>>,
}

impl CompiledRuleset {
    pub fn compile(ruleset: &WfcRuleset) -> Result<Self, String> {
        if ruleset.tiles.is_empty() {
            return Err("ruleset must contain at least one tile".to_string());
        }

        let mut tile_ids = Vec::with_capacity(ruleset.tiles.len());
        let mut tile_indices = BTreeMap::new();
        let mut weights = Vec::with_capacity(ruleset.tiles.len());

        for (index, tile) in ruleset.tiles.iter().enumerate() {
            if !tile.weight.is_finite() || tile.weight <= 0.0 {
                return Err(format!(
                    "tile {:?} must have a finite positive weight",
                    tile.id
                ));
            }
            if tile_indices.insert(tile.id, index).is_some() {
                return Err(format!("duplicate tile id {:?}", tile.id));
            }
            tile_ids.push(tile.id);
            weights.push(tile.weight);
        }

        let directions = WfcDirection::active(ruleset.topology);
        let tile_count = tile_ids.len();
        let mut allowed = vec![vec![DomainBits::empty(tile_count); tile_count]; directions.len()];
        let mut provided = vec![vec![false; tile_count]; directions.len()];

        for rule in &ruleset.adjacency {
            let Some(direction_index) = directions.iter().position(|dir| *dir == rule.direction)
            else {
                return Err(format!(
                    "direction {:?} is invalid for topology {:?}",
                    rule.direction, ruleset.topology
                ));
            };
            let Some(&tile_index) = tile_indices.get(&rule.tile) else {
                return Err(format!(
                    "adjacency rule references unknown tile {:?}",
                    rule.tile
                ));
            };
            let mut mask = DomainBits::empty(tile_count);
            for allowed_tile in &rule.allowed_tiles {
                let Some(&allowed_index) = tile_indices.get(allowed_tile) else {
                    return Err(format!(
                        "adjacency rule references unknown allowed tile {:?}",
                        allowed_tile
                    ));
                };
                mask.insert(allowed_index);
            }
            allowed[direction_index][tile_index].or_assign(&mask);
            provided[direction_index][tile_index] = true;
        }

        for (direction_index, direction) in directions.iter().enumerate() {
            for tile_index in 0..tile_count {
                if !provided[direction_index][tile_index] {
                    return Err(format!(
                        "missing adjacency rule for tile {:?} in direction {:?}",
                        tile_ids[tile_index], direction
                    ));
                }
                if allowed[direction_index][tile_index].is_empty() {
                    return Err(format!(
                        "tile {:?} has no valid neighbors in direction {:?}",
                        tile_ids[tile_index], direction
                    ));
                }
            }
        }

        Ok(Self {
            topology: ruleset.topology,
            tile_ids,
            tile_indices,
            weights,
            allowed,
        })
    }

    pub fn topology(&self) -> WfcTopology {
        self.topology
    }

    pub fn tile_count(&self) -> usize {
        self.tile_ids.len()
    }

    pub fn tile_index(&self, tile_id: WfcTileId) -> Option<usize> {
        self.tile_indices.get(&tile_id).copied()
    }

    pub fn tile_id(&self, index: usize) -> WfcTileId {
        self.tile_ids[index]
    }

    pub fn weight(&self, index: usize) -> f32 {
        self.weights[index]
    }

    pub fn full_domain(&self) -> DomainBits {
        DomainBits::full(self.tile_count())
    }

    pub fn mask_for_tiles(&self, tile_ids: &[WfcTileId]) -> Result<DomainBits, String> {
        let mut mask = DomainBits::empty(self.tile_count());
        for tile_id in tile_ids {
            let Some(index) = self.tile_index(*tile_id) else {
                return Err(format!("unknown tile id {:?}", tile_id));
            };
            mask.insert(index);
        }
        Ok(mask)
    }

    pub fn allowed_mask(&self, direction: WfcDirection, tile_index: usize) -> &DomainBits {
        let directions = WfcDirection::active(self.topology);
        let direction_index = directions
            .iter()
            .position(|dir| *dir == direction)
            .expect("direction should be active for topology");
        &self.allowed[direction_index][tile_index]
    }
}
