use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcBorder, WfcBorderConstraint, WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed,
    WfcSolution, WfcTileDefinition, WfcTileId, WfcTileSymmetry, WfcTopology, solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Tile definitions with symmetry annotations. `Rotate2` means the solver will
// generate a 180-degree variant automatically; `Rotate4` gives all four
// 90-degree rotations. Adjacency rules are authored for rotation 0 only --
// the solver mirrors them to rotated variants.
// ---------------------------------------------------------------------------

fn autorotation_request(seed: u64, width: u32, height: u32) -> WfcRequest {
    let meadow = WfcTileId(0);
    let straight = WfcTileId(1);
    let corner = WfcTileId(2);
    let water = WfcTileId(3);

    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(meadow, 4.0, "Meadow"),
            // A straight road segment -- authored facing north/south.
            // Rotate2 generates an east/west variant.
            WfcTileDefinition::new(straight, 1.2, "Road Straight")
                .with_symmetry(WfcTileSymmetry::Rotate2),
            // A corner road segment -- authored facing NE.
            // Rotate4 generates NW, SW, SE variants.
            WfcTileDefinition::new(corner, 0.8, "Road Corner")
                .with_symmetry(WfcTileSymmetry::Rotate4),
            WfcTileDefinition::new(water, 0.9, "Water"),
        ],
    )
    // Meadow can neighbor anything.
    .with_rule(meadow, WfcDirection::XPos, [meadow, straight, corner, water])
    .with_rule(meadow, WfcDirection::XNeg, [meadow, straight, corner, water])
    .with_rule(meadow, WfcDirection::YPos, [meadow, straight, corner, water])
    .with_rule(meadow, WfcDirection::YNeg, [meadow, straight, corner, water])
    // Straight (north/south) has road sides on Y, open sides on X.
    .with_rule(straight, WfcDirection::XPos, [meadow, water])
    .with_rule(straight, WfcDirection::XNeg, [meadow, water])
    .with_rule(straight, WfcDirection::YPos, [meadow, straight, corner])
    .with_rule(straight, WfcDirection::YNeg, [meadow, straight, corner])
    // Corner (NE) has road on +X and +Y, open on -X and -Y.
    .with_rule(corner, WfcDirection::XPos, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::XNeg, [meadow, water])
    .with_rule(corner, WfcDirection::YPos, [meadow, straight, corner])
    .with_rule(corner, WfcDirection::YNeg, [meadow, water])
    // Water clusters (no road adjacency).
    .with_rule(water, WfcDirection::XPos, [meadow, water])
    .with_rule(water, WfcDirection::XNeg, [meadow, water])
    .with_rule(water, WfcDirection::YPos, [meadow, water])
    .with_rule(water, WfcDirection::YNeg, [meadow, water]);

    let mut request = WfcRequest::new(WfcGridSize::new_2d(width, height), ruleset, WfcSeed(seed));
    // Keep borders road-free.
    request.border_constraints = vec![
        WfcBorderConstraint::new(WfcBorder::MinX, [meadow, water]),
        WfcBorderConstraint::new(WfcBorder::MaxX, [meadow, water]),
        WfcBorderConstraint::new(WfcBorder::MinY, [meadow, water]),
        WfcBorderConstraint::new(WfcBorder::MaxY, [meadow, water]),
    ];
    request
}

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct AutoRotationConfig {
    seed: u64,
    width: u32,
    height: u32,
}

impl Default for AutoRotationConfig {
    fn default() -> Self {
        Self {
            seed: 11,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Auto Rotation", position = "top-right")]
struct AutoRotationPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 28.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 20.0, step = 1.0)]
    height: u32,
}

impl Default for AutoRotationPane {
    fn default() -> Self {
        Self {
            seed: 11,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource, Default)]
struct CurrentSolution(Option<WfcSolution>);

#[derive(Component)]
struct AutoRotationRoot;

#[derive(Component)]
struct AutoRotationOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.09, 0.08)));
    app.init_resource::<AutoRotationConfig>();
    app.init_resource::<AutoRotationPane>();
    app.init_resource::<CurrentSolution>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc auto_rotation".into(),
            resolution: (1380, 900).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins((
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        PanePlugin,
    ));
    app.register_pane::<AutoRotationPane>();
    common::install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
            regenerate_solution,
            render_solution,
            update_overlay,
        )
            .chain(),
    );
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Auto Rotation Camera"), Camera2d));
    commands.spawn((
        Name::new("Auto Rotation Overlay"),
        AutoRotationOverlay,
        Text::new("auto_rotation"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(360),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.10, 0.09, 0.86)),
    ));
}

fn sync_pane_to_config(pane: Res<AutoRotationPane>, mut config: ResMut<AutoRotationConfig>) {
    let next = AutoRotationConfig {
        seed: pane.seed as u64,
        width: pane.width.max(4),
        height: pane.height.max(4),
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_solution(config: Res<AutoRotationConfig>, mut solution: ResMut<CurrentSolution>) {
    if !config.is_changed() && solution.0.is_some() {
        return;
    }

    let request = autorotation_request(config.seed, config.width, config.height);
    solution.0 = Some(solve_wfc(&request).expect("auto-rotation request should solve"));
}

fn render_solution(
    mut commands: Commands,
    solution: Res<CurrentSolution>,
    roots: Query<Entity, With<AutoRotationRoot>>,
) {
    if !solution.is_changed() {
        return;
    }
    let Some(solution) = &solution.0 else {
        return;
    };

    for entity in &roots {
        commands.entity(entity).despawn();
    }

    let tile_size = 54.0;
    let origin = Vec2::new(
        -(solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            AutoRotationRoot,
            common::spatial_root("Auto Rotation Root", Transform::default()),
        ))
        .with_children(|parent| {
            for y in 0..solution.grid.size.height {
                for x in 0..solution.grid.size.width {
                    let position = UVec3::new(x, y, 0);
                    let tile = solution.grid.tile_at(position).expect("tile should exist");
                    let rotation_steps = solution
                        .grid
                        .rotation_at(position)
                        .expect("rotation should exist");

                    parent
                        .spawn((
                            Name::new(format!("Cell ({x}, {y})")),
                            Transform::from_xyz(
                                origin.x + x as f32 * tile_size,
                                origin.y + y as f32 * tile_size,
                                0.0,
                            ),
                            GlobalTransform::default(),
                        ))
                        .with_children(|cell| {
                            cell.spawn((
                                Sprite::from_color(ground_color(tile), Vec2::splat(tile_size - 2.0)),
                                Transform::from_xyz(0.0, 0.0, 0.0),
                            ));
                            match tile.0 {
                                1 => {
                                    let rotation =
                                        Quat::from_rotation_z(rotation_steps as f32 * FRAC_PI_2);
                                    cell.spawn((
                                        Transform::from_rotation(rotation),
                                        GlobalTransform::default(),
                                    ))
                                    .with_children(|road| {
                                        road.spawn((
                                            Sprite::from_color(
                                                Color::srgb(0.77, 0.69, 0.49),
                                                Vec2::new(tile_size * 0.34, tile_size + 4.0),
                                            ),
                                            Transform::from_xyz(0.0, 0.0, 1.0),
                                        ));
                                        road.spawn((
                                            Sprite::from_color(
                                                Color::srgb(0.95, 0.91, 0.78),
                                                Vec2::new(tile_size * 0.04, tile_size * 0.7),
                                            ),
                                            Transform::from_xyz(0.0, 0.0, 2.0),
                                        ));
                                    });
                                }
                                2 => {
                                    let rotation =
                                        Quat::from_rotation_z(rotation_steps as f32 * FRAC_PI_2);
                                    cell.spawn((
                                        Transform::from_rotation(rotation),
                                        GlobalTransform::default(),
                                    ))
                                    .with_children(|road| {
                                        road.spawn((
                                            Sprite::from_color(
                                                Color::srgb(0.77, 0.69, 0.49),
                                                Vec2::new(tile_size * 0.34, tile_size * 0.72),
                                            ),
                                            Transform::from_xyz(0.0, tile_size * 0.14, 1.0),
                                        ));
                                        road.spawn((
                                            Sprite::from_color(
                                                Color::srgb(0.77, 0.69, 0.49),
                                                Vec2::new(tile_size * 0.72, tile_size * 0.34),
                                            ),
                                            Transform::from_xyz(tile_size * 0.14, 0.0, 1.0),
                                        ));
                                        road.spawn((
                                            Sprite::from_color(
                                                Color::srgb(0.95, 0.91, 0.78),
                                                Vec2::new(tile_size * 0.04, tile_size * 0.42),
                                            ),
                                            Transform::from_xyz(0.0, tile_size * 0.14, 2.0),
                                        ));
                                        road.spawn((
                                            Sprite::from_color(
                                                Color::srgb(0.95, 0.91, 0.78),
                                                Vec2::new(tile_size * 0.42, tile_size * 0.04),
                                            ),
                                            Transform::from_xyz(tile_size * 0.14, 0.0, 2.0),
                                        ));
                                    });
                                }
                                3 => {
                                    cell.spawn((
                                        Sprite::from_color(
                                            Color::srgba(0.86, 0.95, 1.0, 0.16),
                                            Vec2::splat(tile_size * 0.74),
                                        ),
                                        Transform::from_xyz(0.0, 0.0, 1.0),
                                    ));
                                }
                                _ => {}
                            }
                        });
                }
            }
        });
}

fn update_overlay(
    solution: Res<CurrentSolution>,
    mut overlay: Single<&mut Text, With<AutoRotationOverlay>>,
) {
    if !solution.is_changed() {
        return;
    }
    let Some(solution) = &solution.0 else {
        return;
    };
    **overlay = Text::new(format!(
        "auto_rotation\nsignature: {}\nseed: {}\nsize: {}x{}\nstraight and corner roads are authored once and rotated by the solver",
        solution.signature,
        solution.seed.0,
        solution.grid.size.width,
        solution.grid.size.height
    ));
}

fn ground_color(tile: WfcTileId) -> Color {
    match tile.0 {
        3 => Color::srgb(0.16, 0.41, 0.66),
        _ => Color::srgb(0.24, 0.45, 0.23),
    }
}
