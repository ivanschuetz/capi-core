use super::app_state::{
    get_uint_value_or_error, global_state, local_state, local_state_from_account,
    read_address_from_state, AppStateKey, ApplicationGlobalState, ApplicationLocalStateError,
    ApplicationStateExt,
};
use crate::{
    api::version::{bytes_to_versions, Version, VersionedAddress},
    flows::create_dao::{
        share_amount::ShareAmount,
        shares_percentage::SharesPercentage,
        storage::load_dao::{DaoAppId, DaoId},
    },
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    model::algod::v2::{Account, ApplicationLocalState, TealKeyValue, TealValue},
};
use anyhow::{anyhow, Result};
use data_encoding::{BASE64, HEXLOWER};
use std::{collections::BTreeMap, convert::TryInto};

const GLOBAL_TOTAL_RECEIVED: AppStateKey = AppStateKey("CentralReceivedTotal");

const GLOBAL_CUSTOMER_ESCROW_ADDRESS: AppStateKey = AppStateKey("CustomerEscrowAddress");

const GLOBAL_FUNDS_ASSET_ID: AppStateKey = AppStateKey("FundsAssetId");
const GLOBAL_SHARES_ASSET_ID: AppStateKey = AppStateKey("SharesAssetId");

const GLOBAL_DAO_NAME: AppStateKey = AppStateKey("DaoName");
const GLOBAL_DAO_DESC: AppStateKey = AppStateKey("DaoDesc");
const GLOBAL_SHARE_PRICE: AppStateKey = AppStateKey("SharePrice");
const GLOBAL_INVESTORS_SHARE: AppStateKey = AppStateKey("InvestorsPart");

const GLOBAL_LOGO_URL: AppStateKey = AppStateKey("LogoUrl");
const GLOBAL_SOCIAL_MEDIA_URL: AppStateKey = AppStateKey("SocialMediaUrl");

const GLOBAL_SHARES_LOCKED: AppStateKey = AppStateKey("LockedShares");

// this doesn't seem needed, but needed to re-create Dao struct from state
// TODO different structs for creation inputs and Dao?,
// the number of shares initially reserved to investors seems useless other than when creating the dao,
// and if it were to be needed it can be calculated with indexer (supply - xfer to app escrow when setting the dao up)
const SHARES_FOR_INVESTORS: AppStateKey = AppStateKey("SharesForInvestors");

// not sure this is needed
const GLOBAL_OWNER: AppStateKey = AppStateKey("Owner");

const GLOBAL_VERSIONS: AppStateKey = AppStateKey("Versions");

const LOCAL_CLAIMED_TOTAL: AppStateKey = AppStateKey("ClaimedTotal");
const LOCAL_CLAIMED_INIT: AppStateKey = AppStateKey("ClaimedInit");
const LOCAL_SHARES: AppStateKey = AppStateKey("Shares");
const LOCAL_DAO: AppStateKey = AppStateKey("Dao");

pub const GLOBAL_SCHEMA_NUM_BYTE_SLICES: u64 = 7; // customer escrow, dao name, dao descr, logo, social media, owner, versions
pub const GLOBAL_SCHEMA_NUM_INTS: u64 = 7; // total received, shares asset id, funds asset id, share price, investors part, shares locked, shares for investors

pub const LOCAL_SCHEMA_NUM_BYTE_SLICES: u64 = 0;
pub const LOCAL_SCHEMA_NUM_INTS: u64 = 4; // for investors: "shares", "claimed total", "claimed init", "dao"

// TODO rename in DaoGlobalState
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralAppGlobalState {
    pub received: FundsAmount,

    pub customer_escrow: VersionedAddress,

    pub app_approval_version: Version,
    pub app_clear_version: Version,

    pub funds_asset_id: FundsAssetId,
    pub shares_asset_id: u64,

    pub project_name: String,
    pub project_desc: String,
    pub share_price: FundsAmount,
    pub investors_share: SharesPercentage,
    pub shares_for_investors: ShareAmount,

    pub logo_url: String,
    pub social_media_url: String,

    pub owner: Address,

    pub locked_shares: ShareAmount,
}

/// Returns Ok only if called after dao setup (branch_setup_dao), where all the global state is initialized.
pub async fn dao_global_state(algod: &Algod, app_id: DaoAppId) -> Result<CentralAppGlobalState> {
    let gs = global_state(algod, app_id.0).await?;

    let expected_gs_len = GLOBAL_SCHEMA_NUM_BYTE_SLICES + GLOBAL_SCHEMA_NUM_INTS;
    if gs.len() != expected_gs_len as usize {
        println!("DAO global state:");
        print_state(&gs.0)?;
        return Err(anyhow!(
            "Unexpected global state length: {}. Expected: {expected_gs_len}. Was the DAO setup performed already?",
            gs.len(),
        ));
    }

    let total_received = FundsAmount::new(get_int_or_err(&GLOBAL_TOTAL_RECEIVED, &gs)?);

    let customer_escrow = read_address_from_state(&gs, GLOBAL_CUSTOMER_ESCROW_ADDRESS)?;

    let funds_asset_id = FundsAssetId(get_int_or_err(&GLOBAL_FUNDS_ASSET_ID, &gs)?);
    let shares_asset_id = get_int_or_err(&GLOBAL_SHARES_ASSET_ID, &gs)?;

    let project_name = String::from_utf8(get_bytes_or_err(&GLOBAL_DAO_NAME, &gs)?)?;
    let project_desc = String::from_utf8(get_bytes_or_err(&GLOBAL_DAO_DESC, &gs)?)?;

    let share_price = FundsAmount::new(get_int_or_err(&GLOBAL_SHARE_PRICE, &gs)?);
    let investors_share = get_int_or_err(&GLOBAL_INVESTORS_SHARE, &gs)?.try_into()?;

    let logo_url = String::from_utf8(get_bytes_or_err(&GLOBAL_LOGO_URL, &gs)?)?;
    let social_media_url = String::from_utf8(get_bytes_or_err(&GLOBAL_SOCIAL_MEDIA_URL, &gs)?)?;

    let owner = read_address_from_state(&gs, GLOBAL_OWNER)?;

    let versions_bytes = get_bytes_or_err(&GLOBAL_VERSIONS, &gs)?;
    let versions = bytes_to_versions(&versions_bytes)?;

    let shares_locked = ShareAmount::new(get_int_or_err(&GLOBAL_SHARES_LOCKED, &gs)?);

    let shares_for_investors = ShareAmount::new(get_int_or_err(&SHARES_FOR_INVESTORS, &gs)?);

    Ok(CentralAppGlobalState {
        received: total_received,
        customer_escrow: VersionedAddress::new(customer_escrow, versions.customer_escrow),
        app_approval_version: versions.app_approval,
        app_clear_version: versions.app_clear,
        funds_asset_id,
        shares_asset_id,
        project_name,
        project_desc,
        share_price,
        investors_share: investors_share,
        logo_url,
        social_media_url,
        owner,
        locked_shares: shares_locked,
        shares_for_investors,
    })
}

fn print_state(values: &[TealKeyValue]) -> Result<()> {
    let mut key_values = BTreeMap::new();
    for kv in values {
        let key_bytes = BASE64.decode(kv.key.as_bytes())?;
        key_values.insert(String::from_utf8(key_bytes)?, value_to_str(&kv.value)?);
    }

    // separate step in case we split the fn
    for (k, v) in key_values {
        println!("{k} => {v:?}")
    }

    Ok(())
}

fn value_to_str(value: &TealValue) -> Result<String> {
    match &value.value_type {
        1 => Ok(value
            .bytes
            .clone()
            .try_into()
            // try first to interpret bytes as address
            .map(|array| Address(array).to_string())
            // if not address, display as hex
            .unwrap_or_else(|_| to_hex_str(&value.bytes.clone()))),
        2 => Ok(value.uint.to_string()),
        _ => {
            return Err(anyhow!(
                "Unexpected global value type: {}",
                value.value_type
            ))
        }
    }
}

fn to_hex_str(bytes: &[u8]) -> String {
    format!("0x{}", HEXLOWER.encode(bytes))
}

fn get_int_or_err(key: &AppStateKey, gs: &ApplicationGlobalState) -> Result<u64> {
    gs.find_uint(key).ok_or_else(|| {
        anyhow!(
            "Key: {key:?} (int) not set in global state: {gs:?}, global state len: {}",
            gs.len()
        )
    })
}

fn get_bytes_or_err(key: &AppStateKey, gs: &ApplicationGlobalState) -> Result<Vec<u8>> {
    gs.find_bytes(key).ok_or_else(|| {
        anyhow!(
            "Key: {key:?} (bytes) not set in global state: {gs:?}, global state len: {}",
            gs.len()
        )
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralAppInvestorState {
    pub shares: ShareAmount,
    pub claimed: FundsAmount,
    /// Value to which "claimed" is initialized when the investor locks the shares
    /// We need this mainly for UX, to subtract it from "claimed", in order to show the user what they actually have claimed.
    /// elaboration: "claimed" is initialized to what the investor would be entitled to receive (based on received global state and held shares),
    /// to prevent double claiming (i.e. we allow to claim dividend only for future income).
    /// So we need to subtract this initial value from it, to show the investor what they actually claimed.
    pub claimed_init: FundsAmount,
    pub dao_id: DaoId,
}

pub async fn dao_investor_state(
    algod: &Algod,
    investor: &Address,
    app_id: DaoAppId,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError<'static>> {
    let local_state = local_state(algod, investor, app_id.0).await?;
    central_investor_state_from_local_state(&local_state)
}

pub fn central_investor_state_from_acc(
    account: &Account,
    app_id: DaoAppId,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError<'static>> {
    let local_state = local_state_from_account(account, app_id.0)?;
    central_investor_state_from_local_state(&local_state)
        .map_err(|e| ApplicationLocalStateError::Msg(e.to_string()))
}

/// Expects the user to be invested (as the name indicates) - returns error otherwise.
fn central_investor_state_from_local_state(
    state: &ApplicationLocalState,
) -> Result<CentralAppInvestorState, ApplicationLocalStateError<'static>> {
    if state.len() != ((LOCAL_SCHEMA_NUM_BYTE_SLICES + LOCAL_SCHEMA_NUM_INTS) as usize) {
        println!("Investor local state:");
        print_state(&state.key_value).map_err(|e| {
            ApplicationLocalStateError::Msg(format!("Error printing local state: {e}"))
        })?;
        return Err(ApplicationLocalStateError::Msg(format!(
            "Unexpected investor local state length: {}, state: {state:?}",
            state.len(),
        )));
    }

    let shares = get_uint_value_or_error(state, &LOCAL_SHARES)?;
    let claimed = FundsAmount::new(get_uint_value_or_error(state, &LOCAL_CLAIMED_TOTAL)?);
    let claimed_init = FundsAmount::new(get_uint_value_or_error(state, &LOCAL_CLAIMED_INIT)?);
    let dao_id = DaoId(DaoAppId(get_uint_value_or_error(state, &LOCAL_DAO)?));

    Ok(CentralAppInvestorState {
        shares: ShareAmount::new(shares),
        claimed,
        claimed_init,
        dao_id,
    })
}

/// Gets dao ids for all the capi apps where the user is opted in
pub fn find_state_with_a_capi_dao_id(
    app_local_state: &ApplicationLocalState,
) -> Result<Option<DaoId>> {
    let maybe_int_value = app_local_state.find_uint(&LOCAL_DAO);
    match maybe_int_value {
        Some(value) => Ok(Some(DaoId(DaoAppId(value)))),
        // Not found is Ok: we just didn't find a matching key value
        None => Ok(None),
    }
}
