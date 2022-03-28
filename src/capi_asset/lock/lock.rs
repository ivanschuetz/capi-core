use crate::{
    capi_asset::{
        capi_app_id::CapiAppId,
        capi_asset_id::{CapiAssetAmount, CapiAssetId},
    },
    flows::create_dao::storage::load_dao::TxId,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::CallApplication, tx_group::TxGroup, SignedTransaction, Transaction, TransferAsset,
        TxnBuilder,
    },
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

/// Note that this is only for shares that have been bought in the market
/// The investing flow doesn't use this: there's an xfer from the investing account to the locking escrow in the investing tx group
pub async fn lock_capi_assets(
    algod: &Algod,
    investor: &Address,
    asset_amount: CapiAssetAmount,
    capi_asset_id: CapiAssetId,
    capi_app_id: CapiAppId,
    capi_escrow: &Address,
) -> Result<LockToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Init investor's local state
    let mut app_call_tx = TxnBuilder::with(
        &params,
        CallApplication::new(*investor, capi_app_id.0)
            .app_arguments(vec!["lock".as_bytes().to_vec()])
            .build(),
    )
    .build()?;

    // Send holder's assets to lock escrow
    let mut shares_xfer_tx = TxnBuilder::with(
        &params,
        TransferAsset::new(*investor, capi_asset_id.0, asset_amount.val(), *capi_escrow).build(),
    )
    .build()?;

    let txs_for_group = &mut [&mut app_call_tx, &mut shares_xfer_tx];
    TxGroup::assign_group_id(txs_for_group)?;

    Ok(LockToSign {
        capi_app_call_setup_tx: app_call_tx.clone(),
        shares_xfer_tx: shares_xfer_tx.clone(),
    })
}

pub async fn submit_capi_assets_lock(algod: &Algod, signed: LockSigned) -> Result<TxId> {
    log::debug!("Submit capi asset lock..");
    let txs = vec![
        signed.capi_app_call_setup_tx.clone(),
        signed.shares_xfer_tx_signed.clone(),
    ];
    // crate::teal::debug_teal_rendered(&txs, "capi_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Lock tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockToSign {
    pub capi_app_call_setup_tx: Transaction,
    pub shares_xfer_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockSigned {
    pub capi_app_call_setup_tx: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
}
