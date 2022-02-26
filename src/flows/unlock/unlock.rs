use crate::flows::create_project::{share_amount::ShareAmount, storage::load_project::TxId};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::CloseApplication, contract_account::ContractAccount, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn unlock(
    algod: &Algod,
    investor: Address,
    // required to be === held shares (otherwise central app rejects the tx)
    share_amount: ShareAmount,
    shares_asset_id: u64,
    central_app_id: u64,
    locking_escrow: &ContractAccount,
) -> Result<UnlockToSign> {
    let params = algod.suggested_transaction_params().await?;

    // App call to validate the retrieved shares count and clear local state
    let mut central_app_optout_tx = TxnBuilder::with(
        &params,
        CloseApplication::new(investor, central_app_id).build(),
    )
    .build()?;

    // Retrieve investor's assets from locking escrow
    let mut shares_xfer_tx = TxnBuilder::with(
        &params,
        TransferAsset::new(
            *locking_escrow.address(),
            shares_asset_id,
            share_amount.val(),
            investor,
        )
        .build(),
    )
    .build()?;

    // Pay for the shares transfer tx
    let mut pay_shares_xfer_fee_tx = TxnBuilder::with(
        &params,
        Pay::new(
            investor,
            *locking_escrow.address(),
            params.fee.max(params.min_fee),
        )
        .build(),
    )
    .build()?;

    let txs_for_group = vec![
        &mut central_app_optout_tx,
        &mut shares_xfer_tx,
        &mut pay_shares_xfer_fee_tx,
    ];
    TxGroup::assign_group_id(txs_for_group)?;

    let signed_shares_xfer_tx = locking_escrow.sign(&shares_xfer_tx, vec![])?;

    Ok(UnlockToSign {
        central_app_optout_tx,
        shares_xfer_tx: signed_shares_xfer_tx,
        pay_shares_xfer_fee_tx,
    })
}

pub async fn submit_unlock(algod: &Algod, signed: UnlockSigned) -> Result<TxId> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.central_app_optout_tx,
        signed.shares_xfer_tx_signed,
        signed.pay_shares_xfer_fee_tx,
    ];

    // crate::teal::debug_teal_rendered(&txs, "locking_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Unlock tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockToSign {
    pub central_app_optout_tx: Transaction,
    pub shares_xfer_tx: SignedTransaction,
    pub pay_shares_xfer_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockSigned {
    pub central_app_optout_tx: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
    pub pay_shares_xfer_fee_tx: SignedTransaction,
}
