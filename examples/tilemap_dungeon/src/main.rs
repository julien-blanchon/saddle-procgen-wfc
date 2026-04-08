#[cfg(feature = "e2e")]
mod e2e;

use bevy::prelude::*;
use saddle_ai_fov::{FovPlugin, GridFov, GridFovState, GridMapSpec, GridOpacityMap};
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcBorder, WfcBorderConstraint, WfcDirection, WfcFixedCell, WfcGlobalConstraint, WfcGridSize,
    WfcRequest, WfcRuleset, WfcSeed, WfcTileCountConstraint, WfcTileDefinition, WfcTileId,
    WfcTopology, solve_wfc,
};
use saddle_world_tilemap::{
    TileCell, TileCoord, TileLayerConfig, TileLayerId, TileLayerRenderConfig, TileLayerState,
    Tilemap, TilemapDebugOverlay, TilemapDebugSettings, TilemapGeometry, TilemapPlugin,
};
use saddle_world_tilemap_example_support as support;

const TILE_SIZE: f32 = 30.0;

const GROUND_LAYER: TileLayerId = support::GROUND_LAYER;
const DETAIL_LAYER: TileLayerId = support::DETAIL_LAYER;
const HIGHLIGHT_LAYER: TileLayerId = support::HIGHLIGHT_LAYER;

const WALL: u16 = 0;
const FLOOR: u16 = 1;
const ENTRANCE: u16 = 2;
const EXIT: u16 = 3;

#[derive(Resource, Clone, PartialEq)]
struct DungeonConfig {
    seed: u64,
    width: u32,
    height: u32,
    show_fov: bool,
    fov_radius: u32,
}

impl Default for DungeonConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 24,
            height: 18,
            show_fov: true,
            fov_radius: 6,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "WFC Dungeon", position = "top-right")]
struct DungeonPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 12.0, max = 40.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 10.0, max = 30.0, step = 1.0)]
    height: u32,
    #[pane(toggle)]
    show_fov: bool,
    #[pane(slider, min = 2.0, max = 12.0, step = 1.0)]
    fov_radius: u32,
}

impl Default for DungeonPane {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 24,
            height: 18,
            show_fov: true,
            fov_radius: 6,
        }
    }
}

#[derive(Component)]
struct DungeonCamera;

#[derive(Component)]
struct FovViewer;

#[derive(Component)]
struct FovOverlayRoot;

#[derive(Resource)]
struct DungeonScene {
    palette: support::DemoPalette,
    map_entity: Option<Entity>,
    overlay_entity: Entity,
    camera_entity: Entity,
    viewer_pos: IVec2,
}

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.04)));
    app.insert_resource(support::TilemapExamplePane {
        highlight_alpha: 0.82,
        ..default()
    });
    app.init_resource::<DungeonConfig>();
    app.init_resource::<DungeonPane>();
    app.add_plugins(
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "wfc tilemap_dungeon".into(),
                    resolution: (1500, 920).into(),
                    ..default()
                }),
                ..default()
            }),
    );
    app.add_plugins((
        support::pane_plugins(),
        TilemapPlugin::default().with_debug_settings(TilemapDebugSettings {
            enabled: false,
            draw_dirty_chunks: false,
            ..default()
        }),
        FovPlugin::default(),
    ));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::TilemapDungeonE2EPlugin);
    app.register_pane::<support::TilemapExamplePane>();
    app.register_pane::<DungeonPane>();
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
            support::sync_example_pane,
            rebuild_dungeon,
            move_viewer,
            update_fov_overlay,
        )
            .chain(),
    );
    app.run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let palette = support::DemoPalette::new(&mut images);
    let overlay = support::spawn_overlay(
        &mut commands,
        "WFC dungeon + tilemap + FOV\nWASD to move viewer, pane to retune.\nWFC generates walls and floors, tilemap renders, FOV computes visibility.",
    );
    let camera = commands
        .spawn((
            Name::new("Dungeon Camera"),
            DungeonCamera,
            Camera2d,
            Transform::from_xyz(0.0, 0.0, 999.0),
        ))
        .id();

    commands.insert_resource(DungeonScene {
        palette,
        map_entity: None,
        overlay_entity: overlay,
        camera_entity: camera,
        viewer_pos: IVec2::ZERO,
    });
}

fn sync_pane_to_config(pane: Res<DungeonPane>, mut config: ResMut<DungeonConfig>) {
    let next = DungeonConfig {
        seed: pane.seed as u64,
        width: pane.width.max(12),
        height: pane.height.max(10),
        show_fov: pane.show_fov,
        fov_radius: pane.fov_radius.clamp(2, 12),
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

fn build_tilemap(palette: &support::DemoPalette, solution: &saddle_procgen_wfc::WfcSolution) -> Tilemap {
    let mut map = Tilemap::new(
        TilemapGeometry::square(Vec2::splat(TILE_SIZE)),
        UVec2::splat(12),
    );
    let catalog = palette.catalog();
    map.insert_layer(TileLayerState::new(
        TileLayerConfig::visual(
            GROUND_LAYER,
            "Ground",
            TileLayerRenderConfig::new(palette.atlas.clone()).with_z_index(0.0),
        ),
        catalog.clone(),
    ));
    map.insert_layer(TileLayerState::new(
        TileLayerConfig::visual(
            DETAIL_LAYER,
            "Detail",
            TileLayerRenderConfig::new(palette.atlas.clone()).with_z_index(2.0),
        ),
        catalog.clone(),
    ));
    map.insert_layer(TileLayerState::new(
        TileLayerConfig::visual(
            HIGHLIGHT_LAYER,
            "FOV Overlay",
            TileLayerRenderConfig::new(palette.atlas.clone())
                .with_z_index(4.0)
                .with_tint(Color::srgba(1.0, 1.0, 1.0, 0.82)),
        ),
        catalog,
    ));

    for y in 0..solution.grid.size.height {
        for x in 0..solution.grid.size.width {
            let coord = TileCoord::new(x as i32, y as i32);
            let tile_id = solution
                .grid
                .tile_at(UVec3::new(x, y, 0))
                .expect("tile should exist")
                .0;
            match tile_id {
                WALL => {
                    map.set_tile(GROUND_LAYER, coord, TileCell::new(palette.tiles.wall));
                }
                FLOOR => {
                    map.set_tile(GROUND_LAYER, coord, TileCell::new(palette.tiles.grass));
                }
                ENTRANCE => {
                    map.set_tile(
                        GROUND_LAYER,
                        coord,
                        TileCell::new(palette.tiles.grass)
                            .with_tint(Color::srgb(0.3, 0.85, 0.45)),
                    );
                }
                EXIT => {
                    map.set_tile(
                        GROUND_LAYER,
                        coord,
                        TileCell::new(palette.tiles.grass)
                            .with_tint(Color::srgb(0.85, 0.35, 0.25)),
                    );
                }
                _ => {
                    map.set_tile(GROUND_LAYER, coord, TileCell::new(palette.tiles.grass));
                }
            }
        }
    }

    map
}

fn build_fov_grid(solution: &saddle_procgen_wfc::WfcSolution) -> GridOpacityMap {
    let spec = GridMapSpec {
        origin: Vec2::ZERO,
        dimensions: UVec2::new(solution.grid.size.width, solution.grid.size.height),
        cell_size: Vec2::splat(TILE_SIZE),
    };
    GridOpacityMap::from_fn(spec, |cell| {
        if cell.x < 0 || cell.y < 0 {
            return true;
        }
        let tile = solution
            .grid
            .tile_at(UVec3::new(cell.x as u32, cell.y as u32, 0));
        tile.is_none_or(|t| t.0 == WALL)
    })
}

fn find_entrance(solution: &saddle_procgen_wfc::WfcSolution) -> IVec2 {
    for y in 0..solution.grid.size.height {
        for x in 0..solution.grid.size.width {
            if solution.grid.tile_at(UVec3::new(x, y, 0)).map(|t| t.0) == Some(ENTRANCE) {
                return IVec2::new(x as i32, y as i32);
            }
        }
    }
    IVec2::new(1, 1)
}

fn rebuild_dungeon(
    mut commands: Commands,
    config: Res<DungeonConfig>,
    mut scene: ResMut<DungeonScene>,
    mut overlays: Query<&mut Text, With<support::OverlayText>>,
    mut cameras: Query<&mut Transform, With<DungeonCamera>>,
    viewers: Query<Entity, With<FovViewer>>,
    fov_roots: Query<Entity, With<FovOverlayRoot>>,
) {
    if !config.is_changed() && scene.map_entity.is_some() {
        return;
    }

    if let Some(entity) = scene.map_entity.take() {
        commands.entity(entity).despawn();
    }
    for entity in &viewers {
        commands.entity(entity).despawn();
    }
    for entity in &fov_roots {
        commands.entity(entity).despawn();
    }

    let solution = match solve_dungeon(&config) {
        Ok(s) => s,
        Err(_failure) => {
            if let Ok(mut overlay) = overlays.get_mut(scene.overlay_entity) {
                *overlay = Text::new(format!(
                    "wfc tilemap_dungeon\nseed: {} FAILED — try another seed",
                    config.seed,
                ));
            }
            return;
        }
    };

    let opacity_grid = build_fov_grid(&solution);
    let entrance = find_entrance(&solution);
    scene.viewer_pos = entrance;

    commands.insert_resource(opacity_grid);

    let viewer_world = Vec3::new(
        entrance.x as f32 * TILE_SIZE,
        entrance.y as f32 * TILE_SIZE,
        10.0,
    );
    commands.spawn((
        Name::new("FOV Viewer"),
        FovViewer,
        GridFov::new(config.fov_radius as i32),
        Transform::from_translation(viewer_world),
    ));

    let map = build_tilemap(&scene.palette, &solution);
    let center = support::map_local_center(
        &map,
        UVec2::new(config.width, config.height),
    );
    let map_entity = support::spawn_map(
        &mut commands,
        "Dungeon Tilemap",
        map,
        Vec3::ZERO,
        TilemapDebugOverlay::default(),
    );

    if let Ok(mut camera) = cameras.get_mut(scene.camera_entity) {
        camera.translation = Vec3::new(center.x, center.y, 999.0);
    }

    if let Ok(mut overlay) = overlays.get_mut(scene.overlay_entity) {
        *overlay = Text::new(format!(
            "wfc tilemap_dungeon\nseed: {}\nsize: {}x{}\nsignature: {}\nWASD to move viewer\nFOV radius: {}",
            config.seed,
            config.width,
            config.height,
            solution.signature,
            config.fov_radius,
        ));
    }

    scene.map_entity = Some(map_entity);
}

fn move_viewer(
    keys: Res<ButtonInput<KeyCode>>,
    config: Res<DungeonConfig>,
    mut scene: ResMut<DungeonScene>,
    mut viewer_query: Query<(&mut Transform, &mut GridFov), With<FovViewer>>,
    opacity: Option<Res<GridOpacityMap>>,
) {
    let Some(opacity) = opacity else { return };
    let mut delta = IVec2::ZERO;
    if keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::ArrowUp) {
        delta.y += 1;
    }
    if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
        delta.y -= 1;
    }
    if keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::ArrowLeft) {
        delta.x -= 1;
    }
    if keys.just_pressed(KeyCode::KeyD) || keys.just_pressed(KeyCode::ArrowRight) {
        delta.x += 1;
    }

    if delta == IVec2::ZERO {
        return;
    }

    let next = scene.viewer_pos + delta;
    if next.x < 0
        || next.y < 0
        || next.x >= config.width as i32
        || next.y >= config.height as i32
    {
        return;
    }

    if opacity.is_opaque(next) {
        return;
    }

    scene.viewer_pos = next;

    for (mut transform, mut fov) in &mut viewer_query {
        transform.translation = Vec3::new(
            next.x as f32 * TILE_SIZE,
            next.y as f32 * TILE_SIZE,
            10.0,
        );
        fov.config.radius = config.fov_radius as i32;
    }
}

fn update_fov_overlay(
    mut commands: Commands,
    config: Res<DungeonConfig>,
    scene: Res<DungeonScene>,
    fov_roots: Query<Entity, With<FovOverlayRoot>>,
    viewers: Query<&GridFovState, With<FovViewer>>,
) {
    for entity in &fov_roots {
        commands.entity(entity).despawn();
    }

    if !config.show_fov {
        return;
    }

    let Ok(fov_state) = viewers.single() else {
        return;
    };

    commands
        .spawn((
            Name::new("FOV Overlay"),
            FovOverlayRoot,
            Transform::default(),
            Visibility::Visible,
        ))
        .with_children(|parent| {
            for y in 0..config.height as i32 {
                for x in 0..config.width as i32 {
                    let cell = IVec2::new(x, y);
                    let is_visible = fov_state.visible_now.contains(&cell);
                    let is_explored = fov_state.explored.contains(&cell);

                    if is_visible {
                        continue;
                    }

                    let alpha = if is_explored { 0.55 } else { 0.92 };
                    parent.spawn((
                        Sprite::from_color(
                            Color::srgba(0.0, 0.0, 0.05, alpha),
                            Vec2::splat(TILE_SIZE),
                        ),
                        Transform::from_xyz(
                            x as f32 * TILE_SIZE,
                            y as f32 * TILE_SIZE,
                            8.0,
                        ),
                    ));
                }
            }

            parent.spawn((
                Sprite::from_color(
                    Color::srgb(0.2, 0.9, 0.4),
                    Vec2::splat(TILE_SIZE * 0.5),
                ),
                Transform::from_xyz(
                    scene.viewer_pos.x as f32 * TILE_SIZE,
                    scene.viewer_pos.y as f32 * TILE_SIZE,
                    9.0,
                ),
            ));
        });
}
