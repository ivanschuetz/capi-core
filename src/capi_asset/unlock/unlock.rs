use crate::{
    algo_helpers::calculate_total_fee,
    capi_asset::{
        capi_app_id::CapiAppId,
        capi_asset_id::{CapiAssetAmount, CapiAssetId},
    },
    flows::create_project::storage::load_project::TxId,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::{CloseApplication, TxnFee},
        contract_account::ContractAccount,
        tx_group::TxGroup,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn unlock_capi_assets(
    algod: &Algod,
    investor: &Address,
    // required to be === held shares (otherwise app rejects the tx)
    share_amount: CapiAssetAmount,
    shares_asset_id: CapiAssetId,
    capi_app_id: CapiAppId,
    capi_escrow: &ContractAccount,
) -> Result<UnlockToSign> {
    let params = algod.suggested_transaction_params().await?;

    // App call to validate the retrieved shares count and clear local state
    let capi_app_optout_tx = &mut TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        CloseApplication::new(*investor, capi_app_id.0)
            .app_arguments(vec!["unlock".as_bytes().to_vec()])
            .build(),
    )
    .build()?;

    // Retrieve investor's assets from locking escrow
    let shares_xfer_tx = &mut TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *capi_escrow.address(),
            shares_asset_id.0,
            share_amount.val(),
            *investor,
        )
        .build(),
    )
    .build()?;

    capi_app_optout_tx.fee = calculate_total_fee(&params, &[capi_app_optout_tx, shares_xfer_tx])?;
    TxGroup::assign_group_id(&mut [capi_app_optout_tx, shares_xfer_tx])?;

    let signed_shares_xfer_tx = capi_escrow.sign(shares_xfer_tx, vec![])?;

    Ok(UnlockToSign {
        capi_app_optout_tx: capi_app_optout_tx.clone(),
        shares_xfer_tx: signed_shares_xfer_tx,
    })
}

pub async fn submit_capi_assets_unlock(algod: &Algod, signed: UnlockSigned) -> Result<TxId> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.capi_app_optout_tx, signed.shares_xfer_tx_signed];

    // crate::teal::debug_teal_rendered(&txs, "capi_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "app_capi_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Unlock tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockToSign {
    pub capi_app_optout_tx: Transaction,
    pub shares_xfer_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockSigned {
    pub capi_app_optout_tx: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
}
