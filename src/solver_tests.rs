use std::collections::BTreeSet;

use super::*;

fn alternating_rules_2d() -> WfcRuleset {
    let white = WfcTileId(0);
    let black = WfcTileId(1);
    WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(white, 1.0, "White"),
            WfcTileDefinition::new(black, 1.0, "Black"),
        ],
    )
    .with_rule(white, WfcDirection::XPos, [black])
    .with_rule(white, WfcDirection::XNeg, [black])
    .with_rule(white, WfcDirection::YPos, [black])
    .with_rule(white, WfcDirection::YNeg, [black])
    .with_rule(black, WfcDirection::XPos, [white])
    .with_rule(black, WfcDirection::XNeg, [white])
    .with_rule(black, WfcDirection::YPos, [white])
    .with_rule(black, WfcDirection::YNeg, [white])
}

fn flexible_rules_2d() -> WfcRuleset {
    let grass = WfcTileId(0);
    let road = WfcTileId(1);
    let water = WfcTileId(2);
    let all = [grass, road, water];
    WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(grass, 5.0, "Grass"),
            WfcTileDefinition::new(road, 2.0, "Road"),
            WfcTileDefinition::new(water, 1.0, "Water"),
        ],
    )
    .with_rule(grass, WfcDirection::XPos, all)
    .with_rule(grass, WfcDirection::XNeg, all)
    .with_rule(grass, WfcDirection::YPos, all)
    .with_rule(grass, WfcDirection::YNeg, all)
    .with_rule(road, WfcDirection::XPos, [grass, road])
    .with_rule(road, WfcDirection::XNeg, [grass, road])
    .with_rule(road, WfcDirection::YPos, [grass, road])
    .with_rule(road, WfcDirection::YNeg, [grass, road])
    .with_rule(water, WfcDirection::XPos, [grass, water])
    .with_rule(water, WfcDirection::XNeg, [grass, water])
    .with_rule(water, WfcDirection::YPos, [grass, water])
    .with_rule(water, WfcDirection::YNeg, [grass, water])
}

fn autorotation_rules_2d() -> WfcRuleset {
    let meadow = WfcTileId(0);
    let straight = WfcTileId(1);
    let corner = WfcTileId(2);
    WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(meadow, 3.0, "Meadow"),
            WfcTileDefinition::new(straight, 1.0, "Straight")
                .with_symmetry(WfcTileSymmetry::Rotate2),
            WfcTileDefinition::new(corner, 1.0, "Corner").with_symmetry(WfcTileSymmetry::Rotate4),
        ],
    )
    .with_rule(meadow, WfcDirection::XPos, [meadow, straight, corner])
    .with_rule(meadow, WfcDirection::XNeg, [meadow, straight, corner])
    .with_rule(meadow, WfcDirection::YPos, [meadow, straight, corner])
    .with_rule(meadow, WfcDirection::YNeg, [meadow, straight, corner])
    .with_rule(straight, WfcDirection::XPos, [meadow])
    .with_rule(straight, WfcDirection::XNeg, [meadow])
    .with_rule(straight, WfcDirection::YPos, [meadow, straight, corner])
    .with_rule(straight, WfcDirection::YNeg, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::XPos, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::XNeg, [meadow])
    .with_rule(corner, WfcDirection::YPos, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::YNeg, [meadow])
}

fn stacked_rules_3d() -> WfcRuleset {
    let air = WfcTileId(0);
    let stone = WfcTileId(1);
    WfcRuleset::new(
        WfcTopology::Cartesian3d,
        vec![
            WfcTileDefinition::new(air, 2.0, "Air"),
            WfcTileDefinition::new(stone, 1.0, "Stone"),
        ],
    )
    .with_rule(air, WfcDirection::XPos, [air, stone])
    .with_rule(air, WfcDirection::XNeg, [air, stone])
    .with_rule(air, WfcDirection::YPos, [air, stone])
    .with_rule(air, WfcDirection::YNeg, [air, stone])
    .with_rule(air, WfcDirection::ZPos, [air])
    .with_rule(air, WfcDirection::ZNeg, [air, stone])
    .with_rule(stone, WfcDirection::XPos, [stone, air])
    .with_rule(stone, WfcDirection::XNeg, [stone, air])
    .with_rule(stone, WfcDirection::YPos, [stone, air])
    .with_rule(stone, WfcDirection::YNeg, [stone, air])
    .with_rule(stone, WfcDirection::ZPos, [stone, air])
    .with_rule(stone, WfcDirection::ZNeg, [stone])
}

fn forced_backtrack_rules() -> WfcRequest {
    let a = WfcTileId(0);
    let b = WfcTileId(1);
    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(a, 8.0, "A"),
            WfcTileDefinition::new(b, 1.0, "B"),
        ],
    )
    .with_rule(a, WfcDirection::XPos, [a])
    .with_rule(a, WfcDirection::XNeg, [a])
    .with_rule(a, WfcDirection::YPos, [a])
    .with_rule(a, WfcDirection::YNeg, [a])
    .with_rule(b, WfcDirection::XPos, [b])
    .with_rule(b, WfcDirection::XNeg, [b])
    .with_rule(b, WfcDirection::YPos, [b])
    .with_rule(b, WfcDirection::YNeg, [b]);
    let mut request = WfcRequest::new(WfcGridSize::new_2d(2, 1), ruleset, WfcSeed(1));
    request
        .global_constraints
        .push(WfcGlobalConstraint::TileCount(WfcTileCountConstraint {
            tile: b,
            min_count: Some(2),
            max_count: Some(2),
        }));
    request
}

fn verify_solution(request: &WfcRequest, solution: &WfcSolution) {
    let grid = super::solver::grid::CompiledGrid::new(request.ruleset.topology, request.grid_size)
        .expect("request grid should compile");
    let rules = super::solver::rules::CompiledRuleset::compile(&request.ruleset)
        .expect("rules should compile");

    for cell in 0..grid.total_cells() {
        let position = grid.position_of(cell);
        let tile = solution
            .grid
            .tile_at(position)
            .expect("solution should contain every cell");
        let rotation = solution
            .grid
            .rotation_at(position)
            .expect("solution rotation should exist");
        let tile_index = rules
            .variant_index(tile, rotation)
            .expect("solution tile variant should exist");
        for &direction in grid.directions() {
            if let Some(neighbor) = grid.neighbor(cell, direction) {
                let neighbor_position = grid.position_of(neighbor);
                let neighbor_tile = solution
                    .grid
                    .tile_at(neighbor_position)
                    .expect("neighbor should exist");
                let neighbor_rotation = solution
                    .grid
                    .rotation_at(neighbor_position)
                    .expect("neighbor rotation should exist");
                let neighbor_index = rules
                    .variant_index(neighbor_tile, neighbor_rotation)
                    .expect("neighbor tile variant should exist");
                assert!(
                    rules
                        .allowed_mask(direction, tile_index)
                        .contains(neighbor_index),
                    "tile {:?} at {:?} is not compatible with {:?} at {:?} in direction {:?}",
                    tile,
                    position,
                    neighbor_tile,
                    neighbor_position,
                    direction
                );
            }
        }
    }

    for fixed in &request.fixed_cells {
        assert_eq!(solution.grid.tile_at(fixed.position), Some(fixed.tile));
    }

    for bans in &request.banned_cells {
        let tile = solution
            .grid
            .tile_at(bans.position)
            .expect("banned cell should exist");
        assert!(!bans.banned_tiles.contains(&tile));
    }

    for border in &request.border_constraints {
        for cell in 0..grid.total_cells() {
            if grid.is_on_border(cell, border.border) {
                let tile = solution
                    .grid
                    .tile_at(grid.position_of(cell))
                    .expect("border cell should exist");
                assert!(border.allowed_tiles.contains(&tile));
            }
        }
    }

    for constraint in &request.global_constraints {
        match constraint {
            WfcGlobalConstraint::TileCount(count) => {
                let actual = solution
                    .grid
                    .tiles
                    .iter()
                    .filter(|tile| **tile == count.tile)
                    .count() as u32;
                if let Some(min_count) = count.min_count {
                    assert!(actual >= min_count);
                }
                if let Some(max_count) = count.max_count {
                    assert!(actual <= max_count);
                }
            }
        }
    }
}

#[test]
fn solves_alternating_grid_and_respects_rules() {
    let request = WfcRequest::new(
        WfcGridSize::new_2d(6, 6),
        alternating_rules_2d(),
        WfcSeed(42),
    );

    let solution = solve_wfc(&request).expect("alternating rules should solve");
    assert_eq!(solution.grid.tiles.len(), 36);
    verify_solution(&request, &solution);
}

#[test]
fn weighted_solves_are_deterministic_for_the_same_seed() {
    let request = WfcRequest::new(WfcGridSize::new_2d(10, 10), flexible_rules_2d(), WfcSeed(9));

    let a = solve_wfc(&request).expect("first solve should succeed");
    let b = solve_wfc(&request).expect("second solve should succeed");

    assert_eq!(a.signature, b.signature);
    assert_eq!(a.grid.tiles, b.grid.tiles);
}

#[test]
fn different_seeds_produce_multiple_valid_outputs() {
    let rules = flexible_rules_2d();
    let mut signatures = BTreeSet::new();

    for seed in 0..10 {
        let request = WfcRequest::new(WfcGridSize::new_2d(10, 10), rules.clone(), WfcSeed(seed));
        let solution = solve_wfc(&request).expect("solve should succeed");
        verify_solution(&request, &solution);
        signatures.insert(solution.signature);
    }

    assert!(
        signatures.len() >= 2,
        "different seeds should explore at least two valid outputs"
    );
}

#[test]
fn compiler_rotates_tile_families_and_preserves_unique_weights() {
    let meadow = WfcTileId(0);
    let straight = WfcTileId(1);
    let ruleset = autorotation_rules_2d();
    let compiled = super::solver::rules::CompiledRuleset::compile(&ruleset)
        .expect("autorotation rules should compile");

    assert_eq!(compiled.tile_count(), 7);

    let straight_vertical = compiled
        .variant_index(straight, 0)
        .expect("vertical straight should exist");
    let straight_horizontal = compiled
        .variant_index(straight, 1)
        .expect("horizontal straight should exist");
    let meadow_variant = compiled
        .variant_index(meadow, 0)
        .expect("meadow should exist");

    assert_eq!(compiled.tile_rotation(straight_vertical), 0);
    assert_eq!(compiled.tile_rotation(straight_horizontal), 1);
    assert!((compiled.weight(straight_vertical) - 0.5).abs() <= f32::EPSILON);

    assert!(
        compiled
            .allowed_mask(WfcDirection::XPos, straight_vertical)
            .contains(meadow_variant)
    );
    assert!(
        !compiled
            .allowed_mask(WfcDirection::XPos, straight_vertical)
            .contains(straight_horizontal)
    );
    assert!(
        compiled
            .allowed_mask(WfcDirection::XPos, straight_horizontal)
            .contains(straight_horizontal)
    );
    assert!(
        !compiled
            .allowed_mask(WfcDirection::XPos, straight_horizontal)
            .contains(straight_vertical)
    );
    let corner_horizontal = compiled
        .variant_index(WfcTileId(2), 1)
        .expect("rotated corner should exist");
    assert!(
        compiled
            .allowed_mask(WfcDirection::XPos, straight_horizontal)
            .contains(corner_horizontal)
    );
}

#[test]
fn contradictory_constraints_report_failure() {
    let a = WfcTileId(0);
    let b = WfcTileId(1);
    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(a, 1.0, "A"),
            WfcTileDefinition::new(b, 1.0, "B"),
        ],
    )
    .with_rule(a, WfcDirection::XPos, [a, b])
    .with_rule(a, WfcDirection::XNeg, [a, b])
    .with_rule(a, WfcDirection::YPos, [a, b])
    .with_rule(a, WfcDirection::YNeg, [a, b])
    .with_rule(b, WfcDirection::XPos, [a, b])
    .with_rule(b, WfcDirection::XNeg, [a, b])
    .with_rule(b, WfcDirection::YPos, [a, b])
    .with_rule(b, WfcDirection::YNeg, [a, b]);
    let mut request = WfcRequest::new(WfcGridSize::new_2d(1, 1), ruleset, WfcSeed(3));
    request.border_constraints = vec![
        WfcBorderConstraint::new(WfcBorder::MinX, [a]),
        WfcBorderConstraint::new(WfcBorder::MaxX, [b]),
    ];

    let failure = solve_wfc(&request).expect_err("conflicting borders must fail");
    assert_eq!(failure.reason, WfcFailureReason::Contradiction);
    assert!(failure.contradiction.is_some());
}

#[test]
fn backtracking_recovers_from_a_bad_first_choice() {
    let request = forced_backtrack_rules();

    let solution = solve_wfc(&request).expect("backtracking should find the all-B solution");
    assert!(solution.stats.backtrack_count >= 1);
    assert!(solution.grid.tiles.iter().all(|tile| *tile == WfcTileId(1)));
}

#[test]
fn fixed_cells_and_borders_are_preserved() {
    let wall = WfcTileId(0);
    let floor = WfcTileId(1);
    let all = [wall, floor];
    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(wall, 1.0, "Wall"),
            WfcTileDefinition::new(floor, 1.0, "Floor"),
        ],
    )
    .with_rule(wall, WfcDirection::XPos, all)
    .with_rule(wall, WfcDirection::XNeg, all)
    .with_rule(wall, WfcDirection::YPos, all)
    .with_rule(wall, WfcDirection::YNeg, all)
    .with_rule(floor, WfcDirection::XPos, all)
    .with_rule(floor, WfcDirection::XNeg, all)
    .with_rule(floor, WfcDirection::YPos, all)
    .with_rule(floor, WfcDirection::YNeg, all);
    let mut request = WfcRequest::new(WfcGridSize::new_2d(5, 5), ruleset, WfcSeed(11));
    request
        .fixed_cells
        .push(WfcFixedCell::new(UVec3::new(2, 2, 0), floor));
    request.border_constraints = vec![
        WfcBorderConstraint::new(WfcBorder::MinX, [wall]),
        WfcBorderConstraint::new(WfcBorder::MaxX, [wall]),
        WfcBorderConstraint::new(WfcBorder::MinY, [wall]),
        WfcBorderConstraint::new(WfcBorder::MaxY, [wall]),
    ];

    let solution = solve_wfc(&request).expect("bordered room should solve");
    verify_solution(&request, &solution);
}

#[test]
fn per_cell_bans_are_preserved() {
    let meadow = WfcTileId(0);
    let road = WfcTileId(1);
    let water = WfcTileId(2);
    let mut request = WfcRequest::new(WfcGridSize::new_2d(6, 6), flexible_rules_2d(), WfcSeed(13));
    request.banned_cells = vec![
        WfcCellBans::new(UVec3::new(0, 0, 0), [water]),
        WfcCellBans::new(UVec3::new(3, 3, 0), [road, water]),
        WfcCellBans::new(UVec3::new(5, 5, 0), [meadow]),
    ];

    let solution = solve_wfc(&request).expect("banned cells should still allow a valid solve");
    verify_solution(&request, &solution);
}

#[test]
fn three_dimensional_rules_solve_and_validate() {
    let request = WfcRequest::new(
        WfcGridSize::new_3d(4, 4, 4),
        stacked_rules_3d(),
        WfcSeed(27),
    );

    let solution = solve_wfc(&request).expect("3D rules should solve");
    assert_eq!(solution.grid.tiles.len(), 64);
    verify_solution(&request, &solution);
}

#[test]
fn grid_indexing_matches_expected_neighbors_in_two_and_three_dimensions() {
    let grid_2d =
        super::solver::grid::CompiledGrid::new(WfcTopology::Cartesian2d, WfcGridSize::new_2d(3, 2))
            .expect("2D grid should compile");
    let center = grid_2d
        .index_of(UVec3::new(1, 1, 0))
        .expect("center should exist");
    assert_eq!(grid_2d.neighbor(center, WfcDirection::XNeg), Some(3));
    assert_eq!(grid_2d.neighbor(center, WfcDirection::YNeg), Some(1));
    assert_eq!(grid_2d.neighbor(center, WfcDirection::YPos), None);

    let grid_3d = super::solver::grid::CompiledGrid::new(
        WfcTopology::Cartesian3d,
        WfcGridSize::new_3d(2, 2, 2),
    )
    .expect("3D grid should compile");
    let corner = grid_3d
        .index_of(UVec3::new(0, 0, 0))
        .expect("corner should exist");
    assert_eq!(grid_3d.neighbor(corner, WfcDirection::XPos), Some(1));
    assert_eq!(grid_3d.neighbor(corner, WfcDirection::YPos), Some(2));
    assert_eq!(grid_3d.neighbor(corner, WfcDirection::ZPos), Some(4));
}

#[test]
fn representative_sizes_record_stats() {
    let medium_2d = WfcRequest::new(
        WfcGridSize::new_2d(32, 32),
        flexible_rules_2d(),
        WfcSeed(99),
    );
    let large_2d = WfcRequest::new(
        WfcGridSize::new_2d(64, 64),
        flexible_rules_2d(),
        WfcSeed(1234),
    );
    let medium_3d = WfcRequest::new(
        WfcGridSize::new_3d(16, 16, 16),
        stacked_rules_3d(),
        WfcSeed(100),
    );

    let a = solve_wfc(&medium_2d).expect("32x32 solve should succeed");
    let b = solve_wfc(&large_2d).expect("64x64 solve should succeed");
    let c = solve_wfc(&medium_3d).expect("16x16x16 solve should succeed");

    eprintln!(
        "wfc perf 32x32: {:.2}ms, 64x64: {:.2}ms, 16x16x16: {:.2}ms",
        a.stats.elapsed_ms, b.stats.elapsed_ms, c.stats.elapsed_ms
    );
    assert!(a.stats.elapsed_ms >= 0.0);
    assert!(b.stats.elapsed_ms >= 0.0);
    assert!(c.stats.elapsed_ms >= 0.0);
}
