use crate::{
    api::version::VersionedTealSourceTemplate,
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    flows::create_dao::{
        create_dao_specs::CreateDaoSpecs,
        model::{CreateAssetsToSign, CreateSharesSpecs},
        storage::load_dao::DaoAppId,
    },
    network_util::wait_for_pending_transaction,
};
use algonaut::{
    algod::v2::Algod,
    core::{to_app_address, Address, SuggestedTransactionParams},
    model::algod::v2::PendingTransaction,
    transaction::{CreateAsset, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::{anyhow, Result};
use futures::join;

use super::create_app::create_app_tx;

pub async fn create_assets(
    algod: &Algod,
    creator: &Address,
    owner: &Address,
    specs: &CreateDaoSpecs,
    app_approval: &VersionedTealSourceTemplate,
    app_clear: &VersionedTealSourceTemplate,
    precision: u64,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<CreateAssetsToSign> {
    let params = algod.suggested_transaction_params().await?;
    let create_shares_tx = &mut create_shares_tx(&params, &specs.shares, *creator).await?;

    let create_app_tx = &mut create_app_tx(
        algod,
        &app_approval,
        &app_clear,
        creator,
        owner,
        specs.shares.supply,
        precision,
        specs.investors_part(),
        &params,
        capi_deps,
        specs.share_price,
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
    // crate::teal::debug_teal_rendered(&vec![signed.create_app.clone()], "dao_app_approval")
    //     .unwrap();

    // Note that we don't use a tx group here but send the 2 transactions separately,
    // When in a group, the resulting pending transaction contains an id (app id / asset id) only for the first tx in the group.
    // see testing::algorand_checks::cannot_create_asset_and_app_in_same_group
    let shares_asset_id_fut = send_and_retrieve_asset_id(algod, &signed.create_shares);
    let app_id_fut = send_and_retrieve_app_id(algod, &signed.create_app);
    let (shares_asset_id_res, app_id_res) = join!(shares_asset_id_fut, app_id_fut);
    let shares_asset_id = shares_asset_id_res?;
    let app_id = app_id_res?;

    let app_address = to_app_address(app_id.0);

    log::debug!("Dao assets created. Shares id: {shares_asset_id}, app id: {app_id:?}, app address: {app_address:?}");

    Ok(CreateAssetsResult {
        shares_asset_id,
        app_id,
    })
}

async fn send_and_retrieve_asset_id(algod: &Algod, tx: &SignedTransaction) -> Result<u64> {
    let p_tx = send_and_wait_for_pending_tx(algod, tx).await?;
    p_tx.asset_index
        .ok_or_else(|| anyhow!("Shares asset id in pending tx not set"))
}

async fn send_and_retrieve_app_id(algod: &Algod, tx: &SignedTransaction) -> Result<DaoAppId> {
    let p_tx = send_and_wait_for_pending_tx(algod, tx).await?;
    Ok(DaoAppId(
        p_tx.application_index
            .ok_or_else(|| anyhow!("App id in pending tx not set"))?,
    ))
}

async fn send_and_wait_for_pending_tx(
    algod: &Algod,
    tx: &SignedTransaction,
) -> Result<PendingTransaction> {
    let res = algod.broadcast_signed_transaction(tx).await?;
    wait_for_pending_transaction(algod, &res.tx_id.parse()?)
        .await?
        .ok_or_else(|| anyhow!("No pending tx to retrieve asset_od"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateDaoAssetsSigned {
    pub create_shares: SignedTransaction,
    pub create_app: SignedTransaction,
}

#[derive(Debug)]
pub struct CreateAssetsResult {
    pub shares_asset_id: u64,
    pub app_id: DaoAppId,
}

async fn create_shares_tx(
    params: &SuggestedTransactionParams,
    shares_specs: &CreateSharesSpecs,
    creator: Address,
) -> Result<Transaction> {
    let unit_and_asset_name = shares_specs.token_name.to_owned();
    Ok(TxnBuilder::with(
        params,
        CreateAsset::new(creator, shares_specs.supply.val(), 0, false)
            .unit_name(unit_and_asset_name.clone())
            .asset_name(unit_and_asset_name)
            .build(),
    )
    .build()?)
}
