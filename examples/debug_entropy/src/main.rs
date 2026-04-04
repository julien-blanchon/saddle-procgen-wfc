use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcBorder, WfcBorderConstraint, WfcDirection, WfcFailureReason, WfcFixedCell,
    WfcGlobalConstraint, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcSettings,
    WfcTileCountConstraint, WfcTileDefinition, WfcTileId, WfcTopology, solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Build a request that is designed to produce a contradiction, with debug
// snapshots enabled so we can visualize entropy at the moment of failure.
// ---------------------------------------------------------------------------

fn contradiction_request(seed: u64) -> WfcRequest {
    let wall = WfcTileId(0);
    let floor = WfcTileId(1);
    let entrance = WfcTileId(2);
    let exit = WfcTileId(3);
    let room_neighbors = [wall, floor, entrance, exit];
    let border_neighbors = [wall, entrance, exit];

    // Start from a constrained room setup ...
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

    // ... then enable debug snapshots and add an extra conflicting constraint
    // that forces MinX to be wall-only (conflicting with the pinned entrance).
    request.settings = WfcSettings {
        capture_debug_snapshot: true,
        ..default()
    };
    request
        .border_constraints
        .push(WfcBorderConstraint::new(WfcBorder::MinX, [wall]));

    request
}

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct EntropyConfig {
    seed: u64,
}

impl Default for EntropyConfig {
    fn default() -> Self {
        Self { seed: 41 }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Debug Entropy", position = "top-right")]
struct EntropyPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
}

impl Default for EntropyPane {
    fn default() -> Self {
        Self { seed: 41 }
    }
}

#[derive(Resource, Default)]
struct CurrentFailure(Option<saddle_procgen_wfc::WfcFailure>);

#[derive(Component)]
struct EntropyGridRoot;

#[derive(Component)]
struct EntropyOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.045, 0.045, 0.055)));
    app.init_resource::<EntropyConfig>();
    app.init_resource::<EntropyPane>();
    app.init_resource::<CurrentFailure>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc debug_entropy".into(),
            resolution: (1320, 860).into(),
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
    app.register_pane::<EntropyPane>();
    common::install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
            regenerate_failure,
            render_failure,
            update_overlay,
        )
            .chain(),
    );
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Entropy Camera"), Camera2d));
    commands.spawn((
        Name::new("Entropy Overlay"),
        EntropyOverlay,
        Text::new("debug_entropy"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(380),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.07, 0.06, 0.10, 0.84)),
    ));
}

fn sync_pane_to_config(pane: Res<EntropyPane>, mut config: ResMut<EntropyConfig>) {
    let next = EntropyConfig {
        seed: pane.seed as u64,
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_failure(config: Res<EntropyConfig>, mut failure: ResMut<CurrentFailure>) {
    if !config.is_changed() && failure.0.is_some() {
        return;
    }
    let next = solve_wfc(&contradiction_request(config.seed))
        .expect_err("debug view expects contradiction");
    assert_eq!(next.reason, WfcFailureReason::Contradiction);
    failure.0 = Some(next);
}

fn render_failure(
    mut commands: Commands,
    failure: Res<CurrentFailure>,
    roots: Query<Entity, With<EntropyGridRoot>>,
) {
    if !failure.is_changed() {
        return;
    }
    let Some(failure) = &failure.0 else {
        return;
    };

    for entity in &roots {
        commands.entity(entity).despawn();
    }

    let snapshot = failure
        .debug
        .as_ref()
        .expect("debug snapshot should be present");
    let tile_size = 52.0;
    let origin = Vec2::new(
        -(failure.grid_size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(failure.grid_size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            EntropyGridRoot,
            common::spatial_root("Entropy Grid", Transform::default()),
        ))
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

fn update_overlay(
    failure: Res<CurrentFailure>,
    mut overlay: Single<&mut Text, With<EntropyOverlay>>,
) {
    if !failure.is_changed() {
        return;
    }
    let Some(failure) = &failure.0 else {
        return;
    };
    **overlay = Text::new(format!(
        "debug_entropy\nreason: {:?}\n{}\ncontradiction: {:?}\nseed: {}",
        failure.reason,
        failure.message,
        failure.contradiction.as_ref().map(|item| item.position),
        failure.seed.0
    ));
}
