use algonaut::{
    core::Address,
    indexer::v2::Indexer,
    model::indexer::v2::{AssetHolding, QueryAccount},
};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;

/// Returns holders of the asset with their respective amounts and percentages.
/// See [shares_holders_holdings] doc for more details.
/// This function just "decorates" [shares_holders_holdings] with the percentage calculation.
pub async fn shares_holders_distribution(
    indexer: &Indexer,
    asset_id: u64,
    asset_supply: u64,
) -> Result<Vec<ShareHoldingPercentage>> {
    let holdings = shares_holders_holdings(indexer, asset_id).await?;
    let asset_supply_decimal: Decimal = asset_supply.into();

    if asset_supply_decimal.is_zero() {
        return Err(anyhow!(
            "Invalid state: it shouldn't be allowed to create asset with a 0 supply"
        ));
    }

    let mut holding_percentages = vec![];
    for h in holdings {
        let amount_decimal: Decimal = h.amount.into();
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

/// See [shares_holders_holdings] doc
pub async fn holders_count(indexer: &Indexer, asset_id: u64) -> Result<usize> {
    let holders_holdings = shares_holders_holdings(indexer, asset_id).await?;
    Ok(holders_holdings.len())
}

/// Returns a list all (unique) addresses that hold the asset, with their respective amounts.
/// Note: amount > 0, i.e. excludes addresses that are opted in but don't hold the asset.
async fn shares_holders_holdings(indexer: &Indexer, asset_id: u64) -> Result<Vec<ShareHolding>> {
    let accounts = indexer
        .accounts(&QueryAccount {
            asset_id: Some(asset_id),
            ..QueryAccount::default()
        })
        .await?;

    log::debug!("Getting holders distribution: {:?}", accounts);

    let mut holdings = vec![];
    for holder in accounts.accounts {
        // TODO we probably should modify Algonaut to return an empty vector here, None doesn't seem to make sense semantically.
        let asset_holding = holder.assets.as_ref().ok_or_else(|| {
            anyhow!("Invalid state: account has no holdings (we just queried by asset id)")
        })?;

        let asset_amount = find_amount(asset_id, &asset_holding)?;
        // if accounts have no assets but are opted in, we get 0 count - filter those out
        if asset_amount > 0 {
            holdings.push(ShareHolding {
                amount: find_amount(asset_id, &asset_holding)?,
                address: holder.address,
            })
        }
    }
    Ok(holdings)
}

/// Helper to get asset holding amount for asset id
/// Private: assumes that `asset_holding` is the result of indexer query by asset id
fn find_amount(asset_id: u64, asset_holding: &[AssetHolding]) -> Result<u64> {
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
        Ok(holding.amount)
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
    pub amount: u64,
}

pub struct ShareHoldingPercentage {
    pub address: Address,
    pub amount: u64,
    pub percentage: Decimal,
}
