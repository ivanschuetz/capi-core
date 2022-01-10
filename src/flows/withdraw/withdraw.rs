use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        contract_account::ContractAccount, tx_group::TxGroup, Pay, SignedTransaction, Transaction,
        TxnBuilder,
    },
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::withdrawal_note_prefix::generate_withdrawal_tx_note;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn withdraw(
    algod: &Algod,
    creator: Address,
    inputs: &WithdrawalInputs,
    central_escrow: &ContractAccount,
) -> Result<WithdrawToSign> {
    log::debug!("Creating withdrawal txs..");

    let params = algod.suggested_transaction_params().await?;

    // Funds transfer from escrow to creator
    let mut withdraw_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(*central_escrow.address(), creator, inputs.amount).build(),
    )
    .note(to_note(inputs)?)
    .build();

    // The creator pays the fee of the withdraw tx (signed by central escrow)
    let mut pay_withdraw_fee_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        Pay::new(creator, *central_escrow.address(), FIXED_FEE).build(),
    )
    .build();

    TxGroup::assign_group_id(vec![&mut withdraw_tx, &mut pay_withdraw_fee_tx])?;

    let signed_withdraw_tx = central_escrow.sign(&withdraw_tx, vec![])?;

    Ok(WithdrawToSign {
        withdraw_tx: signed_withdraw_tx,
        pay_withdraw_fee_tx,
    })
}

fn to_note(withdrawal: &WithdrawalInputs) -> Result<Vec<u8>> {
    // TODO compression, e.g. https://github.com/silentsokolov/rust-smaz
    // in a test it compressed ~40% of regular english text (from random wikipedia article)
    // it increased WASM file size by only ~16kb
    let body = withdrawal.description.clone();
    // The reason that we generate the note with a UUID and not the hash,
    // is that we want to verify in TEAL that this UUID is used (so all the transactions can be found by prefix)
    Ok(generate_withdrawal_tx_note(&body))
}

pub async fn submit_withdraw(algod: &Algod, signed: &WithdrawSigned) -> Result<String> {
    log::debug!("Submit withdrawal txs..");

    let txs = vec![
        signed.withdraw_tx.clone(),
        signed.pay_withdraw_fee_tx.clone(),
    ];

    // crate::teal::debug_teal_rendered(&txs, "central_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Withdrawal txs tx id: {}", res.tx_id);

    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawToSign {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawSigned {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: SignedTransaction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WithdrawalInputs {
    pub project_uuid: Uuid,
    pub amount: MicroAlgos,
    pub description: String,
}
