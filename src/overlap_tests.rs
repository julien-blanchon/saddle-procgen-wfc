use super::*;

fn sample_patchwork() -> WfcTileGrid {
    let meadow = WfcTileId(0);
    let road = WfcTileId(1);
    let water = WfcTileId(2);
    let tiles = vec![
        meadow, road, road, meadow,
        meadow, road, water, water,
        meadow, meadow, water, meadow,
        road, road, meadow, meadow,
    ];
    WfcTileGrid {
        topology: WfcTopology::Cartesian2d,
        size: WfcGridSize::new_2d(4, 4),
        rotations: vec![0; tiles.len()],
        tiles,
    }
}

#[test]
fn overlap_solver_learns_patterns_from_a_sample() {
    let mut request =
        WfcOverlapRequest::new(sample_patchwork(), WfcGridSize::new_2d(8, 8), WfcSeed(77));
    request.options = WfcOverlapOptions {
        pattern_width: 2,
        pattern_height: 2,
        periodic_input: true,
        periodic_output: true,
    };

    let solution = solve_overlap_wfc_2d(&request).expect("overlap solve should succeed");
    assert_eq!(solution.grid.size, WfcGridSize::new_2d(8, 8));
    assert_eq!(solution.grid.tiles.len(), 64);
    assert!(solution
        .grid
        .tiles
        .iter()
        .all(|tile| request.sample.tiles.contains(tile)));

    let again = solve_overlap_wfc_2d(&request).expect("same seed should stay deterministic");
    assert_eq!(solution.signature, again.signature);
    assert_eq!(solution.grid.tiles, again.grid.tiles);
}

#[test]
fn overlap_solver_rejects_too_small_non_periodic_samples() {
    let mut request =
        WfcOverlapRequest::new(sample_patchwork(), WfcGridSize::new_2d(6, 6), WfcSeed(4));
    request.options = WfcOverlapOptions {
        pattern_width: 5,
        pattern_height: 2,
        periodic_input: false,
        periodic_output: false,
    };

    let failure = solve_overlap_wfc_2d(&request).expect_err("undersized sample should fail");
    assert_eq!(failure.reason, WfcFailureReason::InvalidRequest);
    assert!(failure.message.contains("pattern window"));
}
