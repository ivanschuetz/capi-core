use std::collections::HashMap;

use algonaut::{
    algod::v2::Algod,
    core::{to_app_address, Address},
    indexer::v2::Indexer,
    model::indexer::v2::{Account, AssetHolding, QueryAccount},
};
use anyhow::{anyhow, Result};
use mbase::{
    checked::CheckedAdd,
    models::{asset_amount::AssetAmount, dao_app_id::DaoAppId, share_amount::ShareAmount},
    state::{app_state::ApplicationLocalStateError, dao_app_state::dao_investor_state},
};
use rust_decimal::Decimal;

/// Returns holders of the asset with their respective amounts and percentages.
/// See [share_sholders] doc for more details.
/// This function just "decorates" [share_sholders] with the percentage calculation.
pub async fn shares_holders_distribution(
    algod: &Algod,
    indexer: &Indexer,
    asset_id: u64,
    app_id: DaoAppId,
    asset_supply: u64,
) -> Result<Vec<ShareHoldingPercentage>> {
    let holdings = share_sholders(algod, indexer, asset_id, app_id).await?;
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

async fn share_sholders(
    algod: &Algod,
    indexer: &Indexer,
    asset_id: u64,
    app_id: DaoAppId,
) -> Result<Vec<ShareHolding>> {
    let free_holders = free_assets_holdings(indexer, asset_id, &to_app_address(app_id.0)).await?;
    let lockers = lockers_holdings(algod, indexer, app_id).await?;
    let mut merged = merge(free_holders, lockers)?;
    // sort descendingly by amount
    merged.sort_by(|h1, h2| h2.amount.val().cmp(&h1.amount.val()));
    Ok(merged)
}

fn merge(
    free_holdings: Vec<ShareHolding>,
    locked_holdings: Vec<ShareHolding>,
) -> Result<Vec<ShareHolding>> {
    let mut map: HashMap<[u8; 32], ShareAmount> = free_holdings
        .iter()
        .map(|h| (h.address.0, h.amount.to_owned()))
        .collect();

    for holding in locked_holdings {
        if let Some(share_amount) = map.get_mut(&holding.address.0) {
            *share_amount = share_amount.add(&holding.amount)?;
        } else {
            map.insert(holding.address.0, holding.amount);
        }
    }

    Ok(map
        .into_iter()
        .map(|(k, v)| ShareHolding {
            address: Address(k),
            amount: v,
        })
        .collect())
}

// TODO paginate? but clarify first whether we'll actually use this, it's quite expensive either way
// we've to fetch the local state for each account to get the share count
async fn opted_in_to_app(indexer: &Indexer, app_id: DaoAppId) -> Result<Vec<Account>> {
    // get all the accounts opted in to the app (lockers/investors)
    let accounts = indexer
        .accounts(&QueryAccount {
            application_id: Some(app_id.0),
            ..QueryAccount::default()
        })
        .await?;

    Ok(accounts.accounts)
}

// TODO when fetching shares distr sometimes,
// Msg("Unexpected investor local state length: 0, state: ApplicationLocalState { id: 75, key_value: [], schema: ApplicationStateSchema { num_byte_slice: 0, num_uint: 4 } }")
// only reason for no local state should be not opted in, but here we're fetching only opted in accounts - what's going on? also don't remember having opted out the account
async fn lockers_holdings(
    algod: &Algod,
    indexer: &Indexer,
    app_id: DaoAppId,
) -> Result<Vec<ShareHolding>> {
    let opted_in_accounts = opted_in_to_app(indexer, app_id).await?;
    let mut holdings = vec![];
    for opted_in_account in opted_in_accounts {
        // TODO (low prio) small optimization: read only the shares amount
        // TODO consider using join to parallelize these requests
        let state_res = dao_investor_state(algod, &opted_in_account.address, app_id).await;
        let amount = match state_res {
            Ok(state) => {
                log::trace!("Share locker state: {:?}", state);
                state.shares
            }
            Err(e) => {
                if e == ApplicationLocalStateError::NotOptedIn {
                    // Not opted in -> has no locked shares for statistics
                    ShareAmount::new(0)
                } else {
                    return Err(e.into());
                }
            }
        };

        holdings.push(ShareHolding {
            address: opted_in_account.address,
            amount,
        })
    }
    Ok(holdings)
}

/// Returns a list all (unique) addresses that hold the asset, with their respective amounts.
/// Note: amount > 0, i.e. excludes addresses that are opted in but don't hold the asset.
async fn free_assets_holdings(
    indexer: &Indexer,
    asset_id: u64,
    app_escrow: &Address,
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
            && &holder.address != app_escrow
        {
            holdings.push(ShareHolding {
                amount: find_amount(asset_id, &holder.assets)?.into(),
                address: holder.address,
            })
        }
    }
    Ok(holdings)
}

// TODO how is this used? it seems awkward to count only free asset holders as general holders?
pub async fn holders_count(
    indexer: &Indexer,
    asset_id: u64,
    app_escrow: &Address,
) -> Result<usize> {
    let holders_holdings = free_assets_holdings(indexer, asset_id, app_escrow).await?;
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

#[derive(Debug, Clone)]
pub struct ShareHolding {
    pub address: Address,
    pub amount: ShareAmount,
}

#[derive(Debug, Clone)]
pub struct ShareHoldingPercentage {
    pub address: Address,
    pub amount: ShareAmount,
    pub percentage: Decimal,
}
