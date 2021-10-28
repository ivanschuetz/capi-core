use algonaut::model::algod::v2::{Application, ApplicationLocalState};
use anyhow::{anyhow, Result};
use data_encoding::BASE64;

use crate::app_state_util::{app_local_state, app_local_state_or_err};

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
        .unwrap_or(0))
}

pub fn has_active_withdrawal_request_global_state(apps_state: &Application) -> Option<bool> {
    withdrawal_amount_global_state(apps_state).map(|a| a > 0)
}

pub fn has_active_withdrawal_request_global_state_or_err(apps_state: &Application) -> Result<bool> {
    // unwrap_or: None -> amount global state not set yet -> no active withdrawal request -> false
    Ok(has_active_withdrawal_request_global_state(apps_state).unwrap_or(false))
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
        .unwrap_or(0))
}

pub fn votes_local_state(apps_state: &[ApplicationLocalState], app_id: u64) -> Option<u64> {
    app_local_state(apps_state, app_id)?
        .key_value
        .iter()
        .find(|s| s.key == BASE64.encode(b"LVotes"))
        .map(|s| s.value.uint)
}

pub fn votes_local_state_or_err(apps_state: &[ApplicationLocalState], app_id: u64) -> Result<u64> {
    votes_local_state(apps_state, app_id).ok_or_else(|| anyhow!("Votes local state not set"))
}

pub fn did_vote_local_state(apps_state: &[ApplicationLocalState], app_id: u64) -> Option<bool> {
    votes_local_state(apps_state, app_id).map(|v| v > 0)
}

pub fn did_vote_local_state_or_err(
    apps_state: &[ApplicationLocalState],
    app_id: u64,
) -> Result<bool> {
    did_vote_local_state(apps_state, app_id).ok_or_else(|| anyhow!("Votes local state not set"))
}

pub fn valid_local_state(apps_state: &[ApplicationLocalState], app_id: u64) -> Option<u64> {
    app_local_state(apps_state, app_id)?
        .key_value
        .iter()
        .find(|s| s.key == BASE64.encode(b"Valid"))
        .map(|s| s.value.uint)
}

pub fn valid_local_state_or_err(apps_state: &[ApplicationLocalState], app_id: u64) -> Result<u64> {
    votes_local_state(apps_state, app_id).ok_or_else(|| anyhow!("Votes local state not set"))
}

// TODO all state accessors should return Result and error if local state not found
// currently there's no way to differentiate whether the app was not found or local state not init yet
// alternatively return option, but ensure None means only app not found -> local state has a default value

/// Last initiated withdrawal round (may be active or withdrawal already happened)
/// None if no withdrawal round has been initiated yet
pub fn withdrawal_round_global_state(app_state: &Application) -> Option<u64> {
    app_state
        .params
        .global_state
        .iter()
        .find(|s| s.key == BASE64.encode(b"WRound"))
        .map(|s| s.value.uint)
}

/// The withdrawal round for which the user's last vote was performed
/// Error if the app is not found (user is not opted in)
/// None if local state hasn't been set yet, i.e. user has never voted
pub fn voted_round_local_state(
    apps_state: &[ApplicationLocalState],
    app_id: u64,
) -> Result<Option<u64>> {
    Ok(app_local_state_or_err(apps_state, app_id)?
        .key_value
        .iter()
        .find(|s| s.key == BASE64.encode(b"VWRound"))
        .map(|s| s.value.uint))
}
