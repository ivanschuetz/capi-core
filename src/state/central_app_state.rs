use super::app_state::{
    get_bytes_value_or_error, get_uint_value_or_error, global_state, local_state,
    local_state_from_account, read_address_from_state, AppStateKey, ApplicationLocalStateError,
    ApplicationStateExt,
};
use crate::{
    flows::create_project::{share_amount::ShareAmount, storage::load_project::ProjectId},
    funds::FundsAmount,
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    model::algod::v2::{Account, ApplicationLocalState},
};
use anyhow::{anyhow, Result};
use std::convert::TryInto;

const GLOBAL_TOTAL_RECEIVED: AppStateKey = AppStateKey("CentralReceivedTotal");
const GLOBAL_CENTRAL_ESCROW_ADDRESS: AppStateKey = AppStateKey("CentralEscrowAddress");
const GLOBAL_CUSTOMER_ESCROW_ADDRESS: AppStateKey = AppStateKey("CustomerEscrowAddress");

const LOCAL_HARVESTED_TOTAL: AppStateKey = AppStateKey("HarvestedTotal");
const LOCAL_SHARES: AppStateKey = AppStateKey("Shares");
const LOCAL_PROJECT: AppStateKey = AppStateKey("Project");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralAppGlobalState {
    pub received: FundsAmount,
    pub central_escrow: Address,
    pub customer_escrow: Address,
}

pub async fn central_global_state(algod: &Algod, app_id: u64) -> Result<CentralAppGlobalState> {
    let global_state = global_state(algod, app_id).await?;
    if global_state.len() != 3 {
        return Err(anyhow!(
            "Unexpected global state length: {}",
            global_state.len()
        ));
    }
    let total_received =
        FundsAmount::new(global_state.find_uint(&GLOBAL_TOTAL_RECEIVED).unwrap_or(0));

    let central_escrow = read_address_from_state(
        &global_state,
        GLOBAL_CENTRAL_ESCROW_ADDRESS,
        "central escrow",
    )?;

    let customer_escrow = read_address_from_state(
        &global_state,
        GLOBAL_CUSTOMER_ESCROW_ADDRESS,
        "customer escrow",
    )?;

    Ok(CentralAppGlobalState {
        received: total_received,
        central_escrow,
        customer_escrow,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralAppInvestorState {
    pub shares: ShareAmount,
    pub harvested: FundsAmount,
    pub project_id: ProjectId,
}

pub async fn central_investor_state(
    algod: &Algod,
    investor: &Address,
    app_id: u64,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError<'static>> {
    let local_state = local_state(algod, investor, app_id).await?;
    central_investor_state_from_local_state(&local_state)
}

pub fn central_investor_state_from_acc(
    account: &Account,
    app_id: u64,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError<'static>> {
    let local_state = local_state_from_account(account, app_id)?;
    central_investor_state_from_local_state(&local_state)
        .map_err(|e| ApplicationLocalStateError::Msg(e.to_string()))
}

/// Expects the user to be invested (as the name indicates) - returns error otherwise.
fn central_investor_state_from_local_state(
    state: &ApplicationLocalState,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError<'static>> {
    if state.len() != 3 {
        return Err(ApplicationLocalStateError::Msg(format!(
            "Unexpected investor local state length: {}",
            state.len(),
        )));
    }

    let shares = get_uint_value_or_error(state, &LOCAL_SHARES)?;
    let harvested = FundsAmount::new(get_uint_value_or_error(state, &LOCAL_HARVESTED_TOTAL)?);
    let project_id_bytes = get_bytes_value_or_error(state, &LOCAL_PROJECT)?;

    let project_id: ProjectId = project_id_bytes
        .as_slice()
        .try_into()
        .map_err(|e: anyhow::Error| ApplicationLocalStateError::Msg(e.to_string()))?;

    Ok(CentralAppInvestorState {
        shares: ShareAmount::new(shares),
        harvested,
        project_id,
    })
}

/// Gets project ids for all the capi apps where the user is opted in
pub fn find_state_with_a_capi_project_id(
    app_local_state: &ApplicationLocalState,
) -> Result<Option<ProjectId>> {
    let maybe_bytes = app_local_state.find_bytes(&LOCAL_PROJECT);
    match maybe_bytes {
        Some(bytes) => {
            let project_id: ProjectId = bytes.as_slice().try_into()?;
            Ok(Some(project_id))
        }
        // Not found is Ok: we just didn't find a matching key value
        None => Ok(None),
    }
}
