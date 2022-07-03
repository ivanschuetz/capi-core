use crate::note::dao_setup_prefix;
use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::{
    api::version::{versions_to_bytes, Version, Versions},
    models::{
        dao_app_id::DaoAppId,
        funds::{FundsAmount, FundsAssetId},
        hash::GlobalStateHash,
        nft::Nft,
        shares_percentage::SharesPercentage,
        timestamp::Timestamp,
    },
};

/// Data to initialize the app's global state with
/// NOTE that this doesn't necessarily include *all* the app's state fields,
/// state initialized to a fixed value can be just set in TEAL / doesn't have to be passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaoInitData {
    pub app_approval_version: Version,
    pub app_clear_version: Version,

    pub shares_asset_id: u64,
    pub funds_asset_id: FundsAssetId,

    pub project_name: String,
    pub descr_hash: Option<GlobalStateHash>,
    pub share_price: FundsAmount,
    pub investors_share: SharesPercentage,

    pub image_hash: Option<GlobalStateHash>,
    pub image_nft: Option<Nft>,
    pub social_media_url: String,

    pub min_raise_target: FundsAmount,
    pub min_raise_target_end_date: Timestamp,
}

impl DaoInitData {
    pub fn versions(&self) -> Versions {
        Versions {
            app_approval: self.app_approval_version,
            app_clear: self.app_clear_version,
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
                data.shares_asset_id.to_be_bytes().to_vec(),
                data.funds_asset_id.0.to_be_bytes().to_vec(),
                data.project_name.as_bytes().to_vec(),
                data.descr_hash
                    .as_ref()
                    .map(|h| h.bytes())
                    .unwrap_or_default(),
                data.share_price.val().to_be_bytes().to_vec(),
                data.investors_share.to_u64()?.to_be_bytes().to_vec(),
                data.image_hash
                    .as_ref()
                    .map(|h| h.bytes())
                    .unwrap_or_default(),
                data.social_media_url.as_bytes().to_vec(),
                versions_to_bytes(data.versions())?,
                data.min_raise_target.val().to_be_bytes().to_vec(),
                data.min_raise_target_end_date.0.to_be_bytes().to_vec(),
            ])
            .foreign_assets(vec![data.funds_asset_id.0, data.shares_asset_id])
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
