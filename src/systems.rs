use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};

use crate::{
    GenerateWfc, WfcFailed, WfcJob, WfcJobId, WfcJobResult, WfcJobStatus, WfcProgress,
    WfcRuntimeDiagnostics, WfcSolved, solve_wfc,
};

#[derive(Resource, Default)]
pub(crate) struct WfcRuntimeState {
    pub active: bool,
    pub next_job_id: u64,
}

#[derive(Component)]
pub(crate) struct PendingWfcTask(pub Task<Result<crate::WfcSolution, crate::WfcFailure>>);

#[derive(Component)]
pub(crate) struct CompletedWfcTask(pub Result<crate::WfcSolution, crate::WfcFailure>);

pub(crate) fn activate_runtime(
    mut state: ResMut<WfcRuntimeState>,
    mut diagnostics: ResMut<WfcRuntimeDiagnostics>,
) {
    state.active = true;
    diagnostics.active = true;
}

pub(crate) fn deactivate_runtime(
    mut commands: Commands,
    mut state: ResMut<WfcRuntimeState>,
    mut diagnostics: ResMut<WfcRuntimeDiagnostics>,
    mut progress: MessageWriter<WfcProgress>,
    mut jobs: Query<(Entity, &mut WfcJob), With<PendingWfcTask>>,
) {
    state.active = false;
    diagnostics.active = false;

    for (entity, mut job) in &mut jobs {
        job.status = WfcJobStatus::Cancelled;
        diagnostics.running_jobs = diagnostics.running_jobs.saturating_sub(1);
        diagnostics.cancelled_jobs = diagnostics.cancelled_jobs.saturating_add(1);
        diagnostics.last_job_id = Some(job.id);
        diagnostics.last_status = Some(WfcJobStatus::Cancelled);
        diagnostics.last_failure_reason = None;
        progress.write(WfcProgress {
            job_id: job.id,
            label: job.label.clone(),
            status: WfcJobStatus::Cancelled,
            signature: None,
        });
        commands.entity(entity).remove::<PendingWfcTask>();
    }
}

pub(crate) fn runtime_is_active(state: Res<WfcRuntimeState>) -> bool {
    state.active
}

pub(crate) fn request_jobs(
    mut commands: Commands,
    mut state: ResMut<WfcRuntimeState>,
    mut diagnostics: ResMut<WfcRuntimeDiagnostics>,
    mut requests: MessageReader<GenerateWfc>,
    mut progress: MessageWriter<WfcProgress>,
) {
    for request in requests.read() {
        state.next_job_id = state.next_job_id.saturating_add(1);
        let job_id = WfcJobId(state.next_job_id);
        let label = request
            .label
            .clone()
            .unwrap_or_else(|| format!("WFC Job {}", job_id.0));
        let solve_request = request.request.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move { solve_wfc(&solve_request) });

        commands.spawn((
            Name::new(format!("WFC Job {}", job_id.0)),
            WfcJob {
                id: job_id,
                label: label.clone(),
                status: WfcJobStatus::Running,
                request: request.request.clone(),
            },
            WfcJobResult::default(),
            PendingWfcTask(task),
        ));

        diagnostics.submitted_jobs = diagnostics.submitted_jobs.saturating_add(1);
        diagnostics.running_jobs = diagnostics.running_jobs.saturating_add(1);
        diagnostics.last_job_id = Some(job_id);
        diagnostics.last_status = Some(WfcJobStatus::Running);
        diagnostics.last_failure_reason = None;

        progress.write(WfcProgress {
            job_id,
            label,
            status: WfcJobStatus::Running,
            signature: None,
        });
    }
}

pub(crate) fn poll_jobs(
    mut commands: Commands,
    mut jobs: Query<(Entity, &mut PendingWfcTask), Without<CompletedWfcTask>>,
) {
    for (entity, mut pending) in &mut jobs {
        if let Some(result) = check_ready(&mut pending.0) {
            commands.entity(entity).insert(CompletedWfcTask(result));
            commands.entity(entity).remove::<PendingWfcTask>();
        }
    }
}

pub(crate) fn apply_results(
    mut commands: Commands,
    mut diagnostics: ResMut<WfcRuntimeDiagnostics>,
    mut progress: MessageWriter<WfcProgress>,
    mut solved_writer: MessageWriter<WfcSolved>,
    mut failed_writer: MessageWriter<WfcFailed>,
    mut jobs: Query<(Entity, &mut WfcJob, &mut WfcJobResult, &CompletedWfcTask)>,
) {
    for (entity, mut job, mut result_component, completed) in &mut jobs {
        diagnostics.running_jobs = diagnostics.running_jobs.saturating_sub(1);
        diagnostics.last_job_id = Some(job.id);

        match &completed.0 {
            Ok(solution) => {
                job.status = WfcJobStatus::Succeeded;
                diagnostics.completed_jobs = diagnostics.completed_jobs.saturating_add(1);
                diagnostics.last_status = Some(WfcJobStatus::Succeeded);
                diagnostics.last_signature = Some(solution.signature);
                diagnostics.last_failure_reason = None;
                result_component.solution = Some(solution.clone());
                result_component.failure = None;
                progress.write(WfcProgress {
                    job_id: job.id,
                    label: job.label.clone(),
                    status: WfcJobStatus::Succeeded,
                    signature: Some(solution.signature),
                });
                solved_writer.write(WfcSolved {
                    job_id: job.id,
                    label: job.label.clone(),
                    solution: solution.clone(),
                });
            }
            Err(failure) => {
                job.status = WfcJobStatus::Failed;
                diagnostics.failed_jobs = diagnostics.failed_jobs.saturating_add(1);
                diagnostics.last_status = Some(WfcJobStatus::Failed);
                diagnostics.last_failure_reason = Some(failure.reason.clone());
                result_component.solution = None;
                result_component.failure = Some(failure.clone());
                progress.write(WfcProgress {
                    job_id: job.id,
                    label: job.label.clone(),
                    status: WfcJobStatus::Failed,
                    signature: None,
                });
                failed_writer.write(WfcFailed {
                    job_id: job.id,
                    label: job.label.clone(),
                    failure: failure.clone(),
                });
            }
        }

        commands.entity(entity).remove::<CompletedWfcTask>();
    }
}

pub(crate) fn refresh_runtime_diagnostics(
    mut diagnostics: ResMut<WfcRuntimeDiagnostics>,
    jobs: Query<(), With<PendingWfcTask>>,
) {
    diagnostics.running_jobs = jobs.iter().count() as u64;
}
