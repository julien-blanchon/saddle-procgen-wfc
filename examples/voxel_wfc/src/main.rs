#[cfg(feature = "e2e")]
mod e2e;

use std::sync::Arc;

use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcTileDefinition, WfcTileGrid,
    WfcTileId, WfcTopology, solve_wfc,
};
use saddle_world_voxel_world::{
    BlockId, ChunkViewer, ChunkViewerSettings, VoxelBlockSampler, VoxelWorldConfig,
    VoxelWorldGenerator, VoxelWorldPlugin,
};

const WFC_SIZE: u32 = 24;
const WFC_HEIGHT: u32 = 12;

const AIR: u16 = 0;
const STONE: u16 = 1;
const CAP: u16 = 2;
const FLOOR_TILE: u16 = 3;

#[derive(Resource, Clone, PartialEq)]
struct WfcVoxelConfig {
    seed: u64,
}

impl Default for WfcVoxelConfig {
    fn default() -> Self {
        Self { seed: 42 }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Voxel WFC", position = "top-right")]
struct WfcVoxelPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
}

impl Default for WfcVoxelPane {
    fn default() -> Self {
        Self { seed: 42 }
    }
}

#[derive(Clone)]
struct WfcBlockSampler {
    grid: Arc<WfcTileGrid>,
    wfc_width: u32,
    wfc_height: u32,
    wfc_depth: u32,
}

impl VoxelBlockSampler for WfcBlockSampler {
    fn sample_block(&self, world_pos: IVec3, _config: &VoxelWorldConfig) -> BlockId {
        if world_pos.x < 0
            || world_pos.y < 0
            || world_pos.z < 0
            || world_pos.x >= self.wfc_width as i32
            || world_pos.y >= self.wfc_depth as i32
            || world_pos.z >= self.wfc_height as i32
        {
            return BlockId::AIR;
        }

        let tile = self.grid.tile_at(UVec3::new(
            world_pos.x as u32,
            world_pos.z as u32,
            world_pos.y as u32,
        ));

        match tile.map(|t| t.0) {
            Some(AIR) => BlockId::AIR,
            Some(STONE) => BlockId::SOLID,
            Some(CAP) => BlockId::SOLID_ALT,
            Some(FLOOR_TILE) => BlockId::SOLID_ACCENT,
            _ => BlockId::AIR,
        }
    }
}

fn build_ruleset() -> WfcRuleset {
    let air = WfcTileId(AIR);
    let stone = WfcTileId(STONE);
    let cap = WfcTileId(CAP);
    let floor = WfcTileId(FLOOR_TILE);

    WfcRuleset::new(
        WfcTopology::Cartesian3d,
        vec![
            WfcTileDefinition::new(air, 3.0, "Air"),
            WfcTileDefinition::new(stone, 2.0, "Stone"),
            WfcTileDefinition::new(cap, 1.0, "Cap"),
            WfcTileDefinition::new(floor, 2.0, "Floor"),
        ],
    )
    // Air neighbors
    .with_rule(air, WfcDirection::XPos, [air, stone, cap, floor])
    .with_rule(air, WfcDirection::XNeg, [air, stone, cap, floor])
    .with_rule(air, WfcDirection::YPos, [air, stone, cap, floor])
    .with_rule(air, WfcDirection::YNeg, [air, stone, cap, floor])
    .with_rule(air, WfcDirection::ZPos, [air, cap])
    .with_rule(air, WfcDirection::ZNeg, [air, stone, cap, floor])
    // Stone: horizontal neighbors are stone or air, above is stone or cap, below is stone or floor
    .with_rule(stone, WfcDirection::XPos, [stone, air])
    .with_rule(stone, WfcDirection::XNeg, [stone, air])
    .with_rule(stone, WfcDirection::YPos, [stone, air])
    .with_rule(stone, WfcDirection::YNeg, [stone, air])
    .with_rule(stone, WfcDirection::ZPos, [stone, cap])
    .with_rule(stone, WfcDirection::ZNeg, [stone, floor])
    // Cap: sits on top of stone, air above, horizontal neighbors are cap or air
    .with_rule(cap, WfcDirection::XPos, [cap, air])
    .with_rule(cap, WfcDirection::XNeg, [cap, air])
    .with_rule(cap, WfcDirection::YPos, [cap, air])
    .with_rule(cap, WfcDirection::YNeg, [cap, air])
    .with_rule(cap, WfcDirection::ZPos, [air])
    .with_rule(cap, WfcDirection::ZNeg, [stone])
    // Floor: sits below stone, below is floor or air, horizontal neighbors are floor or air
    .with_rule(floor, WfcDirection::XPos, [floor, air])
    .with_rule(floor, WfcDirection::XNeg, [floor, air])
    .with_rule(floor, WfcDirection::YPos, [floor, air])
    .with_rule(floor, WfcDirection::YNeg, [floor, air])
    .with_rule(floor, WfcDirection::ZPos, [stone])
    .with_rule(floor, WfcDirection::ZNeg, [air])
}

fn solve_structure(seed: u64) -> Option<WfcTileGrid> {
    let request = WfcRequest::new(
        WfcGridSize::new_3d(WFC_SIZE, WFC_SIZE, WFC_HEIGHT),
        build_ruleset(),
        WfcSeed(seed),
    );
    solve_wfc(&request).ok().map(|s| s.grid)
}

#[derive(Component)]
struct OverlayText;

fn main() {
    let config = WfcVoxelConfig::default();
    let grid = solve_structure(config.seed).expect("initial WFC solve should succeed");

    let sampler = WfcBlockSampler {
        grid: Arc::new(grid),
        wfc_width: WFC_SIZE,
        wfc_height: WFC_SIZE,
        wfc_depth: WFC_HEIGHT,
    };

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.55, 0.65, 0.82)));
    app.insert_resource(config);
    app.init_resource::<WfcVoxelPane>();
    app.insert_resource(VoxelWorldGenerator::new(sampler));
    app.insert_resource(VoxelWorldConfig {
        chunk_dims: UVec3::splat(16),
        request_radius: 3,
        keep_radius: 4,
        seed: 42,
        ..default()
    });
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "wfc voxel_wfc".into(),
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
        VoxelWorldPlugin::default(),
    ));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::VoxelWfcE2EPlugin);
    app.register_pane::<WfcVoxelPane>();
    app.add_systems(Startup, setup);
    app.add_systems(Update, (sync_pane, rotate_camera));
    app.run();
}

fn setup(mut commands: Commands) {
    let center = Vec3::new(
        WFC_SIZE as f32 * 0.5,
        WFC_HEIGHT as f32 * 0.6,
        WFC_SIZE as f32 * 0.5,
    );

    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        ChunkViewer,
        ChunkViewerSettings {
            request_radius: 3,
            keep_radius: 4,
            priority: 10,
        },
        Transform::from_translation(center + Vec3::new(20.0, 15.0, 20.0))
            .looking_at(center, Vec3::Y),
    ));

    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(20.0, 30.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Name::new("Ambient"),
        AmbientLight {
            color: Color::srgb(0.7, 0.75, 0.85),
            brightness: 200.0,
            affects_lightmapped_meshes: false,
        },
    ));

    commands.spawn((
        Name::new("Overlay"),
        OverlayText,
        Text::new(format!(
            "wfc voxel_wfc\nWFC 3D {}x{}x{}\nAuto-orbiting camera",
            WFC_SIZE, WFC_SIZE, WFC_HEIGHT,
        )),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            width: px(300),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.88)),
    ));
}

fn sync_pane(
    pane: Res<WfcVoxelPane>,
    mut config: ResMut<WfcVoxelConfig>,
    mut generator: ResMut<VoxelWorldGenerator>,
    mut overlays: Query<&mut Text, With<OverlayText>>,
) {
    let next_seed = pane.seed as u64;
    if config.seed == next_seed {
        return;
    }
    config.seed = next_seed;

    let Some(grid) = solve_structure(next_seed) else {
        for mut overlay in &mut overlays {
            *overlay = Text::new(format!(
                "wfc voxel_wfc\nseed: {} FAILED",
                next_seed,
            ));
        }
        return;
    };

    let sampler = WfcBlockSampler {
        grid: Arc::new(grid),
        wfc_width: WFC_SIZE,
        wfc_height: WFC_SIZE,
        wfc_depth: WFC_HEIGHT,
    };
    *generator = VoxelWorldGenerator::new(sampler);

    for mut overlay in &mut overlays {
        *overlay = Text::new(format!(
            "wfc voxel_wfc\nseed: {}\nWFC 3D {}x{}x{}\nAuto-orbiting camera",
            next_seed, WFC_SIZE, WFC_SIZE, WFC_HEIGHT,
        ));
    }
}

fn rotate_camera(time: Res<Time>, mut cameras: Query<&mut Transform, With<Camera3d>>) {
    let center = Vec3::new(
        WFC_SIZE as f32 * 0.5,
        WFC_HEIGHT as f32 * 0.3,
        WFC_SIZE as f32 * 0.5,
    );
    let angle = time.elapsed_secs() * 0.15;
    let radius = WFC_SIZE as f32 * 1.0;
    let eye = center + Vec3::new(angle.cos() * radius, WFC_HEIGHT as f32 * 0.8, angle.sin() * radius);

    for mut transform in &mut cameras {
        *transform = Transform::from_translation(eye).looking_at(center, Vec3::Y);
    }
}
