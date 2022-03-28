use crate::{
    api::version::{versions_to_bytes, Version, VersionedAddress, Versions},
    flows::create_dao::{share_amount::ShareAmount, storage::load_dao::DaoAppId},
    funds::{FundsAmount, FundsAssetId},
    note::dao_setup_prefix,
};
use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, Transaction, TxnBuilder},
};
use anyhow::Result;

/// Data to initialize the app's global state with
/// NOTE that this doesn't necessarily include *all* the app's state fields,
/// state initialized to a fixed value can be just set in TEAL / doesn't have to be passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaoInitData {
    pub central_escrow: VersionedAddress,
    pub customer_escrow: VersionedAddress,
    pub investing_escrow: VersionedAddress,
    pub locking_escrow: VersionedAddress,

    pub app_approval_version: Version,
    pub app_clear_version: Version,

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

impl DaoInitData {
    pub fn versions(&self) -> Versions {
        Versions {
            app_approval: self.app_approval_version,
            app_clear: self.app_clear_version,
            central_escrow: self.central_escrow.version,
            customer_escrow: self.customer_escrow.version,
            investing_escrow: self.investing_escrow.version,
            locking_escrow: self.locking_escrow.version,
        }
    }
}

pub async fn setup_app_tx(
    app_id: DaoAppId,
    creator: &Address,
    params: &SuggestedTransactionParams,
    data: &DaoInitData,
) -> Result<Transaction> {
    log::debug!("Setting up app: {app_id:?}");
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*creator, app_id.0)
            .app_arguments(vec![
                data.central_escrow.address.0.to_vec(),
                data.customer_escrow.address.0.to_vec(),
                data.investing_escrow.address.0.to_vec(),
                data.locking_escrow.address.0.to_vec(),
                data.shares_asset_id.to_be_bytes().to_vec(),
                data.funds_asset_id.0.to_be_bytes().to_vec(),
                data.project_name.as_bytes().to_vec(),
                data.project_description.as_bytes().to_vec(),
                data.share_price.val().to_be_bytes().to_vec(),
                data.investors_part.val().to_be_bytes().to_vec(),
                data.logo_url.as_bytes().to_vec(),
                data.social_media_url.as_bytes().to_vec(),
                data.owner.0.to_vec(),
                versions_to_bytes(data.versions())?,
            ])
            .build(),
    )
    // TODO: consider enforcing in TEAL that this note is being set
    // for now it's used only as a helper to filter "daos created by me" (via indexer)
    // so it doesn't need to be secure (it's in the interest of the user / they don't gain anything by omitting it)
    // but maybe this usage changes
    .note(dao_setup_prefix().to_vec())
    .build()?;
    Ok(tx)
}
