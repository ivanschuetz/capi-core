use algonaut::model::algod::v2::{Application, ApplicationLocalState};
use anyhow::{anyhow, Result};
use data_encoding::BASE64;

use crate::app_state_util::app_local_state;

pub fn withdrawal_amount_global_state(apps_state: &Application) -> Option<u64> {
    apps_state
        .params
        .global_state
        .iter()
        .find(|s| s.key == BASE64.encode(b"Amount"))
        .map(|s| s.value.uint)
}

pub fn withdrawal_amount_global_state_or_err(apps_state: &Application) -> Result<u64> {
    Ok(withdrawal_amount_global_state(apps_state)
        // .ok_or_else(|| anyhow!("Withdrawal amount global state not set"))
        .unwrap_or_else(|| 0))
}

pub fn votes_global_state(apps_state: &Application) -> Option<u64> {
    apps_state
        .params
        .global_state
        .iter()
        .find(|s| s.key == BASE64.encode(b"Votes"))
        .map(|s| s.value.uint)
}

pub fn votes_global_state_or_err(apps_state: &Application) -> Result<u64> {
    Ok(votes_global_state(apps_state)
        // .ok_or_else(|| anyhow!("Votes global state not set"))
        .unwrap_or_else(|| 0))
}

pub fn votes_local_state(apps_state: &Vec<ApplicationLocalState>, app_id: u64) -> Option<u64> {
    app_local_state(apps_state, app_id)?
        .key_value
        .iter()
        .find(|s| s.key == BASE64.encode(b"LVotes"))
        .map(|s| s.value.uint)
}

pub fn votes_local_state_or_err(
    apps_state: &Vec<ApplicationLocalState>,
    app_id: u64,
) -> Result<u64> {
    votes_local_state(apps_state, app_id).ok_or_else(|| anyhow!("Votes local state not set"))
}

pub fn valid_local_state(apps_state: &Vec<ApplicationLocalState>, app_id: u64) -> Option<u64> {
    app_local_state(apps_state, app_id)?
        .key_value
        .iter()
        .find(|s| s.key == BASE64.encode(b"Valid"))
        .map(|s| s.value.uint)
}

pub fn valid_local_state_or_err(
    apps_state: &Vec<ApplicationLocalState>,
    app_id: u64,
) -> Result<u64> {
    votes_local_state(apps_state, app_id).ok_or_else(|| anyhow!("Votes local state not set"))
}
