use algonaut::model::algod::v2::ApplicationLocalState;
use anyhow::{anyhow, Result};
use data_encoding::BASE64;

use crate::app_state_util::app_local_state;

pub fn shares_local_state(apps_state: &Vec<ApplicationLocalState>, app_id: u64) -> Option<u64> {
    app_local_state(apps_state, app_id)?
        .key_value
        .iter()
        .find(|s| s.key == BASE64.encode(b"Shares"))
        .map(|s| s.value.uint)
}

pub fn shares_local_state_or_err(
    apps_state: &Vec<ApplicationLocalState>,
    app_id: u64,
) -> Result<u64> {
    shares_local_state(apps_state, app_id).ok_or_else(|| anyhow!("Shares local state not set"))
}
