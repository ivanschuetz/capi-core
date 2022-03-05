use super::create_capi_app::create_app;
use crate::{
    algo_helpers::{send_and_retrieve_app_id, send_and_retrieve_asset_id},
    capi_asset::{
        capi_app_id::CapiAppId,
        capi_asset_id::{CapiAssetAmount, CapiAssetId},
    },
    flows::create_project::create_project::CapiPrograms,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{CreateAsset, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use futures::join;

/// Creates the Capi asset, which entitles holders to a dividend of all the DAOs on the platform.
pub async fn create_capi_assets(
    algod: &Algod,
    supply: CapiAssetAmount,
    creator: &Address,
    params: &SuggestedTransactionParams,
    programs: &CapiPrograms,
    precision: u64,
) -> Result<CreateCapiAssetsToSign> {
    let create_asset = TxnBuilder::with(
        &params,
        CreateAsset::new(*creator, supply.val(), 0, false)
            // Should be called CAPI - for now using a different name to not attract attention on TestNet
            .unit_name("GLOB".to_owned())
            .asset_name("glob".to_owned())
            .build(),
    )
    .build()?;

    let create_app = create_app(
        &algod,
        &programs.app_approval,
        &programs.app_clear,
        &creator,
        supply,
        precision,
        &params,
    )
    .await?;

    Ok(CreateCapiAssetsToSign {
        create_asset,
        create_app,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCapiAssetsToSign {
    pub create_asset: Transaction,
    pub create_app: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCapiAssetsSigned {
    pub create_asset: SignedTransaction,
    pub create_app: SignedTransaction,
}

pub async fn submit_create_capi_assets(
    algod: &Algod,
    signed: &CreateCapiAssetsSigned,
) -> Result<CreateCapiAssetResult> {
    // let txs = vec![signed.create_app.clone()];
    // crate::teal::debug_teal_rendered(&vec![signed.create_app.clone()], "app_central_approval")
    //     .unwrap();

    // Note that we don't use a tx group here but send the 2 transactions separately,
    // When in a group, the resulting pending transaction contains an id (app id / asset id) only for the first tx in the group.
    // see testing::algorand_checks::cannot_create_asset_and_app_in_same_group
    let asset_id_fut = send_and_retrieve_asset_id(algod, &signed.create_asset);
    let app_id_fut = send_and_retrieve_app_id(algod, &signed.create_app);
    let (asset_id_res, app_id_res) = join!(asset_id_fut, app_id_fut);
    let asset_id = asset_id_res?;
    let app_id = app_id_res?;

    Ok(CreateCapiAssetResult {
        asset_id: CapiAssetId(asset_id),
        app_id: CapiAppId(app_id),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCapiAssetResult {
    pub asset_id: CapiAssetId,
    pub app_id: CapiAppId,
}
