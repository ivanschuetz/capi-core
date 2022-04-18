#[cfg(not(target_arch = "wasm32"))]
use crate::{
    capi_asset::{capi_app_id::CapiAppId, capi_asset_id::CapiAssetId},
    flows::create_dao::storage::load_dao::TxId,
    funds::FundsAssetId,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::Serialize;

// TODO no constants
// 2 assets (funds asset and capi asset)
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(300_000);

pub async fn setup_capi_escrow(
    owner: &Address,
    params: &SuggestedTransactionParams,
    capi_asset_id: CapiAssetId,
    funds_asset_id: FundsAssetId,
    app_id: CapiAppId,
) -> Result<SetupCentralEscrowToSign> {
    let mut app_call = app_setup_tx(owner, params, app_id, funds_asset_id, capi_asset_id).await?;

    // pay the optins inner txs fees (to capi asset and funds asset)
    app_call.fee = app_call.fee * 3;

    Ok(SetupCentralEscrowToSign {
        app_call_tx: app_call.clone(),
    })
}

pub async fn submit_setup_capi_escrow(
    algod: &Algod,
    signed: &SetupCentralEscrowSigned,
) -> Result<TxId> {
    log::debug!("Will submit setup capi escrow..");
    let txs = vec![signed.app_call_tx.clone()];

    // crate::teal::debug_teal_rendered(&txs, "capi_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Payment tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

async fn app_setup_tx(
    sender: &Address,
    params: &SuggestedTransactionParams,
    app_id: CapiAppId,
    funds_asset: FundsAssetId,
    capi_asset: CapiAssetId,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*sender, app_id.0)
            .foreign_assets(vec![funds_asset.0, capi_asset.0])
            .app_arguments(vec!["setup".as_bytes().to_vec()])
            .build(),
    )
    .build()?;

    Ok(tx.clone())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCentralEscrowToSign {
    pub app_call_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCentralEscrowSigned {
    pub app_call_tx: SignedTransaction,
}

#[derive(Serialize)]
struct CapiEscrowTemplateContext {
    capi_asset_id: String,
    funds_asset_id: String,
    app_id: String,
}

#[derive(Serialize)]
struct SomeContext {
    address: String,
}
