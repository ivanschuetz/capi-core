use algonaut::model::algod::v2::Application;
use anyhow::{anyhow, Result};
use data_encoding::BASE64;

pub fn withdrawal_amount_global_state(apps_state: &Application) -> Option<u64> {
    apps_state
        .params
        .global_state
        .iter()
        .find(|s| s.key == BASE64.encode(b"Amount"))
        .map(|s| s.value.uint)
}

pub fn withdrawal_amount_global_state_or_err(apps_state: &Application) -> Result<u64> {
    withdrawal_amount_global_state(apps_state)
        .ok_or_else(|| anyhow!("Withdrawal amount global state not set"))
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
    votes_global_state(apps_state).ok_or_else(|| anyhow!("Votes global state not set"))
}
