use crate::flows::{create_dao::storage::load_dao::TxId, withdraw::note::withdrawal_to_note};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::models::{
    dao_app_id::DaoAppId,
    funds::{FundsAmount, FundsAssetId},
};
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn withdraw(
    algod: &Algod,
    creator: Address,
    inputs: &WithdrawalInputs,
    app_id: DaoAppId,
    funds_asset: FundsAssetId,
) -> Result<WithdrawToSign> {
    log::debug!("Creating withdrawal txs..");

    let params = algod.suggested_transaction_params().await?;

    let mut app_call_tx = TxnBuilder::with(
        &params,
        CallApplication::new(creator, app_id.0)
            .app_arguments(vec![
                "withdraw".as_bytes().to_vec(),
                inputs.amount.to_bytes(),
            ])
            .foreign_assets(vec![funds_asset.0])
            .build(),
    )
    .note(withdrawal_to_note(inputs)?)
    .build()?;

    // pay for the inner xfer fee
    app_call_tx.fee = app_call_tx.fee + params.min_fee;

    Ok(WithdrawToSign {
        withdraw_tx: app_call_tx,
    })
}

pub async fn submit_withdraw(algod: &Algod, signed: &WithdrawSigned) -> Result<TxId> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);
    log::debug!("Submit withdrawal txs..");

    let txs = vec![signed.withdraw_tx.clone()];

    // crate::dryrun_util::dryrun_all(algod, &txs).await?;
    // crate::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Withdrawal txs tx id: {}", res.tx_id);

    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawToSign {
    pub withdraw_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawSigned {
    pub withdraw_tx: SignedTransaction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WithdrawalInputs {
    pub amount: FundsAmount,
    pub description: String,
}
