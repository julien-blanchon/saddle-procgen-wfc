use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{GenerateWfc, WfcPlugin, WfcSolved, WfcSystems};
use saddle_procgen_wfc_example_support as common;

#[derive(Resource, Clone, PartialEq)]
struct AsyncConfig {
    seed: u64,
    width: u32,
    height: u32,
}

impl Default for AsyncConfig {
    fn default() -> Self {
        Self {
            seed: 77,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Async Runtime", position = "top-right")]
struct AsyncPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 28.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 20.0, step = 1.0)]
    height: u32,
}

impl Default for AsyncPane {
    fn default() -> Self {
        Self {
            seed: 77,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource, Default)]
struct SolveText(String);

#[derive(Resource, Default)]
struct LastRequested(Option<AsyncConfig>);

#[derive(Component)]
struct AsyncGridRoot;

#[derive(Component)]
struct AsyncOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.055, 0.06, 0.08)));
    app.init_resource::<AsyncConfig>();
    app.init_resource::<AsyncPane>();
    app.init_resource::<SolveText>();
    app.init_resource::<LastRequested>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc async_runtime".into(),
            resolution: (1280, 840).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(PanePlugin);
    app.register_pane::<AsyncPane>();
    app.add_plugins(WfcPlugin::default());
    common::install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
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
            width: px(360),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.08, 0.10, 0.82)),
    ));
}

fn sync_pane_to_config(pane: Res<AsyncPane>, mut config: ResMut<AsyncConfig>) {
    let next = AsyncConfig {
        seed: pane.seed as u64,
        width: pane.width.max(8),
        height: pane.height.max(6),
    };
    if *config != next {
        *config = next;
    }
}

fn request_generation(
    config: Res<AsyncConfig>,
    mut last_requested: ResMut<LastRequested>,
    mut summary: ResMut<SolveText>,
    mut requests: MessageWriter<GenerateWfc>,
) {
    if last_requested.0.as_ref() == Some(&*config) {
        return;
    }

    let mut request = common::basic_request(config.seed);
    request.grid_size = saddle_procgen_wfc::WfcGridSize::new_2d(config.width, config.height);
    summary.0 = format!(
        "async_runtime\nqueued seed {} at {}x{}",
        config.seed, config.width, config.height
    );
    last_requested.0 = Some(config.clone());
    requests.write(GenerateWfc {
        request,
        label: Some(format!(
            "async basic {}x{} seed {}",
            config.width, config.height, config.seed
        )),
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
                common::spatial_root("Async Grid Root", Transform::default()),
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
}

fn update_overlay(summary: Res<SolveText>, mut overlay: Single<&mut Text, With<AsyncOverlay>>) {
    if summary.is_changed() {
        **overlay = Text::new(summary.0.clone());
    }
}
