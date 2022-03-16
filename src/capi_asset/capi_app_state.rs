use super::{capi_app_id::CapiAppId, capi_asset_id::CapiAssetAmount};
use crate::{
    funds::FundsAmount,
    state::app_state::{
        get_uint_value_or_error, global_state, local_state, local_state_from_account, AppStateKey,
        ApplicationLocalStateError, ApplicationStateExt,
    },
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    model::algod::v2::{Account, ApplicationLocalState},
};
use anyhow::Result;

const TOTAL_RECEIVED: AppStateKey = AppStateKey("ReceivedTotal");

const LOCAL_HARVESTED_TOTAL: AppStateKey = AppStateKey("HarvestedTotal");
const LOCAL_SHARES: AppStateKey = AppStateKey("Shares");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapiAppGlobalState {
    pub received: FundsAmount,
}

pub async fn capi_app_global_state(algod: &Algod, app_id: CapiAppId) -> Result<CapiAppGlobalState> {
    let global_state = global_state(algod, app_id.0).await?;
    let total_received = FundsAmount::new(global_state.find_uint(&TOTAL_RECEIVED).unwrap_or(0));
    Ok(CapiAppGlobalState {
        received: total_received,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapiAppHolderState {
    // TODO rename in assets? we're naming this asset everywhere else too. shares -> DAO
    pub shares: CapiAssetAmount,
    pub harvested: FundsAmount,
}

pub async fn capi_app_investor_state(
    algod: &Algod,
    investor: &Address,
    app_id: CapiAppId,
) -> Result<CapiAppHolderState, ApplicationLocalStateError<'static>> {
    let local_state = local_state(algod, investor, app_id.0).await?;
    capi_app_investor_state_from_local_state(&local_state)
}

pub fn capi_app_investor_state_from_acc(
    account: &Account,
    app_id: CapiAppId,
) -> Result<CapiAppHolderState, ApplicationLocalStateError<'static>> {
    let local_state = local_state_from_account(account, app_id.0)?;
    log::debug!("Capi investor local state: {local_state:?}");
    capi_app_investor_state_from_local_state(&local_state)
        .map_err(|e| ApplicationLocalStateError::Msg(e.to_string()))
}

/// Expects the user to be invested (as the name indicates) - returns error otherwise.
fn capi_app_investor_state_from_local_state(
    state: &ApplicationLocalState,
) -> Result<CapiAppHolderState, ApplicationLocalStateError<'static>> {
    let shares = get_uint_value_or_error(state, &LOCAL_SHARES)?;
    let harvested = FundsAmount::new(get_uint_value_or_error(state, &LOCAL_HARVESTED_TOTAL)?);

    Ok(CapiAppHolderState {
        shares: CapiAssetAmount::new(shares),
        harvested,
    })
}
