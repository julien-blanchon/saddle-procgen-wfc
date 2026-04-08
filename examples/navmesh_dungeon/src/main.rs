#[cfg(feature = "e2e")]
mod e2e;

use bevy::prelude::*;
use saddle_ai_navmesh::{
    NavmeshAgent, NavmeshBakeSettings, NavmeshFollowTarget, NavmeshPlugin, NavmeshPrimitive,
    NavmeshPrimitiveSource, NavmeshSource, NavmeshSourceKind, NavmeshSteeringOutput,
    NavmeshSurface,
};
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcBorder, WfcBorderConstraint, WfcDirection, WfcFixedCell, WfcGlobalConstraint, WfcGridSize,
    WfcRequest, WfcRuleset, WfcSeed, WfcTileCountConstraint, WfcTileDefinition, WfcTileId,
    WfcTopology, solve_wfc,
};

const TILE_SIZE: f32 = 2.0;

const WALL: u16 = 0;
const FLOOR: u16 = 1;
const ENTRANCE: u16 = 2;
const EXIT: u16 = 3;

#[derive(Resource, Clone, PartialEq)]
struct DungeonConfig {
    seed: u64,
    width: u32,
    height: u32,
}

impl Default for DungeonConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 20,
            height: 14,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Navmesh Dungeon", position = "top-right")]
struct DungeonPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 32.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 24.0, step = 1.0)]
    height: u32,
}

impl Default for DungeonPane {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 20,
            height: 14,
        }
    }
}

#[derive(Component)]
struct DungeonVisualRoot;

#[derive(Component)]
struct NavmeshFloorSource;

#[derive(Component)]
struct WallMesh;

#[derive(Component)]
struct AgentMarker;

#[derive(Resource, Default)]
struct DungeonState {
    surface_entity: Option<Entity>,
    agent_entity: Option<Entity>,
    built: bool,
    entrance_pos: Vec3,
    exit_pos: Vec3,
}

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.1)));
    app.init_resource::<DungeonConfig>();
    app.init_resource::<DungeonPane>();
    app.init_resource::<DungeonState>();
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "wfc navmesh_dungeon".into(),
                resolution: (1500, 920).into(),
                ..default()
            }),
            ..default()
        }),
    );
    app.add_plugins((
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        PanePlugin,
        NavmeshPlugin::default(),
    ));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::NavmeshDungeonE2EPlugin);
    app.register_pane::<DungeonPane>();
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
            rebuild_dungeon,
            apply_steering,
            update_overlay,
        )
            .chain(),
    );
    app.run();
}

#[derive(Component)]
struct OverlayText;

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Main Camera"),
        Camera3d::default(),
        Transform::from_xyz(20.0, 35.0, 30.0).looking_at(Vec3::new(20.0, 0.0, 14.0), Vec3::Y),
    ));

    commands.spawn((
        Name::new("Directional Light"),
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Name::new("Overlay"),
        OverlayText,
        Text::new("wfc navmesh_dungeon\nGenerating..."),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            width: px(360),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.88)),
    ));
}

fn sync_pane_to_config(pane: Res<DungeonPane>, mut config: ResMut<DungeonConfig>) {
    let next = DungeonConfig {
        seed: pane.seed as u64,
        width: pane.width.max(10),
        height: pane.height.max(8),
    };
    if *config != next {
        *config = next;
    }
}

fn dungeon_ruleset() -> WfcRuleset {
    let wall = WfcTileId(WALL);
    let floor = WfcTileId(FLOOR);
    let entrance = WfcTileId(ENTRANCE);
    let exit = WfcTileId(EXIT);
    let room = [wall, floor, entrance, exit];
    let border = [wall, entrance, exit];

    WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(wall, 1.0, "Wall"),
            WfcTileDefinition::new(floor, 6.0, "Floor"),
            WfcTileDefinition::new(entrance, 1.0, "Entrance"),
            WfcTileDefinition::new(exit, 1.0, "Exit"),
        ],
    )
    .with_rule(wall, WfcDirection::XPos, room)
    .with_rule(wall, WfcDirection::XNeg, room)
    .with_rule(wall, WfcDirection::YPos, room)
    .with_rule(wall, WfcDirection::YNeg, room)
    .with_rule(floor, WfcDirection::XPos, room)
    .with_rule(floor, WfcDirection::XNeg, room)
    .with_rule(floor, WfcDirection::YPos, room)
    .with_rule(floor, WfcDirection::YNeg, room)
    .with_rule(entrance, WfcDirection::XPos, room)
    .with_rule(entrance, WfcDirection::XNeg, border)
    .with_rule(entrance, WfcDirection::YPos, room)
    .with_rule(entrance, WfcDirection::YNeg, room)
    .with_rule(exit, WfcDirection::XPos, border)
    .with_rule(exit, WfcDirection::XNeg, room)
    .with_rule(exit, WfcDirection::YPos, room)
    .with_rule(exit, WfcDirection::YNeg, room)
}

#[allow(clippy::result_large_err)]
fn solve_dungeon(config: &DungeonConfig) -> Result<saddle_procgen_wfc::WfcSolution, saddle_procgen_wfc::WfcFailure> {
    let ruleset = dungeon_ruleset();
    let wall = WfcTileId(WALL);
    let floor = WfcTileId(FLOOR);
    let entrance = WfcTileId(ENTRANCE);
    let exit = WfcTileId(EXIT);

    let mut request = WfcRequest::new(
        WfcGridSize::new_2d(config.width, config.height),
        ruleset,
        WfcSeed(config.seed),
    );
    request.fixed_cells = vec![
        WfcFixedCell::new(UVec3::new(0, config.height / 2, 0), entrance),
        WfcFixedCell::new(UVec3::new(config.width - 1, config.height / 2, 0), exit),
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
            min_count: Some((config.width * config.height / 3).max(20)),
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

    solve_wfc(&request)
}

fn tile_to_world(x: u32, y: u32) -> Vec3 {
    Vec3::new(x as f32 * TILE_SIZE, 0.0, y as f32 * TILE_SIZE)
}

fn rebuild_dungeon(
    mut commands: Commands,
    config: Res<DungeonConfig>,
    mut state: ResMut<DungeonState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    roots: Query<Entity, With<DungeonVisualRoot>>,
    floor_sources: Query<Entity, With<NavmeshFloorSource>>,
    wall_meshes: Query<Entity, With<WallMesh>>,
    agents: Query<Entity, With<AgentMarker>>,
) {
    if !config.is_changed() && state.built {
        return;
    }

    for entity in roots.iter().chain(floor_sources.iter()).chain(wall_meshes.iter()).chain(agents.iter()) {
        commands.entity(entity).despawn();
    }
    if let Some(surface) = state.surface_entity.take() {
        commands.entity(surface).despawn();
    }
    state.agent_entity = None;

    let solution = match solve_dungeon(&config) {
        Ok(s) => s,
        Err(_) => {
            state.built = false;
            return;
        }
    };

    let surface_entity = commands
        .spawn((
            Name::new("Navmesh Surface"),
            NavmeshSurface::default(),
            NavmeshBakeSettings {
                agent_radius: 0.3,
                async_baking: false,
                ..default()
            },
            Transform::default(),
            Visibility::Visible,
        ))
        .id();
    state.surface_entity = Some(surface_entity);

    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.55, 0.35),
        ..default()
    });
    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.3, 0.25),
        ..default()
    });
    let entrance_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.85, 0.45),
        ..default()
    });
    let exit_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.35, 0.25),
        ..default()
    });

    let floor_mesh = meshes.add(Cuboid::new(TILE_SIZE, 0.15, TILE_SIZE));
    let wall_mesh = meshes.add(Cuboid::new(TILE_SIZE, 2.0, TILE_SIZE));

    let mut entrance_pos = Vec3::ZERO;
    let mut exit_pos = Vec3::ZERO;

    for y in 0..solution.grid.size.height {
        for x in 0..solution.grid.size.width {
            let tile_id = solution
                .grid
                .tile_at(UVec3::new(x, y, 0))
                .expect("tile should exist")
                .0;
            let pos = tile_to_world(x, y);

            match tile_id {
                WALL => {
                    commands.spawn((
                        Name::new("Wall"),
                        WallMesh,
                        Mesh3d(wall_mesh.clone()),
                        MeshMaterial3d(wall_material.clone()),
                        Transform::from_translation(pos + Vec3::Y * 1.0),
                    ));
                }
                FLOOR | ENTRANCE | EXIT => {
                    let mat = match tile_id {
                        ENTRANCE => {
                            entrance_pos = pos;
                            entrance_material.clone()
                        }
                        EXIT => {
                            exit_pos = pos;
                            exit_material.clone()
                        }
                        _ => floor_material.clone(),
                    };

                    commands.spawn((
                        Name::new("Floor"),
                        Mesh3d(floor_mesh.clone()),
                        MeshMaterial3d(mat),
                        Transform::from_translation(pos),
                    ));

                    commands.spawn((
                        Name::new("Floor Navmesh Source"),
                        NavmeshFloorSource,
                        NavmeshSource::new(surface_entity, NavmeshSourceKind::Walkable),
                        NavmeshPrimitiveSource::new(NavmeshPrimitive::Quad {
                            size: Vec2::splat(TILE_SIZE),
                        }),
                        Transform::from_translation(pos),
                    ));
                }
                _ => {}
            }
        }
    }

    state.entrance_pos = entrance_pos;
    state.exit_pos = exit_pos;

    let agent_mesh = meshes.add(Sphere::new(0.35));
    let agent_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.6, 0.95),
        ..default()
    });

    let agent_entity = commands
        .spawn((
            Name::new("Agent"),
            AgentMarker,
            Mesh3d(agent_mesh),
            MeshMaterial3d(agent_material),
            Transform::from_translation(entrance_pos + Vec3::Y * 0.5),
            NavmeshAgent::new(surface_entity).with_max_speed(4.0),
            NavmeshFollowTarget::Point(exit_pos),
            NavmeshSteeringOutput::default(),
        ))
        .id();
    state.agent_entity = Some(agent_entity);
    state.built = true;
}

fn apply_steering(
    time: Res<Time>,
    mut agents: Query<(&mut Transform, &NavmeshSteeringOutput), With<AgentMarker>>,
) {
    for (mut transform, steering) in &mut agents {
        if steering.desired_velocity.length_squared() > 0.001 {
            transform.translation += steering.desired_velocity * time.delta_secs();
            transform.translation.y = 0.5;
        }
    }
}

fn update_overlay(
    state: Res<DungeonState>,
    config: Res<DungeonConfig>,
    agents: Query<(&Transform, &NavmeshSteeringOutput), With<AgentMarker>>,
    mut overlays: Query<&mut Text, With<OverlayText>>,
) {
    if !state.is_changed() && !config.is_changed() {
        let any_agent_changed = agents.iter().any(|(_, s)| s.reached_goal);
        if !any_agent_changed && agents.iter().count() == 0 {
            return;
        }
    }

    let agent_info = agents
        .iter()
        .next()
        .map(|(t, s)| {
            format!(
                "\nagent pos: ({:.1}, {:.1})\nreached goal: {}\nremaining: {:.1}",
                t.translation.x,
                t.translation.z,
                s.reached_goal,
                s.remaining_distance,
            )
        })
        .unwrap_or_default();

    for mut overlay in &mut overlays {
        *overlay = Text::new(format!(
            "wfc navmesh_dungeon\nseed: {}\nsize: {}x{}\nentrance: ({:.0}, {:.0})\nexit: ({:.0}, {:.0}){}",
            config.seed,
            config.width,
            config.height,
            state.entrance_pos.x,
            state.entrance_pos.z,
            state.exit_pos.x,
            state.exit_pos.z,
            agent_info,
        ));
    }
}
