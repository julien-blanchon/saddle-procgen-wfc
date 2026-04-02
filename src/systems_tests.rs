use std::time::Duration;

use bevy::ecs::message::Messages;
use bevy::tasks::AsyncComputeTaskPool;

use super::*;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct Activate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct Deactivate;

fn runtime_rules() -> WfcRuleset {
    let floor = WfcTileId(0);
    let wall = WfcTileId(1);
    let all = [floor, wall];
    WfcRuleset::new(
        WfcTopology::Cartesian2d,
        vec![
            WfcTileDefinition::new(floor, 3.0, "Floor"),
            WfcTileDefinition::new(wall, 1.0, "Wall"),
        ],
    )
    .with_rule(floor, WfcDirection::XPos, all)
    .with_rule(floor, WfcDirection::XNeg, all)
    .with_rule(floor, WfcDirection::YPos, all)
    .with_rule(floor, WfcDirection::YNeg, all)
    .with_rule(wall, WfcDirection::XPos, all)
    .with_rule(wall, WfcDirection::XNeg, all)
    .with_rule(wall, WfcDirection::YPos, all)
    .with_rule(wall, WfcDirection::YNeg, all)
}

fn runtime_request() -> WfcRequest {
    WfcRequest::new(WfcGridSize::new_2d(12, 12), runtime_rules(), WfcSeed(5))
}

fn impossible_request() -> WfcRequest {
    let mut request = runtime_request();
    request.border_constraints = vec![
        WfcBorderConstraint::new(WfcBorder::MinX, [WfcTileId(0)]),
        WfcBorderConstraint::new(WfcBorder::MaxX, [WfcTileId(1)]),
    ];
    request.grid_size = WfcGridSize::new_2d(1, 1);
    request
}

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_schedule(Activate);
    app.init_schedule(Deactivate);
    app.add_plugins(WfcPlugin::new(Activate, Deactivate, Update));
    app.world_mut().run_schedule(Activate);
    app
}

#[test]
fn plugin_initializes_runtime_resources_and_messages() {
    let app = test_app();
    assert!(app.world().contains_resource::<WfcRuntimeDiagnostics>());
    assert!(app.world().contains_resource::<Messages<GenerateWfc>>());
    assert!(app.world().contains_resource::<Messages<WfcSolved>>());
    assert!(app.world().contains_resource::<Messages<WfcFailed>>());
    assert!(app.world().contains_resource::<Messages<WfcProgress>>());
}

#[test]
fn request_spawns_job_and_emits_success_messages() {
    let mut app = test_app();
    let mut solved_cursor = app.world().resource::<Messages<WfcSolved>>().get_cursor();
    let mut progress_cursor = app.world().resource::<Messages<WfcProgress>>().get_cursor();

    app.world_mut()
        .resource_mut::<Messages<GenerateWfc>>()
        .write(GenerateWfc {
            request: runtime_request(),
            label: Some("runtime success".into()),
        });

    let job_entity = {
        app.update();
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &WfcJob)>();
        let (entity, job) = query.single(world).expect("one job should spawn");
        assert_eq!(job.status, WfcJobStatus::Running);
        entity
    };

    // Collect progress messages during polling to avoid buffer expiry over many frames.
    let mut all_progress: Vec<WfcProgress> = {
        let progress_messages = app.world().resource::<Messages<WfcProgress>>();
        progress_cursor.read(progress_messages).cloned().collect()
    };

    for _ in 0..256 {
        app.update();
        {
            let progress_messages = app.world().resource::<Messages<WfcProgress>>();
            all_progress.extend(progress_cursor.read(progress_messages).cloned());
        }
        if app
            .world()
            .entity(job_entity)
            .get::<WfcJob>()
            .is_some_and(|job| job.status == WfcJobStatus::Succeeded)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let job = app
        .world()
        .entity(job_entity)
        .get::<WfcJob>()
        .expect("job should remain for inspection");
    let result = app
        .world()
        .entity(job_entity)
        .get::<WfcJobResult>()
        .expect("result component should exist");
    assert_eq!(job.status, WfcJobStatus::Succeeded);
    assert!(result.solution.is_some());
    assert!(result.failure.is_none());

    let solved_messages = app.world().resource::<Messages<WfcSolved>>();
    let solved: Vec<_> = solved_cursor.read(solved_messages).cloned().collect();
    assert_eq!(solved.len(), 1);

    assert!(
        all_progress
            .iter()
            .any(|message| message.status == WfcJobStatus::Running)
    );
    assert!(
        all_progress
            .iter()
            .any(|message| message.status == WfcJobStatus::Succeeded)
    );
}

#[test]
fn failed_jobs_emit_failure_message_without_blocking_updates() {
    let mut app = test_app();
    let mut failed_cursor = app.world().resource::<Messages<WfcFailed>>().get_cursor();

    app.world_mut()
        .resource_mut::<Messages<GenerateWfc>>()
        .write(GenerateWfc {
            request: impossible_request(),
            label: Some("runtime failure".into()),
        });

    let start = std::time::Instant::now();
    for _ in 0..256 {
        app.update();
        let failed_messages = app.world().resource::<Messages<WfcFailed>>();
        if failed_cursor.read(failed_messages).next().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    assert!(
        start.elapsed() < Duration::from_secs(1),
        "polling should stay non-blocking"
    );
    assert_eq!(
        app.world().resource::<WfcRuntimeDiagnostics>().failed_jobs,
        1
    );
}

#[test]
fn deactivate_schedule_cancels_inflight_jobs() {
    let mut app = test_app();
    let request = runtime_request();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        std::thread::sleep(Duration::from_millis(50));
        solve_wfc(&request)
    });
    app.world_mut().spawn((
        WfcJob {
            id: WfcJobId(999),
            label: "cancel me".into(),
            status: WfcJobStatus::Running,
            request: runtime_request(),
        },
        WfcJobResult::default(),
        systems::PendingWfcTask(task),
    ));
    app.world_mut()
        .resource_mut::<WfcRuntimeDiagnostics>()
        .running_jobs = 1;

    app.world_mut().run_schedule(Deactivate);
    app.update();

    let world = app.world_mut();
    let mut query = world.query::<&WfcJob>();
    let job = query.single(world).expect("job should still exist");
    assert_eq!(job.status, WfcJobStatus::Cancelled);
    assert_eq!(world.resource::<WfcRuntimeDiagnostics>().cancelled_jobs, 1);
}

#[test]
fn successful_job_clears_previous_failure_reason() {
    let mut app = test_app();
    app.world_mut()
        .resource_mut::<WfcRuntimeDiagnostics>()
        .last_failure_reason = Some(WfcFailureReason::Contradiction);

    app.world_mut()
        .resource_mut::<Messages<GenerateWfc>>()
        .write(GenerateWfc {
            request: runtime_request(),
            label: Some("runtime success".into()),
        });

    for _ in 0..256 {
        app.update();
        if app
            .world()
            .resource::<WfcRuntimeDiagnostics>()
            .completed_jobs
            >= 1
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let diagnostics = app.world().resource::<WfcRuntimeDiagnostics>();
    assert_eq!(diagnostics.last_status, Some(WfcJobStatus::Succeeded));
    assert_eq!(diagnostics.last_failure_reason, None);
}
