use super::create_app::create_app_tx;
use crate::{
    algo_helpers::{send_and_retrieve_app_id, send_and_retrieve_asset_id},
    flows::create_project::{
        create_project::Programs,
        create_project_specs::CreateProjectSpecs,
        model::{CreateAssetsToSign, CreateSharesSpecs},
    },
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{CreateAsset, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use futures::join;

pub async fn create_assets(
    algod: &Algod,
    creator: &Address,
    specs: &CreateProjectSpecs,
    programs: &Programs,
    precision: u64,
) -> Result<CreateAssetsToSign> {
    let params = algod.suggested_transaction_params().await?;
    let create_shares_tx = &mut create_shares_tx(&params, &specs.shares, *creator).await?;

    let create_app_tx = &mut create_app_tx(
        algod,
        &programs.central_app_approval,
        &programs.central_app_clear,
        &creator,
        specs.shares.supply,
        precision,
        specs.investors_part(),
        &params,
    )
    .await?;

    Ok(CreateAssetsToSign {
        create_shares_tx: create_shares_tx.clone(),
        create_app_tx: create_app_tx.clone(),
    })
}

pub async fn submit_create_assets(
    algod: &Algod,
    signed: &CrateDaoAssetsSigned,
) -> Result<CreateAssetsResult> {
    // let txs = vec![signed.create_app.clone()];
    // crate::teal::debug_teal_rendered(&vec![signed.create_app.clone()], "app_central_approval")
    //     .unwrap();

    // Note that we don't use a tx group here but send the 2 transactions separately,
    // When in a group, the resulting pending transaction contains an id (app id / asset id) only for the first tx in the group.
    // see testing::algorand_checks::cannot_create_asset_and_app_in_same_group
    let shares_asset_id_fut = send_and_retrieve_asset_id(algod, &signed.create_shares);
    let app_id_fut = send_and_retrieve_app_id(algod, &signed.create_app);
    let (shares_asset_id_res, app_id_res) = join!(shares_asset_id_fut, app_id_fut);
    let shares_asset_id = shares_asset_id_res?;
    let app_id = app_id_res?;

    log::debug!("Dao assets created. Shares id: {shares_asset_id}, app id: {app_id}");

    Ok(CreateAssetsResult {
        shares_asset_id,
        app_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateDaoAssetsSigned {
    pub create_shares: SignedTransaction,
    pub create_app: SignedTransaction,
}

#[derive(Debug)]
pub struct CreateAssetsResult {
    pub shares_asset_id: u64,
    pub app_id: u64,
}

async fn create_shares_tx(
    tx_params: &SuggestedTransactionParams,
    shares_specs: &CreateSharesSpecs,
    creator: Address,
) -> Result<Transaction> {
    let unit_and_asset_name = shares_specs.token_name.to_owned();
    Ok(TxnBuilder::with(
        tx_params,
        CreateAsset::new(creator, shares_specs.supply.val(), 0, false)
            .unit_name(unit_and_asset_name.clone())
            .asset_name(unit_and_asset_name)
            .build(),
    )
    .build()?)
}
