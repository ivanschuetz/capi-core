use algonaut::{algod::v2::Algod, core::Address};
use anyhow::Result;

pub async fn asset_holdings(algod: &Algod, address: &Address, asset_id: u64) -> Result<u64> {
    Ok(algod
        .account_information(address)
        .await?
        .assets
        .iter()
        .find(|a| a.asset_id == asset_id)
        .map(|h| h.amount)
        // asset id not found -> user not opted in -> 0 holdings
        // we don't differentiate here between not opted in or opted in with no holdings
        .unwrap_or(0))
}
