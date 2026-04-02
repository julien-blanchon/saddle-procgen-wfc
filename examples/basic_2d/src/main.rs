use saddle_procgen_wfc_example_support as common;

use bevy::prelude::*;
use common::{basic_request, color_for_tile_2d, install_auto_exit, spatial_root};
use saddle_procgen_wfc::solve_wfc;

fn main() {
    let solution = solve_wfc(&basic_request(7)).expect("basic request should solve");
    eprintln!(
        "[basic_2d] signature={} observations={} backtracks={} elapsed_ms={:.2}",
        solution.signature,
        solution.stats.observation_count,
        solution.stats.backtrack_count,
        solution.stats.elapsed_ms
    );

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)));
    app.insert_resource(solution);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc basic_2d".into(),
            resolution: (1280, 840).into(),
            ..default()
        }),
        ..default()
    }));
    install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, solution: Res<saddle_procgen_wfc::WfcSolution>) {
    let tile_size = 42.0;
    let origin = Vec2::new(
        -(solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands.spawn((Name::new("Basic 2D Camera"), Camera2d));
    commands.spawn((
        Name::new("Basic 2D Overlay"),
        Text::new(format!(
            "basic_2d\nsignature: {}\nobs/backtracks: {}/{}\nelapsed: {:.2}ms",
            solution.signature,
            solution.stats.observation_count,
            solution.stats.backtrack_count,
            solution.stats.elapsed_ms
        )),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            ..default()
        },
    ));

    commands
        .spawn(spatial_root("Basic 2D Grid", Transform::default()))
        .with_children(|parent| {
            for y in 0..solution.grid.size.height {
                for x in 0..solution.grid.size.width {
                    let tile = solution
                        .grid
                        .tile_at(UVec3::new(x, y, 0))
                        .expect("tile should exist");
                    parent.spawn((
                        Sprite::from_color(color_for_tile_2d(tile), Vec2::splat(tile_size - 2.0)),
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
