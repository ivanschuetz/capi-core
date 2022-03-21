use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, Transaction, TxnBuilder},
};
use anyhow::Result;

use crate::{
    flows::create_dao::share_amount::ShareAmount,
    funds::{FundsAmount, FundsAssetId},
};

/// Data to initialize the app's global state with
/// NOTE that this doesn't necessarily include *all* the app's state fields,
/// state initialized to a fixed value can be just set in TEAL / doesn't have to be passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaoInitData {
    pub central_escrow: Address,
    pub customer_escrow: Address,
    pub investing_escrow: Address,
    pub locking_escrow: Address,

    pub shares_asset_id: u64,
    pub funds_asset_id: FundsAssetId,

    pub project_name: String,
    pub project_description: String,
    pub share_price: FundsAmount,
    pub investors_part: ShareAmount,

    pub logo_url: String,
    pub social_media_url: String,

    pub owner: Address,
}

pub async fn setup_app_tx(
    app_id: u64,
    creator: &Address,
    params: &SuggestedTransactionParams,
    data: &DaoInitData,
) -> Result<Transaction> {
    log::debug!("Setting up app: {app_id}");
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*creator, app_id)
            .app_arguments(vec![
                data.central_escrow.0.to_vec(),
                data.customer_escrow.0.to_vec(),
                data.investing_escrow.0.to_vec(),
                data.locking_escrow.0.to_vec(),
                data.shares_asset_id.to_be_bytes().to_vec(),
                data.funds_asset_id.0.to_be_bytes().to_vec(),
                data.project_name.as_bytes().to_vec(),
                data.project_description.as_bytes().to_vec(),
                data.share_price.val().to_be_bytes().to_vec(),
                data.investors_part.val().to_be_bytes().to_vec(),
                data.logo_url.as_bytes().to_vec(),
                data.social_media_url.as_bytes().to_vec(),
                data.owner.0.to_vec(),
            ])
            .build(),
    )
    .build()?;
    Ok(tx)
}
