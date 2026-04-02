use saddle_procgen_wfc_example_support as common;

use bevy::prelude::*;
use common::{color_for_tile_3d, install_auto_exit, spatial_root, voxel_request};
use saddle_procgen_wfc::solve_wfc;

fn main() {
    let solution = solve_wfc(&voxel_request(31)).expect("voxel request should solve");

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.03, 0.035, 0.05)));
    app.insert_resource(solution);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc voxel_3d".into(),
            resolution: (1360, 860).into(),
            ..default()
        }),
        ..default()
    }));
    install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    solution: Res<saddle_procgen_wfc::WfcSolution>,
) {
    commands.spawn((
        Name::new("Voxel Camera"),
        Camera3d::default(),
        Transform::from_xyz(16.0, 16.0, 18.0).looking_at(Vec3::new(4.0, 4.0, 2.0), Vec3::Z),
    ));
    commands.spawn((
        Name::new("Voxel Light"),
        DirectionalLight {
            illuminance: 35_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.1, -0.8, 0.0)),
    ));
    commands.spawn((
        Name::new("Voxel Overlay"),
        Text::new(format!(
            "voxel_3d\nsignature: {}\nnon-air voxels render as cubes",
            solution.signature
        )),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));

    let cube = meshes.add(Cuboid::new(0.92, 0.92, 0.92));
    let air = saddle_procgen_wfc::WfcTileId(0);
    commands
        .spawn(spatial_root(
            "Voxel Root",
            Transform::from_xyz(-4.5, -4.5, 0.0),
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
                                base_color: color_for_tile_3d(tile),
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
