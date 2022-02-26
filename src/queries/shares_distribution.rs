use algonaut::{
    algod::v2::Algod,
    core::Address,
    indexer::v2::Indexer,
    model::indexer::v2::{Account, AssetHolding, QueryAccount},
};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;

use crate::{
    asset_amount::AssetAmount, flows::create_project::share_amount::ShareAmount,
    state::central_app_state::central_investor_state,
};

/// Returns holders of the asset with their respective amounts and percentages.
/// See [shares_holders_holdings] doc for more details.
/// This function just "decorates" [shares_holders_holdings] with the percentage calculation.
pub async fn shares_holders_distribution(
    algod: &Algod,
    indexer: &Indexer,
    asset_id: u64,
    app_id: u64,
    asset_supply: u64,
    investing_escrow: &Address,
    locking_escrow: &Address,
) -> Result<Vec<ShareHoldingPercentage>> {
    let holdings = share_sholders(
        algod,
        indexer,
        asset_id,
        app_id,
        investing_escrow,
        locking_escrow,
    )
    .await?;
    let asset_supply_decimal: Decimal = asset_supply.into();

    if asset_supply_decimal.is_zero() {
        return Err(anyhow!(
            "Invalid state: it shouldn't be allowed to create asset with a 0 supply"
        ));
    }

    let mut holding_percentages = vec![];
    for h in holdings {
        let amount_decimal: Decimal = h.amount.as_decimal();
        holding_percentages.push(ShareHoldingPercentage {
            address: h.address,
            amount: h.amount,
            percentage: amount_decimal
                .checked_div(asset_supply_decimal)
                // checked_div doesn't return the error, just an optional
                // since we checked for zero divisor above, this should be an overflow, which shouldn't be possible (TODO confirm) as the divisor is originally u64
                .ok_or_else(|| {
                    anyhow!(
                        "Unexpected: division: {} by {} returned an error",
                        amount_decimal,
                        asset_supply_decimal
                    )
                })?,
        });
    }
    Ok(holding_percentages)
}

/// Addresses holding shares (either the asset directly or local state (locked)). Excluding the investing and locking escrow.
async fn share_sholders(
    algod: &Algod,
    indexer: &Indexer,
    asset_id: u64,
    app_id: u64,
    investing_escrow: &Address,
    locking_escrow: &Address,
) -> Result<Vec<ShareHolding>> {
    let mut holdings =
        free_assets_holdings(indexer, asset_id, investing_escrow, locking_escrow).await?;
    let lockers = lockers_holdings(algod, indexer, app_id).await?;
    holdings.extend(lockers);
    Ok(holdings)
}

// TODO paginate? but clarify first whether we'll actually use this, it's quite expensive either way
// we've to fetch the local state for each account to get the share count
async fn lockers(indexer: &Indexer, app_id: u64) -> Result<Vec<Account>> {
    // get all the accounts opted in to the app (lockers/investors)
    let accounts = indexer
        .accounts(&QueryAccount {
            application_id: Some(app_id),
            ..QueryAccount::default()
        })
        .await?;

    Ok(accounts.accounts)
}

async fn lockers_holdings(
    algod: &Algod,
    indexer: &Indexer,
    app_id: u64,
) -> Result<Vec<ShareHolding>> {
    let lockers = lockers(indexer, app_id).await?;
    let mut holdings = vec![];
    for locker in lockers {
        // TODO (low prio) small optimization: read only the shares amount
        // TODO consider using join to parallelize these requests
        let state = central_investor_state(algod, &locker.address, app_id).await?;
        log::trace!("Share locker state: {:?}", state);

        holdings.push(ShareHolding {
            address: locker.address,
            amount: state.shares,
        })
    }
    Ok(holdings)
}

/// Returns a list all (unique) addresses that hold the asset, with their respective amounts.
/// Note: amount > 0, i.e. excludes addresses that are opted in but don't hold the asset.
async fn free_assets_holdings(
    indexer: &Indexer,
    asset_id: u64,
    investing_escrow: &Address,
    locking_escrow: &Address,
) -> Result<Vec<ShareHolding>> {
    let accounts = indexer
        .accounts(&QueryAccount {
            asset_id: Some(asset_id),
            ..QueryAccount::default()
        })
        .await?;

    log::debug!("Got free shares holders: {:?}", accounts);

    let mut holdings = vec![];
    for holder in accounts.accounts {
        let asset_amount = find_amount(asset_id, &holder.assets)?;

        if asset_amount > 0 // if accounts have no assets but are opted in, we get 0 count - filter those out
            // the investing or locking escrow shouldn't show up on the holders list
            && &holder.address != investing_escrow
            && &holder.address != locking_escrow
        {
            holdings.push(ShareHolding {
                amount: find_amount(asset_id, &holder.assets)?.into(),
                address: holder.address,
            })
        }
    }
    Ok(holdings)
}

/// See [shares_holders_holdings] doc
pub async fn holders_count(
    indexer: &Indexer,
    asset_id: u64,
    investing_escrow: &Address,
    locking_escrow: &Address,
) -> Result<usize> {
    let holders_holdings =
        free_assets_holdings(indexer, asset_id, investing_escrow, locking_escrow).await?;
    Ok(holders_holdings.len())
}

/// Helper to get asset holding amount for asset id
/// Private: assumes that `asset_holding` is the result of indexer query by asset id
fn find_amount(asset_id: u64, asset_holding: &[AssetHolding]) -> Result<AssetAmount> {
    let asset_holdings = asset_holding
        .iter()
        .filter(|h| h.asset_id == asset_id)
        .collect::<Vec<&AssetHolding>>();

    if asset_holdings.len() > 1 {
        // We expect Algorand to return only 1 or 0 holdings per asset id
        return Err(anyhow!(
            "Invalid state: more than one asset holding for asset id"
        ));
    }

    if let Some(holding) = asset_holdings.first() {
        Ok(AssetAmount(holding.amount))
    } else {
        // In context of this file, this is an error, as we are queryng by asset id
        // Note that if the user has no holdings but is opted in, we also get holdings (0 count)
        Err(anyhow!(
            "Invalid state: holdings for asset id not found (we just queried by asset id)."
        ))
    }
}

pub struct ShareHolding {
    pub address: Address,
    pub amount: ShareAmount,
}

pub struct ShareHoldingPercentage {
    pub address: Address,
    pub amount: ShareAmount,
    pub percentage: Decimal,
}
