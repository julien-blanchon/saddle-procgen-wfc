use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcBorder, WfcBorderConstraint, WfcDirection, WfcFixedCell, WfcGlobalConstraint, WfcGridSize,
    WfcRequest, WfcRuleset, WfcSeed, WfcTileCountConstraint, WfcTileDefinition, WfcTileId,
    WfcTopology, solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Constrained room: fixed entrances, border walls, and floor-count constraint.
// ---------------------------------------------------------------------------

fn constrained_room_request(seed: u64) -> WfcRequest {
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
    // Wall and floor can neighbor anything inside the room.
    .with_rule(wall, WfcDirection::XPos, room_neighbors)
    .with_rule(wall, WfcDirection::XNeg, room_neighbors)
    .with_rule(wall, WfcDirection::YPos, room_neighbors)
    .with_rule(wall, WfcDirection::YNeg, room_neighbors)
    .with_rule(floor, WfcDirection::XPos, room_neighbors)
    .with_rule(floor, WfcDirection::XNeg, room_neighbors)
    .with_rule(floor, WfcDirection::YPos, room_neighbors)
    .with_rule(floor, WfcDirection::YNeg, room_neighbors)
    // Entrance faces inward (+X) and sits on the left border (-X = border only).
    .with_rule(entrance, WfcDirection::XPos, room_neighbors)
    .with_rule(entrance, WfcDirection::XNeg, border_neighbors)
    .with_rule(entrance, WfcDirection::YPos, room_neighbors)
    .with_rule(entrance, WfcDirection::YNeg, room_neighbors)
    // Exit faces inward (-X) and sits on the right border (+X = border only).
    .with_rule(exit, WfcDirection::XPos, border_neighbors)
    .with_rule(exit, WfcDirection::XNeg, room_neighbors)
    .with_rule(exit, WfcDirection::YPos, room_neighbors)
    .with_rule(exit, WfcDirection::YNeg, room_neighbors);

    let mut request = WfcRequest::new(WfcGridSize::new_2d(16, 10), ruleset, WfcSeed(seed));

    // Pin the entrance at the left wall and the exit at the right wall.
    request.fixed_cells = vec![
        WfcFixedCell::new(UVec3::new(0, 5, 0), entrance),
        WfcFixedCell::new(UVec3::new(15, 4, 0), exit),
    ];

    // Force borders to only contain walls (plus entrance/exit on their sides).
    request.border_constraints = vec![
        WfcBorderConstraint::new(WfcBorder::MinX, [wall, entrance]),
        WfcBorderConstraint::new(WfcBorder::MaxX, [wall, exit]),
        WfcBorderConstraint::new(WfcBorder::MinY, [wall]),
        WfcBorderConstraint::new(WfcBorder::MaxY, [wall]),
    ];

    // Guarantee at least 60 floor tiles so the room has usable interior space.
    request
        .global_constraints
        .push(WfcGlobalConstraint::TileCount(WfcTileCountConstraint {
            tile: floor,
            min_count: Some(60),
            max_count: None,
        }));
    // Exactly one entrance and one exit.
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

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct RoomConfig {
    seed: u64,
}

impl Default for RoomConfig {
    fn default() -> Self {
        Self { seed: 19 }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Constrained Room", position = "top-right")]
struct RoomPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
}

impl Default for RoomPane {
    fn default() -> Self {
        Self { seed: 19 }
    }
}

#[derive(Resource, Default)]
struct CurrentSolution(Option<saddle_procgen_wfc::WfcSolution>);

#[derive(Component)]
struct RoomGridRoot;

#[derive(Component)]
struct RoomOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.06)));
    app.init_resource::<RoomConfig>();
    app.init_resource::<RoomPane>();
    app.init_resource::<CurrentSolution>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc constrained_room".into(),
            resolution: (1280, 820).into(),
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
    app.register_pane::<RoomPane>();
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
    commands.spawn((Name::new("Room Camera"), Camera2d));
    commands.spawn((
        Name::new("Room Overlay"),
        RoomOverlay,
        Text::new("constrained_room"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(380),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.07, 0.08, 0.84)),
    ));
}

fn sync_pane_to_config(pane: Res<RoomPane>, mut config: ResMut<RoomConfig>) {
    let next = RoomConfig {
        seed: pane.seed as u64,
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_solution(config: Res<RoomConfig>, mut solution: ResMut<CurrentSolution>) {
    if !config.is_changed() && solution.0.is_some() {
        return;
    }
    solution.0 =
        Some(solve_wfc(&constrained_room_request(config.seed)).expect("room should solve"));
}

fn render_solution(
    mut commands: Commands,
    solution: Res<CurrentSolution>,
    roots: Query<Entity, With<RoomGridRoot>>,
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

    let tile_size = 52.0;
    let origin = Vec2::new(
        -(solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            RoomGridRoot,
            common::spatial_root("Constrained Room Grid", Transform::default()),
        ))
        .with_children(|parent| {
            for y in 0..solution.grid.size.height {
                for x in 0..solution.grid.size.width {
                    let tile = solution
                        .grid
                        .tile_at(UVec3::new(x, y, 0))
                        .expect("tile should exist");
                    let color = match tile.0 {
                        2 => Color::srgb(0.16, 0.72, 0.62), // entrance
                        3 => Color::srgb(0.88, 0.42, 0.22), // exit
                        _ => common::color_for_tile_2d(tile),
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

fn update_overlay(
    solution: Res<CurrentSolution>,
    mut overlay: Single<&mut Text, With<RoomOverlay>>,
) {
    if !solution.is_changed() {
        return;
    }
    let Some(solution) = &solution.0 else {
        return;
    };
    **overlay = Text::new(format!(
        "constrained_room\nforced entrances, border walls, and floor-count constraint\nsignature: {}\nseed: {}",
        solution.signature, solution.seed.0
    ));
}
