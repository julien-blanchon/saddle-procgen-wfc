use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{WfcFailure, WfcJobId, WfcJobStatus, WfcRequest, WfcSolution};

#[derive(Message, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct GenerateWfc {
    pub request: WfcRequest,
    pub label: Option<String>,
}

#[derive(Message, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct WfcProgress {
    pub job_id: WfcJobId,
    pub label: String,
    pub status: WfcJobStatus,
    pub signature: Option<u64>,
}

#[derive(Message, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct WfcSolved {
    pub job_id: WfcJobId,
    pub label: String,
    pub solution: WfcSolution,
}

#[derive(Message, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct WfcFailed {
    pub job_id: WfcJobId,
    pub label: String,
    pub failure: WfcFailure,
}
