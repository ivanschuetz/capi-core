use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::TxnFee, contract_account::ContractAccount, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    algo_helpers::calculate_total_fee,
    flows::{create_dao::storage::load_dao::TxId, withdraw::note::withdrawal_to_note},
    funds::{FundsAmount, FundsAssetId},
};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn withdraw(
    algod: &Algod,
    creator: Address,
    funds_asset_id: FundsAssetId,
    inputs: &WithdrawalInputs,
    central_escrow: &ContractAccount,
) -> Result<WithdrawToSign> {
    log::debug!("Creating withdrawal txs..");

    let params = algod.suggested_transaction_params().await?;

    // Funds transfer from escrow to creator
    let mut withdraw_tx = TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *central_escrow.address(),
            funds_asset_id.0,
            inputs.amount.val(),
            creator,
        )
        .build(),
    )
    .note(withdrawal_to_note(inputs)?)
    .build()?;

    // The creator pays the fee of the withdraw tx (signed by central escrow).
    // here we need a dedicated tx, because there's no other txs signed by the creator which could be used to pay the fee.
    let mut pay_withdraw_fee_tx = TxnBuilder::with(
        &params,
        Pay::new(creator, *central_escrow.address(), MicroAlgos(0)).build(),
    )
    .build()?;

    pay_withdraw_fee_tx.fee =
        calculate_total_fee(&params, &[&mut pay_withdraw_fee_tx, &mut withdraw_tx])?;
    TxGroup::assign_group_id(&mut [&mut pay_withdraw_fee_tx, &mut withdraw_tx])?;

    let signed_withdraw_tx = central_escrow.sign(&withdraw_tx, vec![])?;

    Ok(WithdrawToSign {
        withdraw_tx: signed_withdraw_tx,
        pay_withdraw_fee_tx,
    })
}

pub async fn submit_withdraw(algod: &Algod, signed: &WithdrawSigned) -> Result<TxId> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    log::debug!("Submit withdrawal txs..");

    let txs = vec![
        signed.pay_withdraw_fee_tx.clone(),
        signed.withdraw_tx.clone(),
    ];

    // crate::teal::debug_teal_rendered(&txs, "central_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Withdrawal txs tx id: {}", res.tx_id);

    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawToSign {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawSigned {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: SignedTransaction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WithdrawalInputs {
    pub amount: FundsAmount,
    pub description: String,
}
