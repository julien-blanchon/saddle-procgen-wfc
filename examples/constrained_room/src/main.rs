use saddle_procgen_wfc_example_support as common;

use bevy::prelude::*;
use common::{color_for_tile_2d, constrained_room_request, install_auto_exit, spatial_root};
use saddle_procgen_wfc::solve_wfc;

fn main() {
    let solution = solve_wfc(&constrained_room_request(19)).expect("room request should solve");

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.06)));
    app.insert_resource(solution);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc constrained_room".into(),
            resolution: (1280, 820).into(),
            ..default()
        }),
        ..default()
    }));
    install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, solution: Res<saddle_procgen_wfc::WfcSolution>) {
    let tile_size = 52.0;
    let origin = Vec2::new(
        -(solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands.spawn((Name::new("Room Camera"), Camera2d));
    commands.spawn((
        Name::new("Room Overlay"),
        Text::new(format!(
            "constrained_room\nforced entrances, border walls, and floor-count constraint\nsignature: {}",
            solution.signature
        )),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            ..default()
        },
    ));

    commands
        .spawn(spatial_root("Constrained Room Grid", Transform::default()))
        .with_children(|parent| {
            for y in 0..solution.grid.size.height {
                for x in 0..solution.grid.size.width {
                    let tile = solution
                        .grid
                        .tile_at(UVec3::new(x, y, 0))
                        .expect("tile should exist");
                    let color = match tile.0 {
                        2 => Color::srgb(0.16, 0.72, 0.62),
                        3 => Color::srgb(0.88, 0.42, 0.22),
                        _ => color_for_tile_2d(tile),
                    };
                    parent.spawn((
                        Sprite::from_color(color, Vec2::splat(tile_size - 3.0)),
                        Transform::from_xyz(
                            origin.x + x as f32 * tile_size,
                            origin.y + y as f32 * tile_size,
                            0.0,
                        ),
                    ));
                }
            }
        });
}
