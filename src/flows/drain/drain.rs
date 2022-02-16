use crate::{
    flows::create_project::storage::load_project::TxId,
    funds::{FundsAmount, FundsAssetId},
    state::account_state::funds_holdings,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::CallApplication, contract_account::ContractAccount, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn drain_customer_escrow(
    algod: &Algod,
    drainer: &Address,
    central_app_id: u64,
    funds_asset_id: FundsAssetId,
    customer_escrow: &ContractAccount,
    central_escrow: &ContractAccount,
) -> Result<DrainCustomerEscrowToSign> {
    let params = algod.suggested_transaction_params().await?;
    let customer_escrow_holdings =
        funds_holdings(algod, customer_escrow.address(), funds_asset_id).await?;
    let amount_to_drain = customer_escrow_holdings;

    let app_call_tx = &mut drain_app_call_tx(central_app_id, &params, drainer)?;

    let pay_fee_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(*drainer, *customer_escrow.address(), FIXED_FEE).build(),
    )
    .build();

    let drain_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        TransferAsset::new(
            *customer_escrow.address(),
            funds_asset_id.0,
            amount_to_drain.0,
            *central_escrow.address(),
        )
        .build(),
    )
    .build();

    TxGroup::assign_group_id(vec![app_call_tx, pay_fee_tx, drain_tx])?;

    let signed_drain_tx = customer_escrow.sign(drain_tx, vec![])?;

    Ok(DrainCustomerEscrowToSign {
        drain_tx: signed_drain_tx,
        pay_fee_tx: pay_fee_tx.clone(),
        app_call_tx: app_call_tx.clone(),
        amount_to_drain,
    })
}

pub fn drain_app_call_tx(
    app_id: u64,
    params: &SuggestedTransactionParams,
    sender: &Address,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(*sender, app_id).build(),
    )
    .build();
    Ok(tx)
}

pub async fn submit_drain_customer_escrow(
    algod: &Algod,
    signed: &DrainCustomerEscrowSigned,
) -> Result<TxId> {
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.pay_fee_tx.clone(),
    //         signed.drain_tx.clone(),
    //     ],
    //     "app_central_approval",
    // )
    // .unwrap();
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.pay_fee_tx.clone(),
    //         signed.drain_tx.clone(),
    //     ],
    //     "customer_escrow",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[
            signed.app_call_tx_signed.clone(),
            signed.pay_fee_tx.clone(),
            signed.drain_tx.clone(),
        ])
        .await?;
    log::debug!("Drain customer escrow tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainCustomerEscrowToSign {
    pub drain_tx: SignedTransaction,
    pub pay_fee_tx: Transaction,
    pub app_call_tx: Transaction,
    pub amount_to_drain: FundsAmount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrainCustomerEscrowSigned {
    pub drain_tx: SignedTransaction,
    pub pay_fee_tx: SignedTransaction,
    pub app_call_tx_signed: SignedTransaction,
}
