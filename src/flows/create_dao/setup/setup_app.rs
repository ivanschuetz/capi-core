use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, Transaction, TxnBuilder},
};
use anyhow::Result;

use crate::funds::FundsAssetId;

pub async fn setup_app_tx(
    app_id: u64,
    creator: &Address,
    params: &SuggestedTransactionParams,
    central_escrow: &Address,
    customer_escrow: &Address,
    shares_asset_id: u64,
    funds_asset_id: FundsAssetId,
) -> Result<Transaction> {
    log::debug!("Setting up app: {app_id}");
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*creator, app_id)
            .app_arguments(vec![
                central_escrow.0.to_vec(),
                customer_escrow.0.to_vec(),
                shares_asset_id.to_be_bytes().to_vec(),
                funds_asset_id.0.to_be_bytes().to_vec(),
            ])
            .build(),
    )
    .build()?;
    Ok(tx)
}
