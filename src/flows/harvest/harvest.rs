use crate::{
    decimal_util::AsDecimal,
    flows::create_project::{share_amount::ShareAmount, storage::load_project::TxId},
    funds::{FundsAmount, FundsAssetId},
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
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn harvest(
    algod: &Algod,
    harvester: &Address,
    central_app_id: u64,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
    central_escrow: &ContractAccount,
) -> Result<HarvestToSign> {
    log::debug!("Generating harvest txs, harvester: {:?}, central_app_id: {:?}, amount: {:?}, central_escrow: {:?}", harvester, central_app_id, amount, central_escrow);
    let params = algod.suggested_transaction_params().await?;

    // App call to update user's local state with harvested amount
    let app_call_tx = &mut harvest_app_call_tx(central_app_id, &params, harvester)?;

    // The harvester pays the fee of the harvest tx (signed by central escrow)
    let pay_fee_tx = &mut TxnBuilder::with(
        params.clone(),
        Pay::new(
            *harvester,
            *central_escrow.address(),
            params.fee.max(params.min_fee),
        )
        .build(),
    )
    .build();

    // Funds transfer from escrow to creator
    let harvest_tx = &mut TxnBuilder::with(
        params,
        TransferAsset::new(
            *central_escrow.address(),
            funds_asset_id.0,
            amount.0,
            *harvester,
        )
        .build(),
    )
    .build();

    TxGroup::assign_group_id(vec![app_call_tx, pay_fee_tx, harvest_tx])?;

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
        params.to_owned(),
        CallApplication::new(*sender, app_id).build(),
    )
    .build();
    Ok(tx)
}

pub async fn submit_harvest(algod: &Algod, signed: &HarvestSigned) -> Result<TxId> {
    log::debug!("Submit harvest..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.app_call_tx_signed.clone(),
        signed.pay_fee_tx.clone(),
        signed.harvest_tx.clone(),
    ];
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Harvest tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

pub fn calculate_entitled_harvest(
    central_received_total: FundsAmount,
    share_supply: ShareAmount,
    share_count: ShareAmount,
    precision: u64,
    investors_part: ShareAmount,
) -> FundsAmount {
    // TODO review possible overflow, type cast, unwrap
    // for easier understanding we use the same arithmetic as in TEAL
    let investors_share_fractional_percentage = investors_part.0.as_decimal() / 100.as_decimal(); // e.g. 10% -> 0.1

    let entitled_percentage = ((share_count.0 * precision).as_decimal()
        * (investors_share_fractional_percentage * precision.as_decimal())
        / share_supply.0.as_decimal())
    .floor();

    let entitled_total = ((central_received_total.0.as_decimal() * entitled_percentage)
        / (precision.as_decimal() * precision.as_decimal()))
    .floor();

    FundsAmount(entitled_total.to_u128().unwrap() as u64)
}

pub fn investor_can_harvest_amount_calc(
    central_received_total: FundsAmount,
    harvested_total: FundsAmount,
    share_amount: ShareAmount,
    share_supply: ShareAmount,
    precision: u64,
    investors_part: ShareAmount,
) -> FundsAmount {
    // Note that this assumes that investor can't unlock only a part of their shares
    // otherwise, the smaller share count would render a small entitled_total_count which would take a while to catch up with harvested_total, which remains unchanged.
    // the easiest solution is to expect the investor to unlock all their shares
    // if they want to sell only a part, they've to opt-in again with the shares they want to keep.

    let entitled_total = calculate_entitled_harvest(
        central_received_total,
        share_supply,
        share_amount,
        precision,
        investors_part,
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
