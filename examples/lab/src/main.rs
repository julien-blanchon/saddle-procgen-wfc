#[cfg(feature = "e2e")]
mod e2e;

use saddle_procgen_wfc_example_support as common;

use bevy::prelude::*;
#[cfg(feature = "dev")]
use bevy::remote::{RemotePlugin, http::RemoteHttpPlugin};
#[cfg(feature = "dev")]
use bevy_brp_extras::BrpExtrasPlugin;

use common::{
    basic_request, color_for_tile_2d, constrained_room_request, contradiction_request,
    large_request, spatial_root,
};
use saddle_procgen_wfc::{
    GenerateWfc, WfcFailure, WfcFailureReason, WfcPlugin, WfcRuntimeDiagnostics, WfcSolved,
    WfcSystems,
};

const DEFAULT_BRP_PORT: u16 = 15_702;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Reflect)]
pub enum LabView {
    #[default]
    Basic,
    Room,
    Contradiction,
    Large,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Reflect)]
pub enum LabSolveState {
    #[default]
    Idle,
    Running,
    Solved,
    Failed,
}

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
struct LabControl {
    active_view: LabView,
    pending_view: Option<LabView>,
    seed: u64,
    expected_label: Option<String>,
    regenerate_requested: bool,
    started_once: bool,
}

impl Default for LabControl {
    fn default() -> Self {
        Self {
            active_view: LabView::Basic,
            pending_view: Some(LabView::Basic),
            seed: seed_for_view(LabView::Basic),
            expected_label: None,
            regenerate_requested: false,
            started_once: false,
        }
    }
}

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct LabDiagnostics {
    pub active_view: LabView,
    pub solve_state: LabSolveState,
    pub seed: u64,
    pub signature: u64,
    pub visible_cells: usize,
    pub zero_domain_cells: usize,
    pub highlighted_cells: usize,
    pub ambiguous_cells: usize,
    pub contradiction_position: Option<UVec3>,
    pub regeneration_count: u32,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub running_jobs: u64,
    pub recent_signatures: Vec<u64>,
    pub last_failure_reason: Option<WfcFailureReason>,
}

impl Default for LabDiagnostics {
    fn default() -> Self {
        Self {
            active_view: LabView::Basic,
            solve_state: LabSolveState::Idle,
            seed: seed_for_view(LabView::Basic),
            signature: 0,
            visible_cells: 0,
            zero_domain_cells: 0,
            highlighted_cells: 0,
            ambiguous_cells: 0,
            contradiction_position: None,
            regeneration_count: 0,
            completed_jobs: 0,
            failed_jobs: 0,
            running_jobs: 0,
            recent_signatures: Vec::new(),
            last_failure_reason: None,
        }
    }
}

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct BeforeSignature(pub u64);

#[derive(Component)]
struct LabOverlay;

#[derive(Component)]
struct LabVisualRoot;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.05, 0.055, 0.07)));
    app.init_resource::<LabControl>();
    app.init_resource::<LabDiagnostics>();
    app.init_resource::<BeforeSignature>();
    app.register_type::<LabView>();
    app.register_type::<LabSolveState>();
    app.register_type::<LabControl>();
    app.register_type::<LabDiagnostics>();
    app.register_type::<BeforeSignature>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "wfc_lab".into(),
            resolution: (1440, 920).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(WfcPlugin::default());
    #[cfg(feature = "dev")]
    app.add_plugins((
        RemotePlugin::default(),
        BrpExtrasPlugin::with_http_plugin(RemoteHttpPlugin::default().with_port(lab_brp_port())),
    ));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::WfcLabE2EPlugin);
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            handle_keyboard_input,
            apply_lab_requests.before(WfcSystems::Request),
            apply_solution.after(WfcSystems::ApplyResults),
            apply_failure.after(WfcSystems::ApplyResults),
            sync_runtime_diagnostics.after(WfcSystems::ApplyResults),
            update_overlay.after(WfcSystems::ApplyResults),
        ),
    );
    app.run();
}

#[cfg(feature = "dev")]
fn lab_brp_port() -> u16 {
    std::env::var("BRP_EXTRAS_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_BRP_PORT)
}

#[cfg(not(feature = "dev"))]
fn lab_brp_port() -> u16 {
    DEFAULT_BRP_PORT
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("WFC Lab Camera"), Camera2d));
    commands.spawn((
        Name::new("WFC Lab Overlay"),
        LabOverlay,
        Text::new("wfc_lab"),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            width: px(340),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.88)),
    ));
}

#[cfg(feature = "e2e")]
pub(crate) fn set_view(world: &mut World, view: LabView) {
    world.resource_mut::<LabControl>().pending_view = Some(view);
}

#[cfg(feature = "e2e")]
pub(crate) fn request_regeneration(world: &mut World) {
    world.resource_mut::<LabControl>().regenerate_requested = true;
}

fn handle_keyboard_input(keys: Res<ButtonInput<KeyCode>>, mut control: ResMut<LabControl>) {
    if keys.just_pressed(KeyCode::Digit1) {
        control.pending_view = Some(LabView::Basic);
    } else if keys.just_pressed(KeyCode::Digit2) {
        control.pending_view = Some(LabView::Room);
    } else if keys.just_pressed(KeyCode::Digit3) {
        control.pending_view = Some(LabView::Contradiction);
    } else if keys.just_pressed(KeyCode::Digit4) {
        control.pending_view = Some(LabView::Large);
    }

    if keys.just_pressed(KeyCode::Space) {
        control.regenerate_requested = true;
    }
}

fn apply_lab_requests(
    mut control: ResMut<LabControl>,
    mut diagnostics: ResMut<LabDiagnostics>,
    mut requests: MessageWriter<GenerateWfc>,
) {
    if let Some(view) = control.pending_view.take() {
        control.active_view = view;
        control.seed = seed_for_view(view);
        control.regenerate_requested = false;
        control.started_once = true;
        diagnostics.active_view = view;
        diagnostics.solve_state = LabSolveState::Running;
        diagnostics.seed = control.seed;
        let label = format!("{view:?} {}", control.seed);
        control.expected_label = Some(label.clone());
        requests.write(GenerateWfc {
            request: request_for_view(view, control.seed),
            label: Some(label),
        });
        return;
    }

    if control.regenerate_requested || !control.started_once {
        control.regenerate_requested = false;
        if control.started_once {
            control.seed = control.seed.saturating_add(1);
        } else {
            control.started_once = true;
        }
        diagnostics.solve_state = LabSolveState::Running;
        diagnostics.seed = control.seed;
        let label = format!("{:?} {}", control.active_view, control.seed);
        control.expected_label = Some(label.clone());
        requests.write(GenerateWfc {
            request: request_for_view(control.active_view, control.seed),
            label: Some(label),
        });
    }
}

fn apply_solution(
    mut commands: Commands,
    mut results: MessageReader<WfcSolved>,
    roots: Query<Entity, With<LabVisualRoot>>,
    control: Res<LabControl>,
    mut diagnostics: ResMut<LabDiagnostics>,
) {
    for solved in results.read() {
        if control.expected_label.as_deref() != Some(solved.label.as_str()) {
            continue;
        }

        for entity in &roots {
            commands.entity(entity).despawn();
        }

        diagnostics.solve_state = LabSolveState::Solved;
        diagnostics.signature = solved.solution.signature;
        diagnostics.visible_cells = solved.solution.grid.tiles.len();
        diagnostics.zero_domain_cells = 0;
        diagnostics.highlighted_cells = match diagnostics.active_view {
            LabView::Basic | LabView::Large => solved
                .solution
                .grid
                .tiles
                .iter()
                .filter(|tile| tile.0 == 2)
                .count(),
            LabView::Room => solved
                .solution
                .grid
                .tiles
                .iter()
                .filter(|tile| matches!(tile.0, 2 | 3))
                .count(),
            LabView::Contradiction => 0,
        };
        diagnostics.ambiguous_cells = 0;
        diagnostics.contradiction_position = None;
        diagnostics.last_failure_reason = None;
        diagnostics.regeneration_count = diagnostics.regeneration_count.saturating_add(1);
        diagnostics
            .recent_signatures
            .push(solved.solution.signature);
        if diagnostics.recent_signatures.len() > 12 {
            let drop = diagnostics.recent_signatures.len() - 12;
            diagnostics.recent_signatures.drain(0..drop);
        }

        commands
            .spawn((
                LabVisualRoot,
                spatial_root("WFC Lab Visual Root", Transform::default()),
            ))
            .with_children(|parent| {
                let (tile_size, origin) = layout_for_grid(solved.solution.grid.size);
                for y in 0..solved.solution.grid.size.height {
                    for x in 0..solved.solution.grid.size.width {
                        let tile = solved
                            .solution
                            .grid
                            .tile_at(UVec3::new(x, y, 0))
                            .expect("tile should exist");
                        let color = match tile.0 {
                            2 => Color::srgb(0.16, 0.74, 0.62),
                            3 => Color::srgb(0.9, 0.42, 0.2),
                            _ => color_for_tile_2d(tile),
                        };
                        parent.spawn((
                            Sprite::from_color(color, Vec2::splat(tile_size - 3.0)),
                            Transform::from_xyz(
                                origin.x + x as f32 * tile_size,
                                origin.y + y as f32 * tile_size,
                                0.0,
                            ),
                        ));
                        if let Some(marker) = marker_text_for_tile(control.active_view, tile) {
                            parent.spawn((
                                Text2d::new(marker),
                                TextFont::from_font_size((tile_size * 0.34).clamp(12.0, 22.0)),
                                TextColor(Color::WHITE),
                                Transform::from_xyz(
                                    origin.x + x as f32 * tile_size,
                                    origin.y + y as f32 * tile_size,
                                    1.0,
                                ),
                            ));
                        }
                    }
                }
            });
    }
}

fn apply_failure(
    mut commands: Commands,
    mut failures: MessageReader<saddle_procgen_wfc::WfcFailed>,
    roots: Query<Entity, With<LabVisualRoot>>,
    control: Res<LabControl>,
    mut diagnostics: ResMut<LabDiagnostics>,
) {
    for failed in failures.read() {
        if control.expected_label.as_deref() != Some(failed.label.as_str()) {
            continue;
        }

        for entity in &roots {
            commands.entity(entity).despawn();
        }
        render_failure(&mut commands, &failed.failure, &mut diagnostics);
    }
}

fn render_failure(commands: &mut Commands, failure: &WfcFailure, diagnostics: &mut LabDiagnostics) {
    diagnostics.solve_state = LabSolveState::Failed;
    diagnostics.signature = 0;
    diagnostics.last_failure_reason = Some(failure.reason.clone());
    diagnostics.contradiction_position = failure.contradiction.as_ref().map(|item| item.position);

    let Some(snapshot) = &failure.debug else {
        diagnostics.visible_cells = 0;
        diagnostics.zero_domain_cells = 0;
        diagnostics.highlighted_cells = 0;
        diagnostics.ambiguous_cells = 0;
        return;
    };

    diagnostics.visible_cells = snapshot.cells.len();
    diagnostics.zero_domain_cells = snapshot
        .cells
        .iter()
        .filter(|cell| cell.possible_count == 0)
        .count();
    diagnostics.highlighted_cells = 0;
    diagnostics.ambiguous_cells = snapshot
        .cells
        .iter()
        .filter(|cell| cell.possible_count > 1)
        .count();

    commands
        .spawn((
            LabVisualRoot,
            spatial_root("WFC Lab Visual Root", Transform::default()),
        ))
        .with_children(|parent| {
            let (tile_size, origin) = layout_for_grid(failure.grid_size);
            for cell in &snapshot.cells {
                let intensity = (cell.possible_count.max(1) as f32 / 4.0).clamp(0.0, 1.0);
                let color = if cell.possible_count == 0 {
                    Color::srgb(0.94, 0.2, 0.25)
                } else {
                    Color::srgb(0.15 + intensity * 0.55, 0.24, 0.7 - intensity * 0.28)
                };
                parent.spawn((
                    Sprite::from_color(color, Vec2::splat(tile_size - 3.0)),
                    Transform::from_xyz(
                        origin.x + cell.position.x as f32 * tile_size,
                        origin.y + cell.position.y as f32 * tile_size,
                        0.0,
                    ),
                ));
                parent.spawn((
                    Text2d::new(format!("{}", cell.possible_count)),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::WHITE),
                    Transform::from_xyz(
                        origin.x + cell.position.x as f32 * tile_size,
                        origin.y + cell.position.y as f32 * tile_size,
                        1.0,
                    ),
                ));
            }
            if let Some(contradiction) = &failure.contradiction {
                parent.spawn((
                    Text2d::new("X"),
                    TextFont::from_font_size((tile_size * 0.42).clamp(14.0, 24.0)),
                    TextColor(Color::srgb(1.0, 0.93, 0.45)),
                    Transform::from_xyz(
                        origin.x + contradiction.position.x as f32 * tile_size,
                        origin.y + contradiction.position.y as f32 * tile_size,
                        2.0,
                    ),
                ));
            }
        });
}

fn sync_runtime_diagnostics(
    runtime: Res<WfcRuntimeDiagnostics>,
    mut diagnostics: ResMut<LabDiagnostics>,
) {
    diagnostics.completed_jobs = runtime.completed_jobs;
    diagnostics.failed_jobs = runtime.failed_jobs;
    diagnostics.running_jobs = runtime.running_jobs;
}

fn update_overlay(
    diagnostics: Res<LabDiagnostics>,
    mut overlay: Single<&mut Text, With<LabOverlay>>,
) {
    if diagnostics.is_changed() {
        **overlay = Text::new(format!(
            "wfc_lab\nview: {:?}\nstate: {:?}\nseed: {}\nsignature: {}\nvisible cells: {}\nzero-domain cells: {}\n{}\nruntime jobs: running={} completed={} failed={}\nrecent signatures: {}\ncontradiction: {}\ncontrols: 1 basic 2 room 3 contradiction 4 large space regenerate",
            diagnostics.active_view,
            diagnostics.solve_state,
            diagnostics.seed,
            diagnostics.signature,
            diagnostics.visible_cells,
            diagnostics.zero_domain_cells,
            view_metric_line(&diagnostics),
            diagnostics.running_jobs,
            diagnostics.completed_jobs,
            diagnostics.failed_jobs,
            format_recent_signatures(&diagnostics.recent_signatures),
            format_position(diagnostics.contradiction_position),
        ));
    }
}

fn seed_for_view(view: LabView) -> u64 {
    match view {
        LabView::Basic => 7,
        LabView::Room => 19,
        LabView::Contradiction => 41,
        LabView::Large => 87,
    }
}

fn request_for_view(view: LabView, seed: u64) -> saddle_procgen_wfc::WfcRequest {
    match view {
        LabView::Basic => basic_request(seed),
        LabView::Room => constrained_room_request(seed),
        LabView::Contradiction => contradiction_request(seed),
        LabView::Large => large_request(seed),
    }
}

fn layout_for_grid(size: saddle_procgen_wfc::WfcGridSize) -> (f32, Vec2) {
    let tile_size = (980.0 / size.width as f32)
        .min(640.0 / size.height as f32)
        .clamp(12.0, 44.0);
    let center = Vec2::new(200.0, -24.0);
    let origin = Vec2::new(
        center.x - size.width as f32 * tile_size * 0.5 + tile_size * 0.5,
        center.y - size.height as f32 * tile_size * 0.5 + tile_size * 0.5,
    );
    (tile_size, origin)
}

fn marker_text_for_tile(view: LabView, tile: saddle_procgen_wfc::WfcTileId) -> Option<&'static str> {
    match (view, tile.0) {
        (LabView::Room, 2) => Some("IN"),
        (LabView::Room, 3) => Some("OUT"),
        _ => None,
    }
}

fn view_metric_line(diagnostics: &LabDiagnostics) -> String {
    match diagnostics.active_view {
        LabView::Basic | LabView::Large => {
            format!("water cells: {}", diagnostics.highlighted_cells)
        }
        LabView::Room => format!("portal cells: {}", diagnostics.highlighted_cells),
        LabView::Contradiction => format!("ambiguous cells: {}", diagnostics.ambiguous_cells),
    }
}

fn format_recent_signatures(signatures: &[u64]) -> String {
    let tail = signatures.iter().rev().take(4).copied().collect::<Vec<_>>();
    let tail = tail.into_iter().rev().collect::<Vec<_>>();
    format!("{tail:?}")
}

fn format_position(position: Option<UVec3>) -> String {
    position
        .map(|position| format!("({}, {}, {})", position.x, position.y, position.z))
        .unwrap_or_else(|| "none".to_string())
}
