use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcDirection, WfcFixedCell, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcSolution,
    WfcTileDefinition, WfcTileId, WfcTopology, solve_wfc,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Click cells to pin tiles, then WFC fills the rest around your placements.
// Left-click cycles through tile types. Right-click clears a pinned cell.
// ---------------------------------------------------------------------------

const TILE_MEADOW: WfcTileId = WfcTileId(0);
const TILE_ROAD: WfcTileId = WfcTileId(1);
const TILE_WATER: WfcTileId = WfcTileId(2);

const GRID_WIDTH: u32 = 20;
const GRID_HEIGHT: u32 = 14;

fn interactive_ruleset() -> WfcRuleset {
    let all = [TILE_MEADOW, TILE_ROAD, TILE_WATER];
    WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(TILE_MEADOW, 5.0, "Meadow"),
            WfcTileDefinition::new(TILE_ROAD, 2.0, "Road"),
            WfcTileDefinition::new(TILE_WATER, 1.0, "Water"),
        ],
    )
    .with_rule(TILE_MEADOW, WfcDirection::XPos, all)
    .with_rule(TILE_MEADOW, WfcDirection::XNeg, all)
    .with_rule(TILE_MEADOW, WfcDirection::YPos, all)
    .with_rule(TILE_MEADOW, WfcDirection::YNeg, all)
    .with_rule(TILE_ROAD, WfcDirection::XPos, [TILE_MEADOW, TILE_ROAD])
    .with_rule(TILE_ROAD, WfcDirection::XNeg, [TILE_MEADOW, TILE_ROAD])
    .with_rule(TILE_ROAD, WfcDirection::YPos, [TILE_MEADOW, TILE_ROAD])
    .with_rule(TILE_ROAD, WfcDirection::YNeg, [TILE_MEADOW, TILE_ROAD])
    .with_rule(TILE_WATER, WfcDirection::XPos, [TILE_MEADOW, TILE_WATER])
    .with_rule(TILE_WATER, WfcDirection::XNeg, [TILE_MEADOW, TILE_WATER])
    .with_rule(TILE_WATER, WfcDirection::YPos, [TILE_MEADOW, TILE_WATER])
    .with_rule(TILE_WATER, WfcDirection::YNeg, [TILE_MEADOW, TILE_WATER])
}

// ---------------------------------------------------------------------------

#[derive(Resource)]
struct PinnedCells(Vec<Option<WfcTileId>>);

impl Default for PinnedCells {
    fn default() -> Self {
        Self(vec![None; (GRID_WIDTH * GRID_HEIGHT) as usize])
    }
}

impl PinnedCells {
    fn get(&self, x: u32, y: u32) -> Option<WfcTileId> {
        let index = (y * GRID_WIDTH + x) as usize;
        self.0.get(index).copied().flatten()
    }

    fn set(&mut self, x: u32, y: u32, tile: Option<WfcTileId>) {
        let index = (y * GRID_WIDTH + x) as usize;
        if let Some(slot) = self.0.get_mut(index) {
            *slot = tile;
        }
    }

    fn to_fixed_cells(&self) -> Vec<WfcFixedCell> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(index, tile)| {
                let tile = (*tile)?;
                let x = (index % GRID_WIDTH as usize) as u32;
                let y = (index / GRID_WIDTH as usize) as u32;
                Some(WfcFixedCell::new(UVec3::new(x, y, 0), tile))
            })
            .collect()
    }
}

#[derive(Resource, Clone, PartialEq)]
struct InteractiveConfig {
    seed: u64,
}

impl Default for InteractiveConfig {
    fn default() -> Self {
        Self { seed: 13 }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Interactive", position = "top-right")]
struct InteractivePane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
}

impl Default for InteractivePane {
    fn default() -> Self {
        Self { seed: 13 }
    }
}

#[derive(Resource, Default)]
struct CurrentSolution(Option<WfcSolution>);

/// Tracks when the grid needs re-solving.
#[derive(Resource, Default)]
struct NeedsSolve(bool);

#[derive(Component)]
struct InteractiveGridRoot;

#[derive(Component)]
struct InteractiveOverlay;

#[derive(Component)]
struct PinMarker;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.05, 0.06, 0.08)));
    app.init_resource::<PinnedCells>();
    app.init_resource::<InteractiveConfig>();
    app.init_resource::<InteractivePane>();
    app.init_resource::<CurrentSolution>();
    app.insert_resource(NeedsSolve(true));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc interactive".into(),
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
    app.register_pane::<InteractivePane>();
    common::install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
            handle_input,
            regenerate_solution,
            render_solution,
            update_overlay,
        )
            .chain(),
    );
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Interactive Camera"), Camera2d));
    commands.spawn((
        Name::new("Interactive Overlay"),
        InteractiveOverlay,
        Text::new("interactive"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(420),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.07, 0.10, 0.86)),
    ));
}

fn sync_pane_to_config(
    pane: Res<InteractivePane>,
    mut config: ResMut<InteractiveConfig>,
    mut needs_solve: ResMut<NeedsSolve>,
) {
    let next = InteractiveConfig {
        seed: pane.seed as u64,
    };
    if *config != next {
        *config = next;
        needs_solve.0 = true;
    }
}

fn tile_size() -> f32 {
    (980.0 / GRID_WIDTH as f32)
        .min(700.0 / GRID_HEIGHT as f32)
        .clamp(16.0, 50.0)
}

fn grid_origin() -> Vec2 {
    let ts = tile_size();
    Vec2::new(
        -(GRID_WIDTH as f32 * ts) * 0.5 + ts * 0.5,
        -(GRID_HEIGHT as f32 * ts) * 0.5 + ts * 0.5,
    )
}

fn handle_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut pinned: ResMut<PinnedCells>,
    mut needs_solve: ResMut<NeedsSolve>,
) {
    // C clears all pins.
    if keys.just_pressed(KeyCode::KeyC) {
        *pinned = PinnedCells::default();
        needs_solve.0 = true;
        return;
    }

    let left_click = mouse.just_pressed(MouseButton::Left);
    let right_click = mouse.just_pressed(MouseButton::Right);
    if !left_click && !right_click {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };

    let ts = tile_size();
    let origin = grid_origin();
    let grid_x = ((world_pos.x - origin.x + ts * 0.5) / ts).floor() as i32;
    let grid_y = ((world_pos.y - origin.y + ts * 0.5) / ts).floor() as i32;

    if grid_x < 0 || grid_y < 0 || grid_x >= GRID_WIDTH as i32 || grid_y >= GRID_HEIGHT as i32 {
        return;
    }

    let (x, y) = (grid_x as u32, grid_y as u32);

    if right_click {
        pinned.set(x, y, None);
    } else {
        // Left click cycles: None -> Meadow -> Road -> Water -> None.
        let current = pinned.get(x, y);
        let next = match current {
            None => Some(TILE_MEADOW),
            Some(t) if t == TILE_MEADOW => Some(TILE_ROAD),
            Some(t) if t == TILE_ROAD => Some(TILE_WATER),
            _ => None,
        };
        pinned.set(x, y, next);
    }

    needs_solve.0 = true;
}

fn regenerate_solution(
    config: Res<InteractiveConfig>,
    pinned: Res<PinnedCells>,
    mut solution: ResMut<CurrentSolution>,
    mut needs_solve: ResMut<NeedsSolve>,
) {
    if !needs_solve.0 {
        return;
    }
    needs_solve.0 = false;

    let mut request = WfcRequest::new(
        WfcGridSize::new_2d(GRID_WIDTH, GRID_HEIGHT),
        interactive_ruleset(),
        WfcSeed(config.seed),
    );
    request.fixed_cells = pinned.to_fixed_cells();
    request.settings.max_backtracks = 512;

    match solve_wfc(&request) {
        Ok(sol) => solution.0 = Some(sol),
        Err(_) => solution.0 = None,
    }
}

fn render_solution(
    mut commands: Commands,
    solution: Res<CurrentSolution>,
    pinned: Res<PinnedCells>,
    roots: Query<Entity, With<InteractiveGridRoot>>,
) {
    if !solution.is_changed() && !pinned.is_changed() {
        return;
    }

    for entity in &roots {
        commands.entity(entity).despawn();
    }

    let ts = tile_size();
    let origin = grid_origin();

    commands
        .spawn((
            InteractiveGridRoot,
            common::spatial_root("Interactive Grid", Transform::default()),
        ))
        .with_children(|parent| {
            for y in 0..GRID_HEIGHT {
                for x in 0..GRID_WIDTH {
                    let is_pinned = pinned.get(x, y).is_some();

                    let color = if let Some(sol) = &solution.0 {
                        let tile = sol
                            .grid
                            .tile_at(UVec3::new(x, y, 0))
                            .expect("tile should exist");
                        common::color_for_tile_2d(tile)
                    } else {
                        // Failed solve -- show grey.
                        Color::srgb(0.25, 0.25, 0.25)
                    };

                    parent.spawn((
                        Sprite::from_color(color, Vec2::splat(ts - 2.0)),
                        Transform::from_xyz(
                            origin.x + x as f32 * ts,
                            origin.y + y as f32 * ts,
                            0.0,
                        ),
                    ));

                    // Draw a pin marker on user-placed tiles.
                    if is_pinned {
                        parent.spawn((
                            PinMarker,
                            Sprite::from_color(
                                Color::srgba(1.0, 1.0, 1.0, 0.4),
                                Vec2::splat(ts * 0.3),
                            ),
                            Transform::from_xyz(
                                origin.x + x as f32 * ts,
                                origin.y + y as f32 * ts,
                                1.0,
                            ),
                        ));
                    }
                }
            }
        });
}

fn update_overlay(
    solution: Res<CurrentSolution>,
    pinned: Res<PinnedCells>,
    mut overlay: Single<&mut Text, With<InteractiveOverlay>>,
) {
    if !solution.is_changed() && !pinned.is_changed() {
        return;
    }

    let pin_count = pinned.0.iter().filter(|p| p.is_some()).count();
    let status = if solution.0.is_some() {
        "solved"
    } else {
        "CONTRADICTION -- try different pins"
    };
    let sig = solution.0.as_ref().map(|s| s.signature).unwrap_or(0);

    **overlay = Text::new(format!(
        "interactive\nClick to place tiles, WFC fills the rest\n\nstatus: {status}\npinned cells: {pin_count}\nsignature: {sig}\n\nLEFT CLICK cycle tile | RIGHT CLICK clear pin\nC clear all | Adjust seed in pane"
    ));
}
