use saddle_procgen_wfc_example_support as common;

use bevy::prelude::*;
use common::{basic_request, color_for_tile_2d, install_auto_exit, spatial_root};
use saddle_procgen_wfc::{GenerateWfc, WfcPlugin, WfcSolved, WfcSystems};

#[derive(Resource, Default)]
struct SolveText(String);

#[derive(Component)]
struct AsyncGridRoot;

#[derive(Component)]
struct AsyncOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.055, 0.06, 0.08)));
    app.init_resource::<SolveText>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc async_runtime".into(),
            resolution: (1280, 840).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(WfcPlugin::default());
    install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            request_generation.before(WfcSystems::Request),
            apply_solution.after(WfcSystems::ApplyResults),
            update_overlay.after(WfcSystems::ApplyResults),
        ),
    );
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Async Runtime Camera"), Camera2d));
    commands.spawn((
        Name::new("Async Overlay"),
        AsyncOverlay,
        Text::new("async_runtime\nwaiting for result"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            ..default()
        },
    ));
}

fn request_generation(mut writer: Local<bool>, mut requests: MessageWriter<GenerateWfc>) {
    if *writer {
        return;
    }
    *writer = true;
    requests.write(GenerateWfc {
        request: basic_request(77),
        label: Some("async basic".into()),
    });
}

fn apply_solution(
    mut commands: Commands,
    mut results: MessageReader<WfcSolved>,
    roots: Query<Entity, With<AsyncGridRoot>>,
    mut summary: ResMut<SolveText>,
) {
    for solved in results.read() {
        for entity in &roots {
            commands.entity(entity).despawn();
        }

        let tile_size = 42.0;
        let origin = Vec2::new(
            -(solved.solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
            -(solved.solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
        );
        summary.0 = format!(
            "async_runtime\njob: {}\nsignature: {}\nobs/backtracks: {}/{}",
            solved.label,
            solved.solution.signature,
            solved.solution.stats.observation_count,
            solved.solution.stats.backtrack_count
        );

        commands
            .spawn((
                AsyncGridRoot,
                spatial_root("Async Grid Root", Transform::default()),
            ))
            .with_children(|parent| {
                for y in 0..solved.solution.grid.size.height {
                    for x in 0..solved.solution.grid.size.width {
                        let tile = solved
                            .solution
                            .grid
                            .tile_at(UVec3::new(x, y, 0))
                            .expect("tile should exist");
                        parent.spawn((
                            Sprite::from_color(
                                color_for_tile_2d(tile),
                                Vec2::splat(tile_size - 2.0),
                            ),
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
}

fn update_overlay(summary: Res<SolveText>, mut overlay: Single<&mut Text, With<AsyncOverlay>>) {
    if summary.is_changed() {
        **overlay = Text::new(summary.0.clone());
    }
}
