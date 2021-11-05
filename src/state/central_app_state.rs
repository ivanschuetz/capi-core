use super::app_state::{
    global_state, local_state, local_state_from_account, AppStateKey, ApplicationLocalStateError,
    ApplicationStateExt,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    model::algod::v2::{Account, ApplicationLocalState},
};
use anyhow::Result;

const GLOBAL_TOTAL_RECEIVED: AppStateKey = AppStateKey("CentralReceivedTotal");

const LOCAL_HARVESTED_TOTAL: AppStateKey = AppStateKey("HarvestedTotal");
const LOCAL_SHARES: AppStateKey = AppStateKey("Shares");

pub struct CentralAppGlobalState {
    pub received: MicroAlgos,
}

pub async fn central_global_state(algod: &Algod, app_id: u64) -> Result<CentralAppGlobalState> {
    let global_state = global_state(algod, app_id).await?;
    let total_received = MicroAlgos(global_state.find_uint(&GLOBAL_TOTAL_RECEIVED).unwrap_or(0));
    Ok(CentralAppGlobalState {
        received: total_received,
    })
}

pub struct CentralAppInvestorState {
    pub shares: u64,
    pub harvested: MicroAlgos,
}

pub async fn central_investor_state(
    algod: &Algod,
    investor: &Address,
    app_id: u64,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError> {
    let local_state = local_state(algod, investor, app_id).await?;
    Ok(central_investor_state_from_local_state(&local_state))
}

pub fn central_investor_state_from_acc(
    account: &Account,
    app_id: u64,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError> {
    let local_state = local_state_from_account(account, app_id)?;
    Ok(central_investor_state_from_local_state(&local_state))
}

fn central_investor_state_from_local_state(
    state: &ApplicationLocalState,
) -> CentralAppInvestorState {
    let shares = state.find_uint(&LOCAL_SHARES).unwrap_or(0);
    let harvested = MicroAlgos(state.find_uint(&LOCAL_HARVESTED_TOTAL).unwrap_or(0));
    CentralAppInvestorState { shares, harvested }
}
