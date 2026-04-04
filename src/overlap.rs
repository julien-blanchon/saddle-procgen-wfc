use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::{
    WfcBoundaryStitching, WfcDirection, WfcFailure, WfcFailureReason, WfcGridSize, WfcRequest,
    WfcRuleset, WfcSeed, WfcSettings, WfcSolution, WfcTileDefinition, WfcTileGrid, WfcTileId,
    WfcTileVariant, WfcTopology,
    solver::solve_wfc,
};

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct WfcOverlapOptions {
    pub pattern_width: u32,
    pub pattern_height: u32,
    pub periodic_input: bool,
    pub periodic_output: bool,
}

impl Default for WfcOverlapOptions {
    fn default() -> Self {
        Self {
            pattern_width: 3,
            pattern_height: 3,
            periodic_input: true,
            periodic_output: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct WfcOverlapRequest {
    pub sample: WfcTileGrid,
    pub output_size: WfcGridSize,
    pub seed: WfcSeed,
    pub settings: WfcSettings,
    pub options: WfcOverlapOptions,
}

impl WfcOverlapRequest {
    pub fn new(sample: WfcTileGrid, output_size: WfcGridSize, seed: WfcSeed) -> Self {
        Self {
            sample,
            output_size,
            seed,
            settings: WfcSettings::default(),
            options: WfcOverlapOptions::default(),
        }
    }
}

#[derive(Clone, Debug)]
struct CompiledPattern {
    anchor: WfcTileVariant,
}

#[allow(clippy::result_large_err)]
pub fn solve_overlap_wfc_2d(request: &WfcOverlapRequest) -> Result<WfcSolution, WfcFailure> {
    validate_request(request)?;
    let compiled = compile_patterns(request)?;

    let mut inner_request =
        WfcRequest::new(request.output_size, compiled.ruleset, request.seed);
    inner_request.settings = request.settings.clone();
    inner_request.boundary_stitching = if request.options.periodic_output {
        WfcBoundaryStitching::xy()
    } else {
        WfcBoundaryStitching::default()
    };

    let inner_solution = solve_wfc(&inner_request)?;
    let mut grid = WfcTileGrid {
        topology: WfcTopology::Cartesian2d,
        size: request.output_size,
        tiles: Vec::with_capacity(inner_solution.grid.tiles.len()),
        rotations: Vec::with_capacity(inner_solution.grid.rotations.len()),
    };

    for (tile, rotation) in inner_solution
        .grid
        .tiles
        .iter()
        .zip(inner_solution.grid.rotations.iter())
    {
        let pattern_index = pattern_index(*tile, *rotation).ok_or_else(|| {
            invalid_failure(
                request,
                format!("overlap solver emitted an unknown pattern variant {tile:?}/{rotation}"),
            )
        })?;
        let pattern = compiled.patterns.get(pattern_index).ok_or_else(|| {
            invalid_failure(
                request,
                format!("overlap solver emitted a pattern index {pattern_index} outside the compiled set"),
            )
        })?;
        grid.tiles.push(pattern.anchor.tile);
        grid.rotations.push(pattern.anchor.rotation_steps);
    }

    let signature = grid.signature();
    Ok(WfcSolution {
        seed: request.seed,
        grid,
        stats: inner_solution.stats,
        debug: None,
        signature,
    })
}

struct CompiledOverlapModel {
    ruleset: WfcRuleset,
    patterns: Vec<CompiledPattern>,
}

#[allow(clippy::result_large_err)]
fn validate_request(request: &WfcOverlapRequest) -> Result<(), WfcFailure> {
    if request.sample.topology != WfcTopology::Cartesian2d {
        return Err(invalid_failure(
            request,
            "overlap solving currently supports only Cartesian2d samples".to_string(),
        ));
    }
    if request.sample.size.depth != 1 || request.output_size.depth != 1 {
        return Err(invalid_failure(
            request,
            "overlap solving requires 2D sample and output grids".to_string(),
        ));
    }
    if request.sample.tiles.len() != request.sample.size.total_cells()
        || request.sample.rotations.len() != request.sample.size.total_cells()
    {
        return Err(invalid_failure(
            request,
            "sample tile and rotation buffers must match the declared sample size".to_string(),
        ));
    }
    if request.options.pattern_width < 2 || request.options.pattern_height < 2 {
        return Err(invalid_failure(
            request,
            "overlap patterns must be at least 2x2".to_string(),
        ));
    }
    if !request.options.periodic_input
        && (request.sample.size.width < request.options.pattern_width
            || request.sample.size.height < request.options.pattern_height)
    {
        return Err(invalid_failure(
            request,
            "non-periodic overlap samples must be at least as large as the pattern window"
                .to_string(),
        ));
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn compile_patterns(request: &WfcOverlapRequest) -> Result<CompiledOverlapModel, WfcFailure> {
    let mut counts = BTreeMap::<Vec<(WfcTileId, u8)>, u32>::new();
    let pattern_width = request.options.pattern_width as i32;
    let pattern_height = request.options.pattern_height as i32;
    let max_x = if request.options.periodic_input {
        request.sample.size.width as i32
    } else {
        request.sample.size.width as i32 - pattern_width + 1
    };
    let max_y = if request.options.periodic_input {
        request.sample.size.height as i32
    } else {
        request.sample.size.height as i32 - pattern_height + 1
    };

    for y in 0..max_y {
        for x in 0..max_x {
            let pattern = sample_pattern(request, IVec2::new(x, y))?;
            *counts.entry(pattern).or_default() += 1;
        }
    }

    if counts.is_empty() {
        return Err(invalid_failure(
            request,
            "the overlap sampler did not discover any patterns".to_string(),
        ));
    }

    let directions = [
        WfcDirection::XPos,
        WfcDirection::XNeg,
        WfcDirection::YPos,
        WfcDirection::YNeg,
    ];
    let pattern_keys = counts.keys().cloned().collect::<Vec<_>>();
    let mut patterns = Vec::with_capacity(pattern_keys.len());
    let mut ruleset = WfcRuleset::new(WfcTopology::Cartesian2d, Vec::with_capacity(pattern_keys.len()));

    for (index, key) in pattern_keys.iter().enumerate() {
        let anchor = key
            .first()
            .map(|(tile, rotation)| WfcTileVariant::new(*tile, *rotation))
            .ok_or_else(|| invalid_failure(request, "empty pattern discovered".to_string()))?;
        patterns.push(CompiledPattern { anchor });
        ruleset.tiles.push(WfcTileDefinition::new(
            pattern_tile_id(index),
            counts[key] as f32,
            format!("Pattern {index}"),
        ));
    }

    for (source_index, source) in pattern_keys.iter().enumerate() {
        for &direction in &directions {
            let allowed = pattern_keys
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, candidate)| {
                    patterns_overlap(
                        source,
                        candidate,
                        request.options.pattern_width,
                        request.options.pattern_height,
                        direction,
                    )
                    .then_some(pattern_tile_id(candidate_index))
                })
                .collect::<Vec<_>>();
            ruleset.add_rule(pattern_tile_id(source_index), direction, allowed);
        }
    }

    Ok(CompiledOverlapModel { ruleset, patterns })
}

#[allow(clippy::result_large_err)]
fn sample_pattern(
    request: &WfcOverlapRequest,
    origin: IVec2,
) -> Result<Vec<(WfcTileId, u8)>, WfcFailure> {
    let mut pattern = Vec::with_capacity(
        (request.options.pattern_width * request.options.pattern_height) as usize,
    );
    for local_y in 0..request.options.pattern_height as i32 {
        for local_x in 0..request.options.pattern_width as i32 {
            let sample = sample_variant(request, origin + IVec2::new(local_x, local_y))
                .ok_or_else(|| {
                    invalid_failure(
                        request,
                        format!("pattern origin {origin:?} reads outside the non-periodic sample"),
                    )
                })?;
            pattern.push((sample.tile, sample.rotation_steps));
        }
    }
    Ok(pattern)
}

fn sample_variant(request: &WfcOverlapRequest, position: IVec2) -> Option<WfcTileVariant> {
    let sample_width = request.sample.size.width as i32;
    let sample_height = request.sample.size.height as i32;
    let wrapped = if request.options.periodic_input {
        IVec2::new(
            position.x.rem_euclid(sample_width),
            position.y.rem_euclid(sample_height),
        )
    } else {
        if position.x < 0
            || position.y < 0
            || position.x >= sample_width
            || position.y >= sample_height
        {
            return None;
        }
        position
    };

    request
        .sample
        .variant_at(UVec3::new(wrapped.x as u32, wrapped.y as u32, 0))
}

fn patterns_overlap(
    left: &[(WfcTileId, u8)],
    right: &[(WfcTileId, u8)],
    width: u32,
    height: u32,
    direction: WfcDirection,
) -> bool {
    let (shift_x, shift_y) = match direction {
        WfcDirection::XPos => (1i32, 0i32),
        WfcDirection::XNeg => (-1, 0),
        WfcDirection::YPos => (0, 1),
        WfcDirection::YNeg => (0, -1),
        _ => return false,
    };

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let right_x = x - shift_x;
            let right_y = y - shift_y;
            if !(0..width as i32).contains(&right_x) || !(0..height as i32).contains(&right_y) {
                continue;
            }

            if left[(y as usize * width as usize) + x as usize]
                != right[(right_y as usize * width as usize) + right_x as usize]
            {
                return false;
            }
        }
    }

    true
}

fn pattern_tile_id(index: usize) -> WfcTileId {
    WfcTileId(index as u16)
}

fn pattern_index(tile: WfcTileId, rotation: u8) -> Option<usize> {
    (rotation == 0).then_some(tile.0 as usize)
}

fn invalid_failure(request: &WfcOverlapRequest, message: String) -> WfcFailure {
    WfcFailure {
        reason: WfcFailureReason::InvalidRequest,
        seed: request.seed,
        topology: WfcTopology::Cartesian2d,
        grid_size: request.output_size,
        stats: default(),
        contradiction: None,
        debug: None,
        message,
    }
}
