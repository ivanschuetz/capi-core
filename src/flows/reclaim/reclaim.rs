use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::CallApplication, tx_group::TxGroup, SignedTransaction, Transaction, TransferAsset,
        TxnBuilder,
    },
};
use anyhow::Result;
use mbase::models::{dao_app_id::DaoAppId, funds::FundsAssetId, share_amount::ShareAmount, tx_id::TxId};
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn reclaim(
    algod: &Algod,
    reclaimer: &Address,
    app_id: DaoAppId,
    shares_asset_id: u64,
    share_amount: ShareAmount,
    funds_asset: FundsAssetId,
) -> Result<ReclaimToSign> {
    log::debug!("Generating reclaim txs, reclaimer: {reclaimer:?}, central_app_id: {app_id:?}",);
    let params = algod.suggested_transaction_params().await?;

    // app call
    let mut app_call_tx = TxnBuilder::with(
        &params,
        CallApplication::new(*reclaimer, app_id.0)
            .app_arguments(vec!["reclaim".as_bytes().to_vec()])
            .foreign_assets(vec![funds_asset.0])
            .build(),
    )
    .build()?;

    // pay the send funds back inner tx fee
    app_call_tx.fee = app_call_tx.fee * 2;

    // send shares
    let mut shares_xfer_tx = TxnBuilder::with(
        &params,
        TransferAsset::new(
            *reclaimer,
            shares_asset_id,
            share_amount.val(),
            app_id.address(),
        )
        .build(),
    )
    .build()?;
    TxGroup::assign_group_id(&mut [&mut app_call_tx, &mut shares_xfer_tx])?;

    Ok(ReclaimToSign {
        app_call_tx,
        shares_xfer_tx,
    })
}

pub async fn submit_reclaim(algod: &Algod, signed: &ReclaimSigned) -> Result<TxId> {
    log::debug!("Submit reclaim..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.app_call_tx_signed.clone(),
        signed.shares_xfer_tx_signed.clone(),
    ];

    // crate::dryrun_util::dryrun_all(algod, &txs).await?;
    // mbase::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();
    // mbase::teal::debug_teal_rendered(&txs, "central_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Reclaim tx id: {:?}", res.tx_id);
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimToSign {
    pub app_call_tx: Transaction,
    pub shares_xfer_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReclaimSigned {
    pub app_call_tx_signed: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
}
