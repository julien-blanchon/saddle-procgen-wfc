use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Data-driven WFC: load a ruleset from a RON file instead of hardcoding it.
//
// This example demonstrates the serde integration. The ruleset is deserialized
// from `assets/rules.ron` at startup. The same pattern works with JSON, TOML,
// or any format serde supports.
// ---------------------------------------------------------------------------

fn load_ruleset_from_ron() -> WfcRuleset {
    let ron_str = include_str!("../assets/rules.ron");
    ron::from_str(ron_str).expect("assets/rules.ron should be valid RON")
}

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct DataDrivenConfig {
    seed: u64,
    width: u32,
    height: u32,
}

impl Default for DataDrivenConfig {
    fn default() -> Self {
        Self {
            seed: 7,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Data-Driven", position = "top-right")]
struct DataDrivenPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 30.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 22.0, step = 1.0)]
    height: u32,
}

impl Default for DataDrivenPane {
    fn default() -> Self {
        Self {
            seed: 7,
            width: 18,
            height: 12,
        }
    }
}

#[derive(Resource)]
struct LoadedRuleset(WfcRuleset);

#[derive(Resource, Default)]
struct CurrentSolution(Option<saddle_procgen_wfc::WfcSolution>);

#[derive(Component)]
struct GridRoot;

#[derive(Component)]
struct Overlay;

fn main() {
    let ruleset = load_ruleset_from_ron();

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)));
    app.insert_resource(LoadedRuleset(ruleset));
    app.init_resource::<DataDrivenConfig>();
    app.init_resource::<DataDrivenPane>();
    app.init_resource::<CurrentSolution>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc data_driven".into(),
            resolution: (1280, 840).into(),
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
    app.register_pane::<DataDrivenPane>();
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
    commands.spawn((Name::new("Data-Driven Camera"), Camera2d));
    commands.spawn((
        Name::new("Data-Driven Overlay"),
        Overlay,
        Text::new("data_driven"),
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

fn sync_pane_to_config(pane: Res<DataDrivenPane>, mut config: ResMut<DataDrivenConfig>) {
    let next = DataDrivenConfig {
        seed: pane.seed as u64,
        width: pane.width.max(8),
        height: pane.height.max(6),
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_solution(
    config: Res<DataDrivenConfig>,
    ruleset: Res<LoadedRuleset>,
    mut solution: ResMut<CurrentSolution>,
) {
    if !config.is_changed() && solution.0.is_some() {
        return;
    }
    let request = WfcRequest::new(
        WfcGridSize::new_2d(config.width, config.height),
        ruleset.0.clone(),
        WfcSeed(config.seed),
    );
    solution.0 = Some(solve_wfc(&request).expect("data-driven request should solve"));
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

    let tile_size = 42.0;
    let origin = Vec2::new(
        -(solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            GridRoot,
            common::spatial_root("Data-Driven Grid", Transform::default()),
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
    mut overlay: Single<&mut Text, With<Overlay>>,
) {
    if !solution.is_changed() {
        return;
    }
    let Some(solution) = &solution.0 else {
        return;
    };

    // Demonstrate serde roundtrip in the overlay text
    let ron_roundtrip = ron::to_string(&solution.grid.size).unwrap_or_default();

    **overlay = Text::new(format!(
        "data_driven (loaded from RON)\nsignature: {}\nobs/backtracks: {}/{}\nelapsed: {:.2}ms\nsize: {}x{}\nron roundtrip: {}",
        solution.signature,
        solution.stats.observation_count,
        solution.stats.backtrack_count,
        solution.stats.elapsed_ms,
        solution.grid.size.width,
        solution.grid.size.height,
        ron_roundtrip,
    ));
}
