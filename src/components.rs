use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{WfcFailure, WfcFailureReason, WfcRequest, WfcSolution};

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct WfcJobId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum WfcJobStatus {
    Queued,
    #[default]
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct WfcJob {
    pub id: WfcJobId,
    pub label: String,
    pub status: WfcJobStatus,
    pub request: WfcRequest,
}

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct WfcJobResult {
    pub solution: Option<WfcSolution>,
    pub failure: Option<WfcFailure>,
}

#[derive(Resource, Clone, Debug, Reflect, Default, Serialize, Deserialize)]
pub struct WfcRuntimeDiagnostics {
    pub active: bool,
    pub submitted_jobs: u64,
    pub running_jobs: u64,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub cancelled_jobs: u64,
    pub last_job_id: Option<WfcJobId>,
    pub last_status: Option<WfcJobStatus>,
    pub last_signature: Option<u64>,
    pub last_failure_reason: Option<WfcFailureReason>,
}
