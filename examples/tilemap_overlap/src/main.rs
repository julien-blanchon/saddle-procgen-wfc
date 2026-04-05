use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcGridSize, WfcOverlapOptions, WfcOverlapRequest, WfcSeed, WfcTileGrid, WfcTileId,
    WfcTopology, solve_overlap_wfc_2d,
};
use saddle_world_tilemap::{
    TileCell, TileLayerConfig, TileLayerRenderConfig, TileLayerState, Tilemap, TilemapDebugOverlay,
    TilemapDebugSettings, TilemapGeometry, TilemapPlugin,
};
use saddle_world_tilemap_example_support as support;

#[derive(Resource, Clone, PartialEq)]
struct OverlapConfig {
    seed: u64,
    width: u32,
    height: u32,
    pattern_size: u32,
    periodic_output: bool,
}

impl Default for OverlapConfig {
    fn default() -> Self {
        Self {
            seed: 77,
            width: 42,
            height: 30,
            pattern_size: 3,
            periodic_output: true,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Tilemap Overlap", position = "top-right")]
struct OverlapPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 20.0, max = 64.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 16.0, max = 48.0, step = 1.0)]
    height: u32,
    #[pane(slider, min = 2.0, max = 4.0, step = 1.0)]
    pattern_size: u32,
    #[pane(toggle)]
    periodic_output: bool,
}

impl Default for OverlapPane {
    fn default() -> Self {
        Self {
            seed: 77,
            width: 42,
            height: 30,
            pattern_size: 3,
            periodic_output: true,
        }
    }
}

#[derive(Component)]
struct OverlapCamera;

#[derive(Resource)]
struct OverlapScene {
    palette: support::DemoPalette,
    map_entity: Option<Entity>,
    overlay_entity: Entity,
    camera_entity: Entity,
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.04)))
        .insert_resource(support::TilemapExamplePane {
            highlight_alpha: 0.82,
            ..default()
        })
        .init_resource::<OverlapConfig>()
        .init_resource::<OverlapPane>()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "wfc tilemap_overlap".into(),
                        resolution: (1500, 920).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins((
            support::pane_plugins(),
            TilemapPlugin::default().with_debug_settings(TilemapDebugSettings {
                enabled: false,
                draw_dirty_chunks: false,
                ..default()
            }),
        ))
        .register_pane::<support::TilemapExamplePane>()
        .register_pane::<OverlapPane>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (sync_pane_to_config, support::sync_example_pane, rebuild_map).chain(),
        )
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let palette = support::DemoPalette::new(&mut images);
    let overlay = support::spawn_overlay(
        &mut commands,
        "Overlap-model WFC drives a live tilemap surface.\nRetune the seed, output size, and pattern window in the pane to remix the learned patch library.",
    );
    let camera = commands
        .spawn((
            Name::new("Overlap Camera"),
            OverlapCamera,
            Camera2d,
            Transform::from_xyz(0.0, 0.0, 999.0),
        ))
        .id();

    commands.insert_resource(OverlapScene {
        palette,
        map_entity: None,
        overlay_entity: overlay,
        camera_entity: camera,
    });
}

fn sync_pane_to_config(pane: Res<OverlapPane>, mut config: ResMut<OverlapConfig>) {
    let next = OverlapConfig {
        seed: pane.seed as u64,
        width: pane.width.max(12),
        height: pane.height.max(12),
        pattern_size: pane.pattern_size.clamp(2, 4),
        periodic_output: pane.periodic_output,
    };
    if *config != next {
        *config = next;
    }
}

fn rebuild_map(
    mut commands: Commands,
    config: Res<OverlapConfig>,
    mut scene: ResMut<OverlapScene>,
    mut overlays: Query<&mut Text, With<support::OverlayText>>,
    mut cameras: Query<&mut Transform, With<OverlapCamera>>,
) {
    if !config.is_changed() && scene.map_entity.is_some() {
        return;
    }

    if let Some(entity) = scene.map_entity.take() {
        commands.entity(entity).despawn();
    }

    let solution = solve_scene(&config);
    let map = build_tilemap(&scene.palette, &solution);
    let center = support::map_local_center(&map, UVec2::new(config.width, config.height));
    let map_entity = support::spawn_map(
        &mut commands,
        "Overlap Tilemap",
        map,
        Vec3::ZERO,
        TilemapDebugOverlay::default(),
    );

    if let Ok(mut camera) = cameras.get_mut(scene.camera_entity) {
        camera.translation = Vec3::new(center.x, center.y, 999.0);
    }

    if let Ok(mut overlay) = overlays.get_mut(scene.overlay_entity) {
        *overlay = Text::new(format!(
            "wfc overlap + tilemap\nseed: {}\noutput: {}x{}\npattern: {}x{}\nperiodic output: {}\nsignature: {}",
            config.seed,
            config.width,
            config.height,
            config.pattern_size,
            config.pattern_size,
            config.periodic_output,
            solution.signature,
        ));
    }

    scene.map_entity = Some(map_entity);
}

fn solve_scene(config: &OverlapConfig) -> saddle_procgen_wfc::WfcSolution {
    let mut request = WfcOverlapRequest::new(
        sample_patchwork(),
        WfcGridSize::new_2d(config.width, config.height),
        WfcSeed(config.seed),
    );
    request.options = WfcOverlapOptions {
        pattern_width: config.pattern_size,
        pattern_height: config.pattern_size,
        periodic_input: true,
        periodic_output: config.periodic_output,
    };
    solve_overlap_wfc_2d(&request).expect("tilemap overlap scene should solve")
}

fn build_tilemap(
    palette: &support::DemoPalette,
    solution: &saddle_procgen_wfc::WfcSolution,
) -> Tilemap {
    let mut map = Tilemap::new(TilemapGeometry::square(Vec2::splat(30.0)), UVec2::splat(12));
    let catalog = palette.catalog();
    map.insert_layer(TileLayerState::new(
        TileLayerConfig::visual(
            support::GROUND_LAYER,
            "Ground",
            TileLayerRenderConfig::new(palette.atlas.clone()).with_z_index(0.0),
        ),
        catalog.clone(),
    ));
    map.insert_layer(TileLayerState::new(
        TileLayerConfig::visual(
            support::DETAIL_LAYER,
            "Detail",
            TileLayerRenderConfig::new(palette.atlas.clone()).with_z_index(2.0),
        ),
        catalog.clone(),
    ));
    map.insert_layer(TileLayerState::new(
        TileLayerConfig::visual(
            support::HIGHLIGHT_LAYER,
            "Highlight",
            TileLayerRenderConfig::new(palette.atlas.clone())
                .with_z_index(4.0)
                .with_tint(Color::srgba(1.0, 1.0, 1.0, 0.82)),
        ),
        catalog,
    ));

    for y in 0..solution.grid.size.height {
        for x in 0..solution.grid.size.width {
            let coord = saddle_world_tilemap::TileCoord::new(x as i32, y as i32);
            match solution
                .grid
                .tile_at(UVec3::new(x, y, 0))
                .expect("tile should exist")
                .0
            {
                1 => map.set_tile(
                    support::GROUND_LAYER,
                    coord,
                    TileCell::new(palette.tiles.road),
                ),
                2 => map.set_tile(
                    support::GROUND_LAYER,
                    coord,
                    TileCell::new(palette.tiles.water),
                ),
                3 => {
                    map.set_tile(
                        support::GROUND_LAYER,
                        coord,
                        TileCell::new(palette.tiles.grass),
                    );
                    map.set_tile(
                        support::DETAIL_LAYER,
                        coord,
                        TileCell::new(palette.tiles.flower).with_tint(Color::srgb(1.0, 0.92, 0.84)),
                    );
                }
                _ => map.set_tile(
                    support::GROUND_LAYER,
                    coord,
                    TileCell::new(palette.tiles.grass),
                ),
            }
        }
    }

    map
}

fn sample_patchwork() -> WfcTileGrid {
    let meadow = WfcTileId(0);
    let road = WfcTileId(1);
    let water = WfcTileId(2);
    let flowers = WfcTileId(3);
    let tiles = vec![
        meadow, meadow, road, road, meadow, flowers, meadow, road, road, meadow, meadow, flowers,
        meadow, meadow, water, water, meadow, meadow, road, road, water, meadow, meadow, meadow,
        flowers, meadow, meadow, meadow, road, road, flowers, meadow, meadow, road, road, meadow,
    ];

    WfcTileGrid {
        topology: WfcTopology::Cartesian2d,
        size: WfcGridSize::new_2d(6, 6),
        rotations: vec![0; tiles.len()],
        tiles,
    }
}
