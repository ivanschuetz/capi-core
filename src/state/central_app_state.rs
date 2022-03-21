use super::app_state::{
    get_bytes_value_or_error, get_uint_value_or_error, global_state, local_state,
    local_state_from_account, read_address_from_state, AppStateKey, ApplicationGlobalState,
    ApplicationLocalStateError, ApplicationStateExt,
};
use crate::{
    flows::create_dao::{share_amount::ShareAmount, storage::load_dao::DaoId},
    funds::{FundsAmount, FundsAssetId},
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
const GLOBAL_INVESTING_ESCROW_ADDRESS: AppStateKey = AppStateKey("InvestingEscrowAddress");
const GLOBAL_LOCKING_ESCROW_ADDRESS: AppStateKey = AppStateKey("LockingEscrowAddress");

const GLOBAL_FUNDS_ASSET_ID: AppStateKey = AppStateKey("FundsAssetId");
const GLOBAL_SHARES_ASSET_ID: AppStateKey = AppStateKey("SharesAssetId");

const GLOBAL_PROJECT_NAME: AppStateKey = AppStateKey("ProjectName");
const GLOBAL_PROJECT_DESC: AppStateKey = AppStateKey("ProjectDesc");
const GLOBAL_SHARE_PRICE: AppStateKey = AppStateKey("SharePrice");
const GLOBAL_INVESTORS_PART: AppStateKey = AppStateKey("InvestorsPart");

const GLOBAL_LOGO_URL: AppStateKey = AppStateKey("LogoUrl");
const GLOBAL_SOCIAL_MEDIA_URL: AppStateKey = AppStateKey("SocialMediaUrl");

// not sure this is needed
const GLOBAL_OWNER: AppStateKey = AppStateKey("Owner");

const LOCAL_CLAIMED_TOTAL: AppStateKey = AppStateKey("ClaimedTotal");
const LOCAL_SHARES: AppStateKey = AppStateKey("Shares");
const LOCAL_DAO: AppStateKey = AppStateKey("Dao");

pub const GLOBAL_SCHEMA_NUM_BYTE_SLICES: u64 = 9; // central escrow, customer escrow, investing escrow, locking escrow, dao name, dao descr, logo, social media, owner
pub const GLOBAL_SCHEMA_NUM_INTS: u64 = 5; // total received, shares asset id, funds asset id, share price, investors part

pub const LOCAL_SCHEMA_NUM_BYTE_SLICES: u64 = 1; // for investors: "dao"
pub const LOCAL_SCHEMA_NUM_INTS: u64 = 2; // for investors: "shares", "already retrieved"

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralAppGlobalState {
    pub received: FundsAmount,

    pub central_escrow: Address,
    pub customer_escrow: Address,
    pub investing_escrow: Address,
    pub locking_escrow: Address,

    pub funds_asset_id: FundsAssetId,
    pub shares_asset_id: u64,

    pub project_name: String,
    pub project_desc: String,
    pub share_price: FundsAmount,
    pub investors_part: ShareAmount,

    pub logo_url: String,
    pub social_media_url: String,

    pub owner: Address,
}

/// Returns Ok only if called after dao setup (branch_setup_dao), where all the global state is initialized.
pub async fn central_global_state(algod: &Algod, app_id: u64) -> Result<CentralAppGlobalState> {
    let gs = global_state(algod, app_id).await?;
    if gs.len() != ((GLOBAL_SCHEMA_NUM_BYTE_SLICES + GLOBAL_SCHEMA_NUM_INTS) as usize) {
        return Err(anyhow!(
            "Unexpected global state length: {}, state: {gs:?}. Was the DAO setup performed already?",
            gs.len(),
        ));
    }
    let total_received = FundsAmount::new(get_int_or_err(&GLOBAL_TOTAL_RECEIVED, &gs)?);

    let central_escrow =
        read_address_from_state(&gs, GLOBAL_CENTRAL_ESCROW_ADDRESS, "central escrow")?;
    let customer_escrow =
        read_address_from_state(&gs, GLOBAL_CUSTOMER_ESCROW_ADDRESS, "customer escrow")?;
    let investing_escrow =
        read_address_from_state(&gs, GLOBAL_INVESTING_ESCROW_ADDRESS, "investing escrow")?;
    let locking_escrow =
        read_address_from_state(&gs, GLOBAL_LOCKING_ESCROW_ADDRESS, "locking escrow")?;

    let funds_asset_id = FundsAssetId(get_int_or_err(&GLOBAL_FUNDS_ASSET_ID, &gs)?);
    let shares_asset_id = get_int_or_err(&GLOBAL_SHARES_ASSET_ID, &gs)?;

    let project_name = String::from_utf8(get_bytes_or_err(&GLOBAL_PROJECT_NAME, &gs)?)?;
    let project_desc = String::from_utf8(get_bytes_or_err(&GLOBAL_PROJECT_DESC, &gs)?)?;

    let share_price = FundsAmount::new(get_int_or_err(&GLOBAL_SHARE_PRICE, &gs)?);
    let investors_part = ShareAmount::new(get_int_or_err(&GLOBAL_INVESTORS_PART, &gs)?);

    let logo_url = String::from_utf8(get_bytes_or_err(&GLOBAL_LOGO_URL, &gs)?)?;
    let social_media_url = String::from_utf8(get_bytes_or_err(&GLOBAL_SOCIAL_MEDIA_URL, &gs)?)?;

    let owner = read_address_from_state(&gs, GLOBAL_OWNER, "owner")?;

    Ok(CentralAppGlobalState {
        received: total_received,
        central_escrow,
        customer_escrow,
        investing_escrow,
        locking_escrow,
        funds_asset_id,
        shares_asset_id,
        project_name,
        project_desc,
        share_price,
        investors_part,
        logo_url,
        social_media_url,
        owner,
    })
}

fn get_int_or_err(key: &AppStateKey, gs: &ApplicationGlobalState) -> Result<u64> {
    gs.find_uint(key).ok_or_else(|| {
        anyhow!(
            "Key: {key:?} not set in global state: {gs:?}, global state len: {}",
            gs.len()
        )
    })
}

fn get_bytes_or_err(key: &AppStateKey, gs: &ApplicationGlobalState) -> Result<Vec<u8>> {
    gs.find_bytes(key).ok_or_else(|| {
        anyhow!(
            "Key: {key:?} not set in global state: {gs:?}, global state len: {}",
            gs.len()
        )
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralAppInvestorState {
    pub shares: ShareAmount,
    pub claimed: FundsAmount,
    pub dao_id: DaoId,
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
    if state.len() != ((LOCAL_SCHEMA_NUM_BYTE_SLICES + LOCAL_SCHEMA_NUM_INTS) as usize) {
        return Err(ApplicationLocalStateError::Msg(format!(
            "Unexpected investor local state length: {}, state: {state:?}",
            state.len(),
        )));
    }

    let shares = get_uint_value_or_error(state, &LOCAL_SHARES)?;
    let claimed = FundsAmount::new(get_uint_value_or_error(state, &LOCAL_CLAIMED_TOTAL)?);
    let dao_id_bytes = get_bytes_value_or_error(state, &LOCAL_DAO)?;

    let dao_id: DaoId = dao_id_bytes
        .as_slice()
        .try_into()
        .map_err(|e: anyhow::Error| ApplicationLocalStateError::Msg(e.to_string()))?;

    Ok(CentralAppInvestorState {
        shares: ShareAmount::new(shares),
        claimed,
        dao_id,
    })
}

/// Gets dao ids for all the capi apps where the user is opted in
pub fn find_state_with_a_capi_dao_id(
    app_local_state: &ApplicationLocalState,
) -> Result<Option<DaoId>> {
    let maybe_bytes = app_local_state.find_bytes(&LOCAL_DAO);
    match maybe_bytes {
        Some(bytes) => {
            let dao_id: DaoId = bytes.as_slice().try_into()?;
            Ok(Some(dao_id))
        }
        // Not found is Ok: we just didn't find a matching key value
        None => Ok(None),
    }
}
