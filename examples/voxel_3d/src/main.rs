use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcTileDefinition, WfcTileId,
    WfcTopology, solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// 3D voxel tileset with 6-direction adjacency. Three tile types form
// vertical structures: stone pillars capped by a decorative top.
// ---------------------------------------------------------------------------

fn voxel_request(seed: u64, width: u32, height: u32, depth: u32) -> WfcRequest {
    let air = WfcTileId(0);
    let stone = WfcTileId(1);
    let cap = WfcTileId(2);

    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian3d,
        vec![
            WfcTileDefinition::new(air, 3.0, "Air"),
            WfcTileDefinition::new(stone, 2.0, "Stone"),
            WfcTileDefinition::new(cap, 1.0, "Cap"),
        ],
    )
    // Air can neighbor anything laterally and above; below only air or cap.
    .with_rule(air, WfcDirection::XPos, [air, stone, cap])
    .with_rule(air, WfcDirection::XNeg, [air, stone, cap])
    .with_rule(air, WfcDirection::YPos, [air, stone, cap])
    .with_rule(air, WfcDirection::YNeg, [air, stone, cap])
    .with_rule(air, WfcDirection::ZPos, [air, cap])
    .with_rule(air, WfcDirection::ZNeg, [air, stone, cap])
    // Stone forms pillars: only stone or air laterally, stone or cap above, stone below.
    .with_rule(stone, WfcDirection::XPos, [stone, air])
    .with_rule(stone, WfcDirection::XNeg, [stone, air])
    .with_rule(stone, WfcDirection::YPos, [stone, air])
    .with_rule(stone, WfcDirection::YNeg, [stone, air])
    .with_rule(stone, WfcDirection::ZPos, [stone, cap])
    .with_rule(stone, WfcDirection::ZNeg, [stone])
    // Cap only appears on top of stone and has air above.
    .with_rule(cap, WfcDirection::XPos, [cap, air])
    .with_rule(cap, WfcDirection::XNeg, [cap, air])
    .with_rule(cap, WfcDirection::YPos, [cap, air])
    .with_rule(cap, WfcDirection::YNeg, [cap, air])
    .with_rule(cap, WfcDirection::ZPos, [air])
    .with_rule(cap, WfcDirection::ZNeg, [stone]);

    WfcRequest::new(
        WfcGridSize::new_3d(width, height, depth),
        ruleset,
        WfcSeed(seed),
    )
}

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct VoxelConfig {
    seed: u64,
    width: u32,
    height: u32,
    depth: u32,
}

impl Default for VoxelConfig {
    fn default() -> Self {
        Self {
            seed: 31,
            width: 10,
            height: 10,
            depth: 6,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Voxel 3D", position = "top-right")]
struct VoxelPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 8.0, max = 14.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 14.0, step = 1.0)]
    height: u32,
    #[pane(slider, min = 4.0, max = 8.0, step = 1.0)]
    depth: u32,
}

impl Default for VoxelPane {
    fn default() -> Self {
        Self {
            seed: 31,
            width: 10,
            height: 10,
            depth: 6,
        }
    }
}

#[derive(Resource, Default)]
struct CurrentSolution(Option<saddle_procgen_wfc::WfcSolution>);

#[derive(Component)]
struct VoxelRoot;

#[derive(Component)]
struct VoxelOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.03, 0.035, 0.05)));
    app.init_resource::<VoxelConfig>();
    app.init_resource::<VoxelPane>();
    app.init_resource::<CurrentSolution>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc voxel_3d".into(),
            resolution: (1360, 860).into(),
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
    app.register_pane::<VoxelPane>();
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
    commands.spawn((
        Name::new("Voxel Camera"),
        Camera3d::default(),
        Transform::from_xyz(16.0, 16.0, 18.0).looking_at(Vec3::new(4.0, 4.0, 2.0), Vec3::Z),
    ));
    commands.spawn((
        Name::new("Voxel Light"),
        DirectionalLight {
            illuminance: 35_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.1, -0.8, 0.0)),
    ));
    commands.spawn((
        Name::new("Voxel Overlay"),
        VoxelOverlay,
        Text::new("voxel_3d"),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            width: px(360),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.82)),
    ));
}

fn sync_pane_to_config(pane: Res<VoxelPane>, mut config: ResMut<VoxelConfig>) {
    let next = VoxelConfig {
        seed: pane.seed as u64,
        width: pane.width.max(4),
        height: pane.height.max(4),
        depth: pane.depth.max(2),
    };
    if *config != next {
        *config = next;
    }
}

fn regenerate_solution(config: Res<VoxelConfig>, mut solution: ResMut<CurrentSolution>) {
    if !config.is_changed() && solution.0.is_some() {
        return;
    }
    let request = voxel_request(config.seed, config.width, config.height, config.depth);
    solution.0 = Some(solve_wfc(&request).expect("voxel request should solve"));
}

fn render_solution(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    solution: Res<CurrentSolution>,
    roots: Query<Entity, With<VoxelRoot>>,
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

    let cube = meshes.add(Cuboid::new(0.92, 0.92, 0.92));
    let air = WfcTileId(0);
    commands
        .spawn((
            VoxelRoot,
            common::spatial_root("Voxel Root", Transform::from_xyz(-4.5, -4.5, 0.0)),
        ))
        .with_children(|parent| {
            for z in 0..solution.grid.size.depth {
                for y in 0..solution.grid.size.height {
                    for x in 0..solution.grid.size.width {
                        let tile = solution
                            .grid
                            .tile_at(UVec3::new(x, y, z))
                            .expect("tile should exist");
                        if tile == air {
                            continue;
                        }
                        parent.spawn((
                            Mesh3d(cube.clone()),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: common::color_for_tile_3d(tile),
                                perceptual_roughness: 0.92,
                                ..default()
                            })),
                            Transform::from_xyz(x as f32, y as f32, z as f32),
                        ));
                    }
                }
            }
        });
}

fn update_overlay(
    solution: Res<CurrentSolution>,
    mut overlay: Single<&mut Text, With<VoxelOverlay>>,
) {
    if !solution.is_changed() {
        return;
    }
    let Some(solution) = &solution.0 else {
        return;
    };
    **overlay = Text::new(format!(
        "voxel_3d\nsignature: {}\nsize: {}x{}x{}\nnon-air voxels render as cubes",
        solution.signature,
        solution.grid.size.width,
        solution.grid.size.height,
        solution.grid.size.depth
    ));
}
