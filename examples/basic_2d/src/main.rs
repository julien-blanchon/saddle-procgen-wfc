use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::solve_wfc;
use saddle_procgen_wfc_example_support as common;

#[derive(Resource, Clone, PartialEq)]
struct BasicConfig {
    seed: u64,
    width: u32,
    height: u32,
}

impl Default for BasicConfig {
    fn default() -> Self {
        Self {
            seed: 7,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Basic 2D", position = "top-right")]
struct BasicPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 28.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 20.0, step = 1.0)]
    height: u32,
}

impl Default for BasicPane {
    fn default() -> Self {
        Self {
            seed: 7,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource, Default)]
struct CurrentSolution(Option<saddle_procgen_wfc::WfcSolution>);

#[derive(Component)]
struct BasicGridRoot;

#[derive(Component)]
struct BasicOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)));
    app.init_resource::<BasicConfig>();
    app.init_resource::<BasicPane>();
    app.init_resource::<CurrentSolution>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc basic_2d".into(),
            resolution: (1280, 840).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(PanePlugin);
    app.register_pane::<BasicPane>();
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
    commands.spawn((Name::new("Basic 2D Camera"), Camera2d));
    commands.spawn((
        Name::new("Basic 2D Overlay"),
        BasicOverlay,
        Text::new("basic_2d"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(360),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.08, 0.10, 0.82)),
    ));
}

fn sync_pane_to_config(pane: Res<BasicPane>, mut config: ResMut<BasicConfig>) {
    let next = BasicConfig {
        seed: pane.seed as u64,
        width: pane.width.max(8),
        height: pane.height.max(6),
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_solution(config: Res<BasicConfig>, mut solution: ResMut<CurrentSolution>) {
    if !config.is_changed() && solution.0.is_some() {
        return;
    }

    let mut request = common::basic_request(config.seed);
    request.grid_size = saddle_procgen_wfc::WfcGridSize::new_2d(config.width, config.height);
    solution.0 = Some(solve_wfc(&request).expect("basic request should solve"));
}

fn render_solution(
    mut commands: Commands,
    solution: Res<CurrentSolution>,
    roots: Query<Entity, With<BasicGridRoot>>,
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

    let tile_size = 42.0;
    let origin = Vec2::new(
        -(solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            BasicGridRoot,
            common::spatial_root("Basic 2D Grid", Transform::default()),
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
                            common::color_for_tile_2d(tile),
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
    mut overlay: Single<&mut Text, With<BasicOverlay>>,
) {
    if !solution.is_changed() {
        return;
    }
    let Some(solution) = &solution.0 else {
        return;
    };

    **overlay = Text::new(format!(
        "basic_2d\nsignature: {}\nobs/backtracks: {}/{}\nelapsed: {:.2}ms\nsize: {}x{}",
        solution.signature,
        solution.stats.observation_count,
        solution.stats.backtrack_count,
        solution.stats.elapsed_ms,
        solution.grid.size.width,
        solution.grid.size.height
    ));
}
