use saddle_procgen_wfc_example_support as common;

use bevy::prelude::*;
use common::{contradiction_request, install_auto_exit, spatial_root};
use saddle_procgen_wfc::{WfcFailureReason, solve_wfc};

fn main() {
    let failure = solve_wfc(&contradiction_request(41)).expect_err("debug view expects failure");
    assert_eq!(failure.reason, WfcFailureReason::Contradiction);

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.045, 0.045, 0.055)));
    app.insert_resource(failure);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc debug_entropy".into(),
            resolution: (1320, 860).into(),
            ..default()
        }),
        ..default()
    }));
    install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, failure: Res<saddle_procgen_wfc::WfcFailure>) {
    commands.spawn((Name::new("Entropy Camera"), Camera2d));
    let snapshot = failure
        .debug
        .as_ref()
        .expect("debug snapshot should be captured for contradiction request");
    let tile_size = 52.0;
    let origin = Vec2::new(
        -(failure.grid_size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(failure.grid_size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands.spawn((
        Name::new("Entropy Overlay"),
        Text::new(format!(
            "debug_entropy\nreason: {:?}\n{}\ncontradiction: {:?}",
            failure.reason,
            failure.message,
            failure.contradiction.as_ref().map(|item| item.position)
        )),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            ..default()
        },
    ));

    commands
        .spawn(spatial_root("Entropy Grid", Transform::default()))
        .with_children(|parent| {
            for cell in &snapshot.cells {
                let possible = cell.possible_count.max(1) as f32;
                let intensity = (possible / 4.0).clamp(0.0, 1.0);
                let color = if cell.possible_count == 0 {
                    Color::srgb(0.92, 0.18, 0.26)
                } else {
                    Color::srgb(0.18 + intensity * 0.5, 0.22, 0.68 - intensity * 0.3)
                };
                parent.spawn((
                    Sprite::from_color(color, Vec2::splat(tile_size - 3.0)),
                    Transform::from_xyz(
                        origin.x + cell.position.x as f32 * tile_size,
                        origin.y + cell.position.y as f32 * tile_size,
                        0.0,
                    ),
                ));
                parent.spawn((
                    Text2d::new(format!("{}", cell.possible_count)),
                    TextFont::from_font_size(18.0),
                    TextColor(Color::WHITE),
                    Transform::from_xyz(
                        origin.x + cell.position.x as f32 * tile_size,
                        origin.y + cell.position.y as f32 * tile_size,
                        1.0,
                    ),
                ));
            }
        });
}
