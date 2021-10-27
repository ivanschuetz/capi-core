use algonaut::model::algod::v2::{ApplicationLocalState, TealValue};

use anyhow::{anyhow, Result};
use data_encoding::BASE64;

// TODO pass account instead of apps_state?
pub fn app_local_state(
    apps_state: &[ApplicationLocalState],
    app_id: u64,
) -> Option<&ApplicationLocalState> {
    apps_state.iter().find(|s| s.id == app_id)
}

pub fn app_local_state_or_err(
    apps_state: &[ApplicationLocalState],
    app_id: u64,
) -> Result<&ApplicationLocalState> {
    app_local_state(apps_state, app_id)
        .ok_or_else(|| anyhow!("No local state for app id: {}", app_id))
}

pub fn app_local_var(app_state: &ApplicationLocalState, var: &str) -> Option<TealValue> {
    app_state
        .key_value
        .iter()
        .find(|kv| kv.key == BASE64.encode(var.as_bytes()))
        .map(|kv| kv.value.to_owned())
}

pub fn app_local_var_or_err(app_state: &ApplicationLocalState, var: &str) -> Result<TealValue> {
    app_local_var(app_state, var)
        .ok_or_else(|| anyhow!("Local variable: {} not found in app_state", var))
}
