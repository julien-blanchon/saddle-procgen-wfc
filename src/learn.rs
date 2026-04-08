use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use crate::{
    WfcAdjacencyRule, WfcDirection, WfcRuleset, WfcTileDefinition, WfcTileGrid, WfcTileId,
};

/// Scan a hand-placed sample grid and extract which tile adjacencies actually occur.
///
/// Produces a standard [`WfcRuleset`] that can be used with the tiled solver. Tile
/// weights are derived from frequency in the sample (more frequent tiles get higher weight).
///
/// This is different from the overlap model: it extracts tile-level adjacency rules,
/// not overlapping pattern windows. The output is a standard ruleset suitable for
/// direct use with [`solve_wfc`](crate::solve_wfc).
pub fn learn_adjacency_rules(sample: &WfcTileGrid) -> WfcRuleset {
    let topology = sample.topology;
    let directions = WfcDirection::active(topology);

    // Count tile occurrences for weights
    let mut tile_counts: BTreeMap<WfcTileId, u32> = BTreeMap::new();
    for (_, tile) in sample.iter() {
        *tile_counts.entry(tile).or_default() += 1;
    }

    // Collect observed adjacencies
    let mut adjacency: BTreeMap<(WfcTileId, WfcDirection), BTreeSet<WfcTileId>> = BTreeMap::new();

    let w = sample.size.width as i32;
    let h = sample.size.height as i32;
    let d = sample.size.depth as i32;

    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let pos = UVec3::new(x as u32, y as u32, z as u32);
                let Some(tile) = sample.tile_at(pos) else {
                    continue;
                };

                for &dir in directions {
                    let offset = dir.offset();
                    let nx = x + offset.x;
                    let ny = y + offset.y;
                    let nz = z + offset.z;

                    if nx < 0 || ny < 0 || nz < 0 || nx >= w || ny >= h || nz >= d {
                        continue;
                    }

                    let neighbor_pos = UVec3::new(nx as u32, ny as u32, nz as u32);
                    if let Some(neighbor_tile) = sample.tile_at(neighbor_pos) {
                        adjacency
                            .entry((tile, dir))
                            .or_default()
                            .insert(neighbor_tile);
                    }
                }
            }
        }
    }

    // Build tile definitions from observed tiles
    let tiles: Vec<WfcTileDefinition> = tile_counts
        .iter()
        .map(|(id, count)| WfcTileDefinition::new(*id, *count as f32, format!("Tile {}", id.0)))
        .collect();

    // Build adjacency rules. For tiles on the border where a direction was never
    // observed, allow all tiles in that direction (conservative fallback).
    let all_tile_ids: Vec<WfcTileId> = tile_counts.keys().copied().collect();
    let mut rules = Vec::new();

    for &dir in directions {
        for tile_id in tile_counts.keys() {
            let allowed = adjacency
                .get(&(*tile_id, dir))
                .map(|set| set.iter().copied().collect::<Vec<_>>())
                .unwrap_or_else(|| all_tile_ids.clone());
            rules.push(WfcAdjacencyRule::new(*tile_id, dir, allowed));
        }
    }

    WfcRuleset {
        topology,
        tiles,
        adjacency: rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WfcGridSize, WfcRequest, WfcSeed, WfcTileGrid, WfcTopology, solve_wfc};

    #[test]
    fn learns_rules_from_checkerboard() {
        let white = WfcTileId(0);
        let black = WfcTileId(1);

        // Hand-place a 4x4 checkerboard
        let mut sample = WfcTileGrid::new_empty(WfcTopology::Cartesian2d, WfcGridSize::new_2d(4, 4));
        for y in 0..4u32 {
            for x in 0..4u32 {
                let tile = if (x + y) % 2 == 0 { white } else { black };
                sample.set_tile_at(UVec3::new(x, y, 0), tile);
            }
        }

        let ruleset = learn_adjacency_rules(&sample);
        assert_eq!(ruleset.tiles.len(), 2);

        // White should only allow black neighbors and vice versa
        for rule in &ruleset.adjacency {
            if rule.tile == white {
                assert!(
                    rule.allowed_tiles.contains(&black),
                    "white should allow black in {:?}",
                    rule.direction
                );
                assert!(
                    !rule.allowed_tiles.contains(&white),
                    "white should not allow white in {:?}",
                    rule.direction
                );
            }
        }
    }

    #[test]
    fn learned_rules_produce_valid_output() {
        let grass = WfcTileId(0);
        let road = WfcTileId(1);

        // Simple sample: road runs horizontally, grass above and below
        let mut sample = WfcTileGrid::new_empty(WfcTopology::Cartesian2d, WfcGridSize::new_2d(6, 3));
        for x in 0..6u32 {
            sample.set_tile_at(UVec3::new(x, 0, 0), grass);
            sample.set_tile_at(UVec3::new(x, 1, 0), road);
            sample.set_tile_at(UVec3::new(x, 2, 0), grass);
        }

        let ruleset = learn_adjacency_rules(&sample);
        let request = WfcRequest::new(WfcGridSize::new_2d(10, 5), ruleset, WfcSeed(42));
        let solution = solve_wfc(&request).expect("learned rules should solve");
        assert_eq!(solution.grid.tiles.len(), 50);
    }

    #[test]
    fn learned_rules_3d() {
        let air = WfcTileId(0);
        let stone = WfcTileId(1);

        let mut sample = WfcTileGrid::new_empty(WfcTopology::Cartesian3d, WfcGridSize::new_3d(3, 3, 3));
        // Bottom layer: all stone. Top layers: all air.
        for z in 0..3u32 {
            for y in 0..3u32 {
                for x in 0..3u32 {
                    let tile = if z == 0 { stone } else { air };
                    sample.set_tile_at(UVec3::new(x, y, z), tile);
                }
            }
        }

        let ruleset = learn_adjacency_rules(&sample);
        let request = WfcRequest::new(WfcGridSize::new_3d(4, 4, 4), ruleset, WfcSeed(7));
        let solution = solve_wfc(&request).expect("3D learned rules should solve");
        assert_eq!(solution.grid.tiles.len(), 64);
    }
}
