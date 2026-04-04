use std::collections::{BTreeMap, BTreeSet};

use crate::{WfcDirection, WfcRuleset, WfcTileId, WfcTileSymmetry, WfcTileVariant, WfcTopology};

use super::bitset::DomainBits;

#[derive(Clone, Copy, Debug)]
struct CompiledTileVariantEntry {
    tile_id: WfcTileId,
    rotation_steps: u8,
}

#[derive(Clone, Debug)]
pub struct CompiledRuleset {
    topology: WfcTopology,
    variants: Vec<CompiledTileVariantEntry>,
    tile_variants: BTreeMap<WfcTileId, Vec<usize>>,
    weights: Vec<f32>,
    allowed: Vec<Vec<DomainBits>>,
}

impl CompiledRuleset {
    pub fn compile(ruleset: &WfcRuleset) -> Result<Self, String> {
        if ruleset.tiles.is_empty() {
            return Err("ruleset must contain at least one tile".to_string());
        }

        let directions = WfcDirection::active(ruleset.topology);
        let mut base_tile_indices = BTreeMap::new();
        let mut symmetries = Vec::with_capacity(ruleset.tiles.len());
        let mut variants = Vec::new();
        let mut tile_variants = BTreeMap::new();
        let mut weights = Vec::new();

        for tile in &ruleset.tiles {
            if !tile.weight.is_finite() || tile.weight <= 0.0 {
                return Err(format!(
                    "tile {:?} must have a finite positive weight",
                    tile.id
                ));
            }
            if base_tile_indices
                .insert(tile.id, symmetries.len())
                .is_some()
            {
                return Err(format!("duplicate tile id {:?}", tile.id));
            }
            if ruleset.topology == WfcTopology::Cartesian3d
                && tile.symmetry != WfcTileSymmetry::Fixed
            {
                return Err(format!(
                    "tile {:?} uses {:?}, but automatic rotation currently supports only Cartesian2d",
                    tile.id, tile.symmetry
                ));
            }

            let unique_rotations = tile.symmetry.unique_rotations(ruleset.topology);
            let variant_weight = tile.weight / unique_rotations as f32;
            let mut variant_indices = Vec::with_capacity(unique_rotations as usize);
            for rotation_steps in 0..unique_rotations {
                let variant_index = variants.len();
                variants.push(CompiledTileVariantEntry {
                    tile_id: tile.id,
                    rotation_steps,
                });
                weights.push(variant_weight);
                variant_indices.push(variant_index);
            }
            tile_variants.insert(tile.id, variant_indices);
            symmetries.push(tile.symmetry);
        }

        let mut canonical_allowed =
            vec![vec![BTreeSet::<WfcTileId>::new(); ruleset.tiles.len()]; directions.len()];
        let mut provided = vec![vec![false; ruleset.tiles.len()]; directions.len()];

        for rule in &ruleset.adjacency {
            let Some(direction_index) = directions.iter().position(|dir| *dir == rule.direction)
            else {
                return Err(format!(
                    "direction {:?} is invalid for topology {:?}",
                    rule.direction, ruleset.topology
                ));
            };
            let Some(&tile_index) = base_tile_indices.get(&rule.tile) else {
                return Err(format!(
                    "adjacency rule references unknown tile {:?}",
                    rule.tile
                ));
            };

            for allowed_tile in &rule.allowed_tiles {
                if !base_tile_indices.contains_key(allowed_tile) {
                    return Err(format!(
                        "adjacency rule references unknown allowed tile {:?}",
                        allowed_tile
                    ));
                }
                canonical_allowed[direction_index][tile_index].insert(*allowed_tile);
            }
            provided[direction_index][tile_index] = true;
        }

        for (direction_index, direction) in directions.iter().enumerate() {
            for tile_index in 0..ruleset.tiles.len() {
                if !provided[direction_index][tile_index] {
                    return Err(format!(
                        "missing adjacency rule for tile {:?} in direction {:?}",
                        ruleset.tiles[tile_index].id, direction
                    ));
                }
                if canonical_allowed[direction_index][tile_index].is_empty() {
                    return Err(format!(
                        "tile {:?} has no valid neighbors in direction {:?}",
                        ruleset.tiles[tile_index].id, direction
                    ));
                }
            }
        }

        let variant_count = variants.len();
        let mut allowed =
            vec![vec![DomainBits::empty(variant_count); variant_count]; directions.len()];

        for tile in &ruleset.tiles {
            let source_variants = tile_variants
                .get(&tile.id)
                .expect("tile variants should exist for every tile");
            let source_tile_index = base_tile_indices[&tile.id];

            for &variant_index in source_variants {
                let rotation_steps = variants[variant_index].rotation_steps;
                for (world_direction_index, world_direction) in directions.iter().enumerate() {
                    let canonical_direction = inverse_rotate_direction(
                        *world_direction,
                        rotation_steps,
                        ruleset.topology,
                    );
                    let canonical_direction_index = directions
                        .iter()
                        .position(|direction| *direction == canonical_direction)
                        .expect("canonical direction should stay active");

                    let mut mask = DomainBits::empty(variant_count);
                    for allowed_tile_id in
                        &canonical_allowed[canonical_direction_index][source_tile_index]
                    {
                        let allowed_variant = rotated_variant_index(
                            *allowed_tile_id,
                            rotation_steps,
                            ruleset.topology,
                            &tile_variants,
                            &symmetries,
                            &base_tile_indices,
                        )?;
                        mask.insert(allowed_variant);
                    }

                    if mask.is_empty() {
                        return Err(format!(
                            "tile {:?} rotation {} has no valid neighbors in direction {:?}",
                            tile.id, rotation_steps, world_direction
                        ));
                    }
                    allowed[world_direction_index][variant_index] = mask;
                }
            }
        }

        Ok(Self {
            topology: ruleset.topology,
            variants,
            tile_variants,
            weights,
            allowed,
        })
    }

    pub fn topology(&self) -> WfcTopology {
        self.topology
    }

    pub fn tile_count(&self) -> usize {
        self.variants.len()
    }

    pub fn tile_id(&self, index: usize) -> WfcTileId {
        self.variants[index].tile_id
    }

    pub fn tile_rotation(&self, index: usize) -> u8 {
        self.variants[index].rotation_steps
    }

    pub fn tile_variant(&self, index: usize) -> WfcTileVariant {
        WfcTileVariant::new(self.tile_id(index), self.tile_rotation(index))
    }

    #[cfg(test)]
    pub fn variant_index(&self, tile_id: WfcTileId, rotation_steps: u8) -> Option<usize> {
        let variants = self.tile_variants.get(&tile_id)?;
        let unique_rotations = variants.len().max(1) as u8;
        variants
            .get((rotation_steps % unique_rotations) as usize)
            .copied()
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
            let Some(indices) = self.tile_variants.get(tile_id) else {
                return Err(format!("unknown tile id {:?}", tile_id));
            };
            for index in indices {
                mask.insert(*index);
            }
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

fn rotated_variant_index(
    tile_id: WfcTileId,
    rotation_steps: u8,
    topology: WfcTopology,
    tile_variants: &BTreeMap<WfcTileId, Vec<usize>>,
    symmetries: &[WfcTileSymmetry],
    base_tile_indices: &BTreeMap<WfcTileId, usize>,
) -> Result<usize, String> {
    let Some(base_tile_index) = base_tile_indices.get(&tile_id) else {
        return Err(format!("unknown tile id {:?}", tile_id));
    };
    let unique_rotations = symmetries[*base_tile_index].unique_rotations(topology);
    let mapped_rotation = rotation_steps % unique_rotations.max(1);
    tile_variants
        .get(&tile_id)
        .and_then(|variants| variants.get(mapped_rotation as usize))
        .copied()
        .ok_or_else(|| format!("tile {:?} has no rotation {}", tile_id, mapped_rotation))
}

fn inverse_rotate_direction(
    direction: WfcDirection,
    rotation_steps: u8,
    topology: WfcTopology,
) -> WfcDirection {
    if topology != WfcTopology::Cartesian2d {
        return direction;
    }

    let mut rotated = direction;
    for _ in 0..(4 - rotation_steps % 4) % 4 {
        rotated = rotate_direction_clockwise(rotated);
    }
    rotated
}

fn rotate_direction_clockwise(direction: WfcDirection) -> WfcDirection {
    match direction {
        WfcDirection::XPos => WfcDirection::YNeg,
        WfcDirection::YNeg => WfcDirection::XNeg,
        WfcDirection::XNeg => WfcDirection::YPos,
        WfcDirection::YPos => WfcDirection::XPos,
        WfcDirection::ZPos | WfcDirection::ZNeg => direction,
    }
}
