use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcGridSize, WfcRequest, WfcSeed, WfcTileGrid, WfcTileId, WfcTopology, learn_adjacency_rules,
    solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Learned rules: hand-place a small sample grid, learn adjacency from it,
// then use the learned rules to generate a larger map.
//
// The left side shows the hand-authored sample. The right side shows the
// WFC-generated output using rules learned from that sample.
// ---------------------------------------------------------------------------

fn build_sample() -> WfcTileGrid {
    let grass = WfcTileId(0);
    let road = WfcTileId(1);
    let water = WfcTileId(2);
    let sand = WfcTileId(3);

    // A small 8x6 hand-placed sample.
    // Grass fills most of the area. A road runs horizontally across the middle.
    // Water sits at the bottom with sand as a transition border.
    let mut sample =
        WfcTileGrid::new_empty(WfcTopology::Cartesian2d, WfcGridSize::new_2d(8, 6));

    for x in 0..8u32 {
        // Top rows: grass
        sample.set_tile_at(UVec3::new(x, 5, 0), grass);
        sample.set_tile_at(UVec3::new(x, 4, 0), grass);

        // Middle: road
        sample.set_tile_at(UVec3::new(x, 3, 0), road);

        // Below road: grass/sand transition
        sample.set_tile_at(UVec3::new(x, 2, 0), grass);
        sample.set_tile_at(UVec3::new(x, 1, 0), sand);

        // Bottom: water
        sample.set_tile_at(UVec3::new(x, 0, 0), water);
    }

    sample
}

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct LearnedConfig {
    seed: u64,
    width: u32,
    height: u32,
}

impl Default for LearnedConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 20,
            height: 14,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Learned Rules", position = "top-right")]
struct LearnedPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 30.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 22.0, step = 1.0)]
    height: u32,
}

impl Default for LearnedPane {
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
struct SampleRoot;

#[derive(Component)]
struct GeneratedRoot;

#[derive(Component)]
struct Overlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)));
    app.init_resource::<LearnedConfig>();
    app.init_resource::<LearnedPane>();
    app.init_resource::<CurrentSolution>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc learned_rules".into(),
            resolution: (1500, 920).into(),
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
    app.register_pane::<LearnedPane>();
    common::install_auto_exit(&mut app);
    app.add_systems(Startup, (setup, render_sample));
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
            regenerate_solution,
            render_generated,
            update_overlay,
        )
            .chain(),
    );
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Learned Camera"), Camera2d));
    commands.spawn((
        Name::new("Learned Overlay"),
        Overlay,
        Text::new("learned_rules"),
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

fn color_for_learned_tile(tile: WfcTileId) -> Color {
    match tile.0 {
        0 => Color::srgb(0.25, 0.65, 0.30), // Grass
        1 => Color::srgb(0.55, 0.55, 0.55), // Road
        2 => Color::srgb(0.18, 0.42, 0.75), // Water
        3 => Color::srgb(0.82, 0.76, 0.55), // Sand
        _ => Color::srgb(0.5, 0.5, 0.5),
    }
}

fn render_sample(mut commands: Commands) {
    let sample = build_sample();
    let tile_size = 32.0;
    // Position sample on the left side of the screen
    let offset_x = -380.0;
    let origin = Vec2::new(
        offset_x - (sample.width() as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(sample.height() as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            SampleRoot,
            common::spatial_root("Sample Grid", Transform::default()),
        ))
        .with_children(|parent| {
            // Label
            parent.spawn((
                Text2d::new("SAMPLE"),
                TextFont::from_font_size(18.0),
                TextColor(Color::WHITE),
                Transform::from_xyz(
                    offset_x,
                    (sample.height() as f32 * tile_size) * 0.5 + 20.0,
                    1.0,
                ),
            ));

            for (pos, tile) in sample.iter() {
                parent.spawn((
                    Sprite::from_color(
                        color_for_learned_tile(tile),
                        Vec2::splat(tile_size - 2.0),
                    ),
                    Transform::from_xyz(
                        origin.x + pos.x as f32 * tile_size,
                        origin.y + pos.y as f32 * tile_size,
                        0.0,
                    ),
                ));
            }
        });
}

fn sync_pane_to_config(pane: Res<LearnedPane>, mut config: ResMut<LearnedConfig>) {
    let next = LearnedConfig {
        seed: pane.seed as u64,
        width: pane.width.max(8),
        height: pane.height.max(6),
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_solution(config: Res<LearnedConfig>, mut solution: ResMut<CurrentSolution>) {
    if !config.is_changed() && solution.0.is_some() {
        return;
    }

    let sample = build_sample();
    let ruleset = learn_adjacency_rules(&sample);
    let request = WfcRequest::new(
        WfcGridSize::new_2d(config.width, config.height),
        ruleset,
        WfcSeed(config.seed),
    );
    solution.0 = Some(solve_wfc(&request).expect("learned rules should solve"));
}

fn render_generated(
    mut commands: Commands,
    solution: Res<CurrentSolution>,
    roots: Query<Entity, With<GeneratedRoot>>,
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

    let tile_size = 32.0;
    // Position generated grid on the right side
    let offset_x = 200.0;
    let origin = Vec2::new(
        offset_x - (solution.grid.size.width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(solution.grid.size.height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            GeneratedRoot,
            common::spatial_root("Generated Grid", Transform::default()),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("GENERATED"),
                TextFont::from_font_size(18.0),
                TextColor(Color::WHITE),
                Transform::from_xyz(
                    offset_x,
                    (solution.grid.size.height as f32 * tile_size) * 0.5 + 20.0,
                    1.0,
                ),
            ));

            for (pos, tile) in solution.grid.iter() {
                parent.spawn((
                    Sprite::from_color(
                        color_for_learned_tile(tile),
                        Vec2::splat(tile_size - 2.0),
                    ),
                    Transform::from_xyz(
                        origin.x + pos.x as f32 * tile_size,
                        origin.y + pos.y as f32 * tile_size,
                        0.0,
                    ),
                ));
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

    let sample = build_sample();
    let ruleset = learn_adjacency_rules(&sample);

    **overlay = Text::new(format!(
        "learned_rules\n\
         sample: {}x{} ({} tiles observed)\n\
         learned: {} rules across {} tile types\n\
         generated: {}x{}\n\
         signature: {}\n\
         obs/backtracks: {}/{}\n\
         elapsed: {:.2}ms",
        sample.width(),
        sample.height(),
        sample.size.total_cells(),
        ruleset.adjacency.len(),
        ruleset.tiles.len(),
        solution.grid.size.width,
        solution.grid.size.height,
        solution.signature,
        solution.stats.observation_count,
        solution.stats.backtrack_count,
        solution.stats.elapsed_ms,
    ));
}
