use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcDirection, WfcGridSize, WfcRequest, WfcSeed, WfcSocketRulesetBuilder, WfcTileId,
    WfcTileSymmetry, WfcTopology, solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Socket-based ruleset: roads, grass, and directional pipes.
//
// Compare this to basic_2d -- sockets eliminate the need to manually enumerate
// every (tile, direction, allowed_tiles) triple. Instead you label each face
// and adjacency is derived automatically.
// ---------------------------------------------------------------------------

fn socket_request(seed: u64, width: u32, height: u32) -> WfcRequest {
    let mut builder = WfcSocketRulesetBuilder::new(WfcTopology::Cartesian2d)
        // pipe_in only connects to pipe_out, not to itself
        .add_asymmetric_pair("pipe_in", "pipe_out");

    // Grass: all faces are "grass"
    builder
        .add_tile(0u16, 5.0, "Grass")
        .all_sockets("g")
        .done();

    // Straight road (horizontal): road edges left/right, grass top/bottom.
    // With Rotate2, the solver also generates the vertical rotation.
    builder
        .add_tile(1u16, 2.0, "Straight Road")
        .socket(WfcDirection::XPos, "road")
        .socket(WfcDirection::XNeg, "road")
        .socket(WfcDirection::YPos, "g")
        .socket(WfcDirection::YNeg, "g")
        .symmetry(WfcTileSymmetry::Rotate2)
        .done();

    // Corner road: connects +X to -Y, grass on -X and +Y.
    // With Rotate4, all four corner orientations are generated.
    builder
        .add_tile(2u16, 1.0, "Corner Road")
        .socket(WfcDirection::XPos, "road")
        .socket(WfcDirection::XNeg, "g")
        .socket(WfcDirection::YPos, "g")
        .socket(WfcDirection::YNeg, "road")
        .symmetry(WfcTileSymmetry::Rotate4)
        .done();

    // Pipe source: emits pipe_out on +X, grass elsewhere.
    // Fixed symmetry — directional pipes should not auto-rotate because
    // rotating an asymmetric socket creates unsolvable configurations.
    builder
        .add_tile(3u16, 0.3, "Pipe Source")
        .socket(WfcDirection::XPos, "pipe_out")
        .socket(WfcDirection::XNeg, "g")
        .socket(WfcDirection::YPos, "g")
        .socket(WfcDirection::YNeg, "g")
        .done();

    // Pipe sink: receives pipe_in on -X, grass elsewhere.
    builder
        .add_tile(4u16, 0.3, "Pipe Sink")
        .socket(WfcDirection::XPos, "g")
        .socket(WfcDirection::XNeg, "pipe_in")
        .socket(WfcDirection::YPos, "g")
        .socket(WfcDirection::YNeg, "g")
        .done();

    let ruleset = builder.build().expect("socket ruleset should build");
    let mut request = WfcRequest::new(WfcGridSize::new_2d(width, height), ruleset, WfcSeed(seed));
    request.settings.max_backtracks = 1024;
    request
}

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct SocketConfig {
    seed: u64,
    width: u32,
    height: u32,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 20,
            height: 14,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Socket Builder", position = "top-right")]
struct SocketPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 30.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 22.0, step = 1.0)]
    height: u32,
}

impl Default for SocketPane {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 20,
            height: 14,
        }
    }
}

#[derive(Resource, Default)]
struct CurrentSolution(Option<saddle_procgen_wfc::WfcSolution>);

#[derive(Component)]
struct GridRoot;

#[derive(Component)]
struct Overlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)));
    app.init_resource::<SocketConfig>();
    app.init_resource::<SocketPane>();
    app.init_resource::<CurrentSolution>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc socket_builder".into(),
            resolution: (1360, 920).into(),
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
    app.register_pane::<SocketPane>();
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
    commands.spawn((Name::new("Socket Camera"), Camera2d));
    commands.spawn((
        Name::new("Socket Overlay"),
        Overlay,
        Text::new("socket_builder"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(400),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.08, 0.10, 0.82)),
    ));
}

fn sync_pane_to_config(pane: Res<SocketPane>, mut config: ResMut<SocketConfig>) {
    let next = SocketConfig {
        seed: pane.seed as u64,
        width: pane.width.max(8),
        height: pane.height.max(6),
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_solution(config: Res<SocketConfig>, mut solution: ResMut<CurrentSolution>) {
    if !config.is_changed() && solution.0.is_some() {
        return;
    }
    let request = socket_request(config.seed, config.width, config.height);
    solution.0 = Some(solve_wfc(&request).expect("socket request should solve"));
}

fn color_for_socket_tile(tile: WfcTileId) -> Color {
    match tile.0 {
        0 => Color::srgb(0.25, 0.65, 0.30), // Grass
        1 => Color::srgb(0.55, 0.55, 0.55), // Road
        2 => Color::srgb(0.60, 0.50, 0.45), // Corner road
        3 => Color::srgb(0.85, 0.35, 0.20), // Pipe source (red)
        4 => Color::srgb(0.20, 0.45, 0.85), // Pipe sink (blue)
        _ => Color::srgb(0.5, 0.5, 0.5),
    }
}

fn render_solution(
    mut commands: Commands,
    solution: Res<CurrentSolution>,
    roots: Query<Entity, With<GridRoot>>,
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

    let tile_size = 38.0;
    let origin = Vec2::new(
        -(solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            GridRoot,
            common::spatial_root("Socket Grid", Transform::default()),
        ))
        .with_children(|parent| {
            for y in 0..solution.grid.size.height {
                for x in 0..solution.grid.size.width {
                    let tile = solution
                        .grid
                        .tile_at(UVec3::new(x, y, 0))
                        .expect("tile should exist");
                    parent.spawn((
                        Sprite::from_color(
                            color_for_socket_tile(tile),
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

fn update_overlay(
    solution: Res<CurrentSolution>,
    mut overlay: Single<&mut Text, With<Overlay>>,
) {
    if !solution.is_changed() {
        return;
    }
    let Some(solution) = &solution.0 else {
        return;
    };

    let pipe_sources = solution
        .grid
        .tiles
        .iter()
        .filter(|t| t.0 == 3)
        .count();
    let pipe_sinks = solution
        .grid
        .tiles
        .iter()
        .filter(|t| t.0 == 4)
        .count();
    let roads = solution
        .grid
        .tiles
        .iter()
        .filter(|t| t.0 == 1 || t.0 == 2)
        .count();

    **overlay = Text::new(format!(
        "socket_builder\nsignature: {}\nobs/backtracks: {}/{}\nelapsed: {:.2}ms\nroads: {}  pipes: {} src / {} sink",
        solution.signature,
        solution.stats.observation_count,
        solution.stats.backtrack_count,
        solution.stats.elapsed_ms,
        roads,
        pipe_sources,
        pipe_sinks,
    ));
}
