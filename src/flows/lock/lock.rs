use crate::flows::create_project::{
    share_amount::ShareAmount,
    storage::load_project::{ProjectId, TxId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::CallApplication, contract_account::ContractAccount, tx_group::TxGroup,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

/// Note that this is only for shares that have been bought in the market
/// The investing flow doesn't use this: there's an xfer from the investing account to the locking escrow in the investing tx group
pub async fn lock(
    algod: &Algod,
    investor: Address,
    share_amount: ShareAmount,
    shares_asset_id: u64,
    central_app_id: u64,
    locking_escrow: &ContractAccount,
    project_id: &ProjectId,
) -> Result<LockToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Central app setup app call (init investor's local state)
    let mut app_call_tx = TxnBuilder::with(
        params.clone(),
        CallApplication::new(investor, central_app_id)
            .app_arguments(vec![project_id.bytes().to_vec()])
            .build(),
    )
    .build();

    // Send investor's assets to lock escrow
    let mut shares_xfer_tx = TxnBuilder::with(
        params,
        TransferAsset::new(
            investor,
            shares_asset_id,
            share_amount.0,
            *locking_escrow.address(),
        )
        .build(),
    )
    .build();

    let txs_for_group = vec![&mut app_call_tx, &mut shares_xfer_tx];
    TxGroup::assign_group_id(txs_for_group)?;

    Ok(LockToSign {
        central_app_call_setup_tx: app_call_tx.clone(),
        shares_xfer_tx: shares_xfer_tx.clone(),
    })
}

pub async fn submit_lock(algod: &Algod, signed: LockSigned) -> Result<TxId> {
    let txs = vec![
        signed.central_app_call_setup_tx.clone(),
        signed.shares_xfer_tx_signed.clone(),
    ];
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();
    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Lock tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockToSign {
    pub central_app_call_setup_tx: Transaction,
    pub shares_xfer_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockSigned {
    pub central_app_call_setup_tx: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
}
