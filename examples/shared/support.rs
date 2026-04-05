#![allow(dead_code)]

use bevy::prelude::*;
use saddle_procgen_wfc::{
    WfcBorder, WfcBorderConstraint, WfcBoundaryStitching, WfcDirection, WfcFixedCell,
    WfcGlobalConstraint, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcSettings,
    WfcTileCountConstraint, WfcTileDefinition, WfcTileId, WfcTileSymmetry, WfcTopology,
};
use std::borrow::Cow;
use std::time::Duration;

#[derive(Resource)]
pub struct ExampleExitTimer(pub Timer);

pub fn install_auto_exit(app: &mut App) {
    if let Some(seconds) = std::env::var("WFC_EXAMPLE_EXIT_AFTER_SECONDS")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
    {
        let seconds = seconds.max(0.05);
        app.insert_resource(ExampleExitTimer(Timer::from_seconds(
            seconds,
            TimerMode::Once,
        )));
        app.add_systems(Update, exit_after_seconds);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f32(seconds + 1.0));
            std::process::exit(0);
        });
    }
}

fn exit_after_seconds(
    time: Res<Time>,
    timer: Option<ResMut<ExampleExitTimer>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut timer) = timer else {
        return;
    };
    if timer.0.tick(time.delta()).just_finished() {
        exit.write(AppExit::Success);
    }
}

pub fn spatial_root(name: impl Into<Cow<'static, str>>, transform: Transform) -> impl Bundle {
    (Name::new(name), transform, Visibility::Visible)
}

pub fn basic_request(seed: u64) -> WfcRequest {
    let meadow = WfcTileId(0);
    let road = WfcTileId(1);
    let water = WfcTileId(2);
    let all = [meadow, road, water];
    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(meadow, 5.0, "Meadow"),
            WfcTileDefinition::new(road, 2.0, "Road"),
            WfcTileDefinition::new(water, 1.0, "Water"),
        ],
    )
    .with_rule(meadow, WfcDirection::XPos, all)
    .with_rule(meadow, WfcDirection::XNeg, all)
    .with_rule(meadow, WfcDirection::YPos, all)
    .with_rule(meadow, WfcDirection::YNeg, all)
    .with_rule(road, WfcDirection::XPos, [meadow, road])
    .with_rule(road, WfcDirection::XNeg, [meadow, road])
    .with_rule(road, WfcDirection::YPos, [meadow, road])
    .with_rule(road, WfcDirection::YNeg, [meadow, road])
    .with_rule(water, WfcDirection::XPos, [meadow, water])
    .with_rule(water, WfcDirection::XNeg, [meadow, water])
    .with_rule(water, WfcDirection::YPos, [meadow, water])
    .with_rule(water, WfcDirection::YNeg, [meadow, water]);

    WfcRequest::new(WfcGridSize::new_2d(18, 12), ruleset, WfcSeed(seed))
}

pub fn constrained_room_request(seed: u64) -> WfcRequest {
    let wall = WfcTileId(0);
    let floor = WfcTileId(1);
    let entrance = WfcTileId(2);
    let exit = WfcTileId(3);
    let room_neighbors = [wall, floor, entrance, exit];
    let border_neighbors = [wall, entrance, exit];

    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(wall, 1.0, "Wall"),
            WfcTileDefinition::new(floor, 6.0, "Floor"),
            WfcTileDefinition::new(entrance, 1.0, "Entrance"),
            WfcTileDefinition::new(exit, 1.0, "Exit"),
        ],
    )
    .with_rule(wall, WfcDirection::XPos, room_neighbors)
    .with_rule(wall, WfcDirection::XNeg, room_neighbors)
    .with_rule(wall, WfcDirection::YPos, room_neighbors)
    .with_rule(wall, WfcDirection::YNeg, room_neighbors)
    .with_rule(floor, WfcDirection::XPos, room_neighbors)
    .with_rule(floor, WfcDirection::XNeg, room_neighbors)
    .with_rule(floor, WfcDirection::YPos, room_neighbors)
    .with_rule(floor, WfcDirection::YNeg, room_neighbors)
    .with_rule(entrance, WfcDirection::XPos, room_neighbors)
    .with_rule(entrance, WfcDirection::XNeg, border_neighbors)
    .with_rule(entrance, WfcDirection::YPos, room_neighbors)
    .with_rule(entrance, WfcDirection::YNeg, room_neighbors)
    .with_rule(exit, WfcDirection::XPos, border_neighbors)
    .with_rule(exit, WfcDirection::XNeg, room_neighbors)
    .with_rule(exit, WfcDirection::YPos, room_neighbors)
    .with_rule(exit, WfcDirection::YNeg, room_neighbors);

    let mut request = WfcRequest::new(WfcGridSize::new_2d(16, 10), ruleset, WfcSeed(seed));
    request.fixed_cells = vec![
        WfcFixedCell::new(UVec3::new(0, 5, 0), entrance),
        WfcFixedCell::new(UVec3::new(15, 4, 0), exit),
    ];
    request.border_constraints = vec![
        WfcBorderConstraint::new(WfcBorder::MinX, [wall, entrance]),
        WfcBorderConstraint::new(WfcBorder::MaxX, [wall, exit]),
        WfcBorderConstraint::new(WfcBorder::MinY, [wall]),
        WfcBorderConstraint::new(WfcBorder::MaxY, [wall]),
    ];
    request
        .global_constraints
        .push(WfcGlobalConstraint::TileCount(WfcTileCountConstraint {
            tile: floor,
            min_count: Some(60),
            max_count: None,
        }));
    request
        .global_constraints
        .push(WfcGlobalConstraint::TileCount(WfcTileCountConstraint {
            tile: entrance,
            min_count: Some(1),
            max_count: Some(1),
        }));
    request
        .global_constraints
        .push(WfcGlobalConstraint::TileCount(WfcTileCountConstraint {
            tile: exit,
            min_count: Some(1),
            max_count: Some(1),
        }));
    request
}

pub fn autorotation_request(seed: u64, width: u32, height: u32) -> WfcRequest {
    let meadow = WfcTileId(0);
    let straight = WfcTileId(1);
    let corner = WfcTileId(2);
    let water = WfcTileId(3);
    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(meadow, 4.0, "Meadow"),
            WfcTileDefinition::new(straight, 1.2, "Road Straight")
                .with_symmetry(WfcTileSymmetry::Rotate2),
            WfcTileDefinition::new(corner, 0.8, "Road Corner")
                .with_symmetry(WfcTileSymmetry::Rotate4),
            WfcTileDefinition::new(water, 0.9, "Water"),
        ],
    )
    .with_rule(
        meadow,
        WfcDirection::XPos,
        [meadow, straight, corner, water],
    )
    .with_rule(
        meadow,
        WfcDirection::XNeg,
        [meadow, straight, corner, water],
    )
    .with_rule(
        meadow,
        WfcDirection::YPos,
        [meadow, straight, corner, water],
    )
    .with_rule(
        meadow,
        WfcDirection::YNeg,
        [meadow, straight, corner, water],
    )
    .with_rule(straight, WfcDirection::XPos, [meadow, water])
    .with_rule(straight, WfcDirection::XNeg, [meadow, water])
    .with_rule(straight, WfcDirection::YPos, [meadow, straight, corner])
    .with_rule(straight, WfcDirection::YNeg, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::XPos, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::XNeg, [meadow, water])
    .with_rule(corner, WfcDirection::YPos, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::YNeg, [meadow, water])
    .with_rule(water, WfcDirection::XPos, [meadow, water])
    .with_rule(water, WfcDirection::XNeg, [meadow, water])
    .with_rule(water, WfcDirection::YPos, [meadow, water])
    .with_rule(water, WfcDirection::YNeg, [meadow, water]);

    let mut request = WfcRequest::new(WfcGridSize::new_2d(width, height), ruleset, WfcSeed(seed));
    request.border_constraints = vec![
        WfcBorderConstraint::new(WfcBorder::MinX, [meadow, water]),
        WfcBorderConstraint::new(WfcBorder::MaxX, [meadow, water]),
        WfcBorderConstraint::new(WfcBorder::MinY, [meadow, water]),
        WfcBorderConstraint::new(WfcBorder::MaxY, [meadow, water]),
    ];
    request
}

pub fn voxel_request(seed: u64) -> WfcRequest {
    let air = WfcTileId(0);
    let stone = WfcTileId(1);
    let cap = WfcTileId(2);
    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian3d,
        vec![
            WfcTileDefinition::new(air, 3.0, "Air"),
            WfcTileDefinition::new(stone, 2.0, "Stone"),
            WfcTileDefinition::new(cap, 1.0, "Cap"),
        ],
    )
    .with_rule(air, WfcDirection::XPos, [air, stone, cap])
    .with_rule(air, WfcDirection::XNeg, [air, stone, cap])
    .with_rule(air, WfcDirection::YPos, [air, stone, cap])
    .with_rule(air, WfcDirection::YNeg, [air, stone, cap])
    .with_rule(air, WfcDirection::ZPos, [air, cap])
    .with_rule(air, WfcDirection::ZNeg, [air, stone, cap])
    .with_rule(stone, WfcDirection::XPos, [stone, air])
    .with_rule(stone, WfcDirection::XNeg, [stone, air])
    .with_rule(stone, WfcDirection::YPos, [stone, air])
    .with_rule(stone, WfcDirection::YNeg, [stone, air])
    .with_rule(stone, WfcDirection::ZPos, [stone, cap])
    .with_rule(stone, WfcDirection::ZNeg, [stone])
    .with_rule(cap, WfcDirection::XPos, [cap, air])
    .with_rule(cap, WfcDirection::XNeg, [cap, air])
    .with_rule(cap, WfcDirection::YPos, [cap, air])
    .with_rule(cap, WfcDirection::YNeg, [cap, air])
    .with_rule(cap, WfcDirection::ZPos, [air])
    .with_rule(cap, WfcDirection::ZNeg, [stone]);
    WfcRequest::new(WfcGridSize::new_3d(10, 10, 6), ruleset, WfcSeed(seed))
}

pub fn large_request(seed: u64) -> WfcRequest {
    let mut request = basic_request(seed);
    request.grid_size = WfcGridSize::new_2d(64, 48);
    request.settings.max_backtracks = 1_024;
    request
}

pub fn contradiction_request(seed: u64) -> WfcRequest {
    let mut request = constrained_room_request(seed);
    request.settings = WfcSettings {
        capture_debug_snapshot: true,
        ..default()
    };
    request
        .border_constraints
        .push(WfcBorderConstraint::new(WfcBorder::MinX, [WfcTileId(0)]));
    request
}

pub fn stitched_request(seed: u64) -> WfcRequest {
    let white = WfcTileId(0);
    let black = WfcTileId(1);
    let ruleset = WfcRuleset::new(
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
    .with_rule(black, WfcDirection::YNeg, [white]);

    let mut request = WfcRequest::new(WfcGridSize::new_2d(6, 4), ruleset, WfcSeed(seed));
    request.boundary_stitching = WfcBoundaryStitching::xy();
    request.fixed_cells = vec![WfcFixedCell::new(UVec3::new(0, 0, 0), white)];
    request
}

pub fn hex_request(seed: u64) -> WfcRequest {
    let grass = WfcTileId(0);
    let grove = WfcTileId(1);
    let water = WfcTileId(2);
    let ruleset = WfcRuleset::new(
        WfcTopology::Hex2d,
        vec![
            WfcTileDefinition::new(grass, 4.0, "Grass"),
            WfcTileDefinition::new(grove, 2.0, "Grove"),
            WfcTileDefinition::new(water, 1.0, "Water"),
        ],
    )
    .with_rule(grass, WfcDirection::HexEast, [grass, grove, water])
    .with_rule(grass, WfcDirection::HexWest, [grass, grove, water])
    .with_rule(grass, WfcDirection::HexNorthEast, [grass, grove, water])
    .with_rule(grass, WfcDirection::HexNorthWest, [grass, grove, water])
    .with_rule(grass, WfcDirection::HexSouthEast, [grass, grove, water])
    .with_rule(grass, WfcDirection::HexSouthWest, [grass, grove, water])
    .with_rule(grove, WfcDirection::HexEast, [grass, grove])
    .with_rule(grove, WfcDirection::HexWest, [grass, grove])
    .with_rule(grove, WfcDirection::HexNorthEast, [grass, grove])
    .with_rule(grove, WfcDirection::HexNorthWest, [grass, grove])
    .with_rule(grove, WfcDirection::HexSouthEast, [grass, grove])
    .with_rule(grove, WfcDirection::HexSouthWest, [grass, grove])
    .with_rule(water, WfcDirection::HexEast, [grass, water])
    .with_rule(water, WfcDirection::HexWest, [grass, water])
    .with_rule(water, WfcDirection::HexNorthEast, [grass, water])
    .with_rule(water, WfcDirection::HexNorthWest, [grass, water])
    .with_rule(water, WfcDirection::HexSouthEast, [grass, water])
    .with_rule(water, WfcDirection::HexSouthWest, [grass, water]);

    WfcRequest::new(WfcGridSize::new_2d(10, 8), ruleset, WfcSeed(seed))
}

pub fn color_for_tile_2d(tile: WfcTileId) -> Color {
    match tile.0 {
        0 => Color::srgb(0.24, 0.52, 0.26),
        1 => Color::srgb(0.66, 0.56, 0.34),
        2 => Color::srgb(0.17, 0.38, 0.72),
        3 => Color::srgb(0.88, 0.42, 0.22),
        _ => Color::srgb(0.7, 0.7, 0.7),
    }
}

pub fn color_for_tile_3d(tile: WfcTileId) -> Color {
    match tile.0 {
        0 => Color::srgba(0.0, 0.0, 0.0, 0.0),
        1 => Color::srgb(0.42, 0.45, 0.48),
        2 => Color::srgb(0.78, 0.68, 0.42),
        _ => Color::srgb(0.8, 0.2, 0.8),
    }
}
