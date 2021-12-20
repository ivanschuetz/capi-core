use crate::decimal_util::AsDecimal;
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CallApplication, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn harvest(
    algod: &Algod,
    harvester: &Address,
    central_app_id: u64,
    amount: MicroAlgos,
    central_escrow: &ContractAccount,
) -> Result<HarvestToSign> {
    log::debug!("Generating harvest txs, harvester: {:?}, central_app_id: {:?}, amount: {:?}, central_escrow: {:?}", harvester, central_app_id, amount, central_escrow);
    let params = algod.suggested_transaction_params().await?;

    // Escrow call to harvest the amount
    let harvest_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(central_escrow.address, *harvester, amount).build(),
    )
    .build();

    // The harvester pays the fee of the harvest tx (signed by central escrow)
    let pay_fee_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(*harvester, central_escrow.address, FIXED_FEE).build(),
    )
    .build();

    // App call to update user's local state with harvested amount
    let app_call_tx = &mut harvest_app_call_tx(central_app_id, &params, harvester)?;

    TxGroup::assign_group_id(vec![app_call_tx, harvest_tx, pay_fee_tx])?;

    let signed_harvest_tx = central_escrow.sign(harvest_tx, vec![])?;

    Ok(HarvestToSign {
        app_call_tx: app_call_tx.clone(),
        harvest_tx: signed_harvest_tx,
        pay_fee_tx: pay_fee_tx.clone(),
    })
}

pub fn harvest_app_call_tx(
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

pub async fn submit_harvest(algod: &Algod, signed: &HarvestSigned) -> Result<String> {
    log::debug!("Submit harvest..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.app_call_tx_signed.clone(),
        signed.harvest_tx.clone(),
        signed.pay_fee_tx.clone(),
    ];
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Harvest tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

pub fn calculate_entitled_harvest(
    central_received_total: MicroAlgos,
    share_supply: u64,
    share_count: u64,
    precision: u64,
    investors_share: u64,
) -> MicroAlgos {
    // TODO review possible overflow, type cast, unwrap
    // for easier understanding we use the same arithmetic as in TEAL
    let investors_share_fractional_percentage = investors_share.as_decimal() / 100.as_decimal(); // e.g. 10% -> 0.1

    let entitled_percentage = ((share_count * precision).as_decimal()
        * (investors_share_fractional_percentage * precision.as_decimal())
        / share_supply.as_decimal())
    .floor();

    let entitled_total = ((central_received_total.0.as_decimal() * entitled_percentage)
        / (precision.as_decimal() * precision.as_decimal()))
    .floor();

    MicroAlgos(entitled_total.to_u128().unwrap() as u64)
}

pub fn investor_can_harvest_amount_calc(
    central_received_total: MicroAlgos,
    harvested_total: MicroAlgos,
    share_count: u64,
    share_supply: u64,
    precision: u64,
    investors_share: u64,
) -> MicroAlgos {
    // Note that this assumes that investor can't unstake only a part of their shares
    // otherwise, the smaller share count would render a small entitled_total_count which would take a while to catch up with harvested_total, which remains unchanged.
    // the easiest solution is to expect the investor to unstake all their shares
    // if they want to sell only a part, they've to opt-in again with the shares they want to keep.

    let entitled_total = calculate_entitled_harvest(
        central_received_total,
        share_supply,
        share_count,
        precision,
        investors_share,
    );
    entitled_total - harvested_total
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestToSign {
    pub app_call_tx: Transaction,
    pub harvest_tx: SignedTransaction,
    pub pay_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarvestSigned {
    pub app_call_tx_signed: SignedTransaction,
    pub harvest_tx: SignedTransaction,
    pub pay_fee_tx: SignedTransaction,
}
