use algonaut::{
    core::MicroAlgos,
    model::algod::v2::{Application, ApplicationLocalState},
};
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

pub fn already_harvested_local_state(
    apps_state: &Vec<ApplicationLocalState>,
    app_id: u64,
) -> Option<MicroAlgos> {
    app_local_state(apps_state, app_id)?
        .key_value
        .iter()
        .find(|s| s.key == BASE64.encode(b"HarvestedTotal"))
        .map(|s| MicroAlgos(s.value.uint))
}

pub fn already_harvested_local_state_or_err(
    apps_state: &Vec<ApplicationLocalState>,
    app_id: u64,
) -> Result<MicroAlgos> {
    already_harvested_local_state(apps_state, app_id)
        .ok_or_else(|| anyhow!("Already harvested local state not set"))
}

pub fn total_received_amount_global_state(apps_state: &Application) -> Option<MicroAlgos> {
    apps_state
        .params
        .global_state
        .iter()
        .find(|s| s.key == BASE64.encode(b"CentralReceivedTotal"))
        .map(|s| MicroAlgos(s.value.uint))
}

pub fn total_received_amount_global_state_or_err(apps_state: &Application) -> Result<MicroAlgos> {
    total_received_amount_global_state(apps_state)
        .ok_or_else(|| anyhow!("Total received amount global state not set"))
}
