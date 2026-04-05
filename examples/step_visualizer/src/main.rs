use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_procgen_wfc::{
    WfcDirection, WfcGridSize, WfcRequest, WfcRuleset, WfcSeed, WfcStepSnapshot, WfcStepSolver,
    WfcTileDefinition, WfcTileId, WfcTopology,
};
#[path = "../../shared/support.rs"]
mod common;

// ---------------------------------------------------------------------------
// Watch WFC solve one observation at a time. Each frame collapses one cell
// and redraws the grid so you can see propagation ripple outward.
// ---------------------------------------------------------------------------

fn step_request(seed: u64, width: u32, height: u32) -> WfcRequest {
    let meadow = WfcTileId(0);
    let road = WfcTileId(1);
    let water = WfcTileId(2);
    let all = [meadow, road, water];

    let ruleset = WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(meadow, 5.0, "Meadow"),
            WfcTileDefinition::new(road, 2.0, "Road"),
            WfcTileDefinition::new(water, 1.0, "Water"),
        ],
    )
    .with_rule(meadow, WfcDirection::XPos, all)
    .with_rule(meadow, WfcDirection::XNeg, all)
    .with_rule(meadow, WfcDirection::YPos, all)
    .with_rule(meadow, WfcDirection::YNeg, all)
    .with_rule(road, WfcDirection::XPos, [meadow, road])
    .with_rule(road, WfcDirection::XNeg, [meadow, road])
    .with_rule(road, WfcDirection::YPos, [meadow, road])
    .with_rule(road, WfcDirection::YNeg, [meadow, road])
    .with_rule(water, WfcDirection::XPos, [meadow, water])
    .with_rule(water, WfcDirection::XNeg, [meadow, water])
    .with_rule(water, WfcDirection::YPos, [meadow, water])
    .with_rule(water, WfcDirection::YNeg, [meadow, water]);

    WfcRequest::new(WfcGridSize::new_2d(width, height), ruleset, WfcSeed(seed))
}

// ---------------------------------------------------------------------------

#[derive(Resource, Clone, PartialEq)]
struct StepConfig {
    seed: u64,
    width: u32,
    height: u32,
    speed: u32,
}

impl Default for StepConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 20,
            height: 14,
            speed: 1,
        }
    }
}

#[derive(Resource, Pane)]
#[pane(title = "Step Visualizer", position = "top-right")]
struct StepPane {
    #[pane(number, min = 0.0, step = 1.0)]
    seed: u32,
    #[pane(slider, min = 10.0, max = 28.0, step = 1.0)]
    width: u32,
    #[pane(slider, min = 8.0, max = 20.0, step = 1.0)]
    height: u32,
    #[pane(slider, min = 1.0, max = 10.0, step = 1.0)]
    speed: u32,
}

impl Default for StepPane {
    fn default() -> Self {
        Self {
            seed: 42,
            width: 20,
            height: 14,
            speed: 1,
        }
    }
}

/// Holds the active step solver and latest snapshot.
#[derive(Resource)]
struct SolverState {
    // Uses an Option<Box> to avoid storing a non-'static reference.
    // The solver owns a clone of the request, so the borrow is self-referential
    // -- we work around this by storing the request alongside.
    request: WfcRequest,
    snapshot: Option<WfcStepSnapshot>,
    finished: bool,
    failed: bool,
}

impl SolverState {
    fn new(config: &StepConfig) -> Self {
        Self {
            request: step_request(config.seed, config.width, config.height),
            snapshot: None,
            finished: false,
            failed: false,
        }
    }
}

/// We need the solver to live across frames but it borrows `WfcRequest`.
/// We work around this by re-creating the solver every time we reset, and
/// stepping it synchronously (one observation per frame is cheap).
#[derive(Resource)]
struct StepSolverHolder {
    /// Boxed solver referencing a heap-allocated request.
    inner: Option<Box<StepSolverOwned>>,
}

struct StepSolverOwned {
    /// Heap-pinned request that the solver borrows from.
    #[allow(dead_code)]
    request: Box<WfcRequest>,
    // SAFETY: the solver borrows from request which is heap-pinned and never
    // moved while the solver is alive. Both are dropped together.
    solver: Option<WfcStepSolver<'static>>,
}

impl StepSolverOwned {
    #[allow(clippy::result_large_err)]
    fn new(request: WfcRequest) -> Result<Self, saddle_procgen_wfc::WfcFailure> {
        let request = Box::new(request);
        // SAFETY: we extend the lifetime of the borrow. The request is pinned
        // on the heap and will not be dropped before the solver.
        let solver = unsafe {
            let req_ref: &WfcRequest = &request;
            let req_static: &'static WfcRequest = std::mem::transmute(req_ref);
            WfcStepSolver::new(req_static)?
        };
        Ok(Self {
            request,
            solver: Some(solver),
        })
    }
}

impl Drop for StepSolverOwned {
    fn drop(&mut self) {
        // Drop solver first (it borrows request).
        self.solver = None;
        // request drops automatically.
    }
}

#[derive(Component)]
struct StepGridRoot;

#[derive(Component)]
struct StepOverlay;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.07)));
    app.init_resource::<StepConfig>();
    app.init_resource::<StepPane>();
    app.insert_resource(StepSolverHolder { inner: None });
    app.insert_resource(SolverState::new(&StepConfig::default()));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc step_visualizer".into(),
            resolution: (1360, 900).into(),
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
    app.register_pane::<StepPane>();
    common::install_auto_exit(&mut app);
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_config,
            reset_solver_on_config_change,
            advance_solver,
            render_grid,
            update_overlay,
        )
            .chain(),
    );
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Step Visualizer Camera"), Camera2d));
    commands.spawn((
        Name::new("Step Visualizer Overlay"),
        StepOverlay,
        Text::new("step_visualizer\nPress SPACE to pause/resume, R to restart"),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(420),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.86)),
    ));
}

fn sync_pane_to_config(pane: Res<StepPane>, mut config: ResMut<StepConfig>) {
    let next = StepConfig {
        seed: pane.seed as u64,
        width: pane.width.max(6),
        height: pane.height.max(6),
        speed: pane.speed.max(1),
    };
    if *config != next {
        *config = next;
    }
}

fn reset_solver_on_config_change(
    config: Res<StepConfig>,
    keys: Res<ButtonInput<KeyCode>>,
    mut holder: ResMut<StepSolverHolder>,
    mut state: ResMut<SolverState>,
) {
    let needs_reset = config.is_changed() || keys.just_pressed(KeyCode::KeyR);
    if !needs_reset {
        return;
    }

    *state = SolverState::new(&config);
    match StepSolverOwned::new(state.request.clone()) {
        Ok(owned) => holder.inner = Some(Box::new(owned)),
        Err(_) => {
            holder.inner = None;
            state.failed = true;
        }
    }
}

fn advance_solver(
    keys: Res<ButtonInput<KeyCode>>,
    mut holder: ResMut<StepSolverHolder>,
    mut state: ResMut<SolverState>,
    config: Res<StepConfig>,
    mut paused: Local<bool>,
) {
    if keys.just_pressed(KeyCode::Space) {
        *paused = !*paused;
    }
    if *paused || state.finished || state.failed {
        return;
    }

    let Some(owned) = holder.inner.as_mut() else {
        return;
    };
    let Some(solver) = owned.solver.as_mut() else {
        return;
    };

    for _ in 0..config.speed {
        if state.finished {
            break;
        }
        match solver.step() {
            Ok(snapshot) => {
                state.finished = snapshot.finished;
                state.snapshot = Some(snapshot);
            }
            Err(_failure) => {
                state.failed = true;
                break;
            }
        }
    }
}

fn render_grid(
    mut commands: Commands,
    state: Res<SolverState>,
    roots: Query<Entity, With<StepGridRoot>>,
) {
    if !state.is_changed() {
        return;
    }
    let Some(snapshot) = &state.snapshot else {
        return;
    };

    for entity in &roots {
        commands.entity(entity).despawn();
    }

    let width = state.request.grid_size.width;
    let height = state.request.grid_size.height;
    let tile_size = (980.0 / width as f32)
        .min(700.0 / height as f32)
        .clamp(16.0, 50.0);
    let origin = Vec2::new(
        -(width as f32 * tile_size) * 0.5 + tile_size * 0.5,
        -(height as f32 * tile_size) * 0.5 + tile_size * 0.5,
    );

    commands
        .spawn((
            StepGridRoot,
            common::spatial_root("Step Grid", Transform::default()),
        ))
        .with_children(|parent| {
            for (index, cell) in snapshot.cells.iter().enumerate() {
                let x = (index % width as usize) as u32;
                let y = (index / width as usize) as u32;

                let is_last_observed = snapshot
                    .last_observed_position
                    .is_some_and(|pos| pos.x == x && pos.y == y);

                let color = cell_color(cell, is_last_observed);
                parent.spawn((
                    Sprite::from_color(color, Vec2::splat(tile_size - 2.0)),
                    Transform::from_xyz(
                        origin.x + x as f32 * tile_size,
                        origin.y + y as f32 * tile_size,
                        0.0,
                    ),
                ));

                // Show possibility count for uncollapsed cells.
                if cell.collapsed.is_none() && cell.possible_count > 0 {
                    parent.spawn((
                        Text2d::new(format!("{}", cell.possible_count)),
                        TextFont::from_font_size((tile_size * 0.38).clamp(10.0, 18.0)),
                        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
                        Transform::from_xyz(
                            origin.x + x as f32 * tile_size,
                            origin.y + y as f32 * tile_size,
                            1.0,
                        ),
                    ));
                }
            }
        });
}

fn cell_color(cell: &saddle_procgen_wfc::WfcStepCell, is_last_observed: bool) -> Color {
    if let Some(variant) = &cell.collapsed {
        // Collapsed cell -- use tile color.
        let base = common::color_for_tile_2d(variant.tile);
        if is_last_observed {
            // Brighten the last observed cell.
            let [r, g, b, a] = base.to_srgba().to_f32_array();
            Color::srgba(
                (r + 0.25).min(1.0),
                (g + 0.25).min(1.0),
                (b + 0.25).min(1.0),
                a,
            )
        } else {
            base
        }
    } else if cell.possible_count == 0 {
        // Contradiction.
        Color::srgb(0.92, 0.15, 0.2)
    } else {
        // Uncollapsed -- color by entropy (fewer possibilities = more saturated).
        let intensity = (cell.possible_count as f32 / 3.0).clamp(0.0, 1.0);
        if is_last_observed {
            Color::srgb(0.9, 0.8, 0.3)
        } else {
            Color::srgb(
                0.12 + intensity * 0.15,
                0.14 + intensity * 0.06,
                0.22 + intensity * 0.18,
            )
        }
    }
}

fn update_overlay(state: Res<SolverState>, mut overlay: Single<&mut Text, With<StepOverlay>>) {
    if !state.is_changed() {
        return;
    }
    let obs = state
        .snapshot
        .as_ref()
        .map(|s| s.observation_count)
        .unwrap_or(0);
    let status = if state.failed {
        "FAILED"
    } else if state.finished {
        "COMPLETE"
    } else {
        "solving..."
    };
    **overlay = Text::new(format!(
        "step_visualizer\nWatch WFC solve one cell at a time\n\nstatus: {status}\nobservations: {obs}\nsize: {}x{}\nseed: {}\n\nSPACE pause/resume | R restart\nAdjust speed, seed, and grid in the pane",
        state.request.grid_size.width, state.request.grid_size.height, state.request.seed.0,
    ));
}
