use std::convert::TryInto;

use crate::{
    algo_helpers::calculate_total_fee,
    decimal_util::AsDecimal,
    flows::create_project::{share_amount::ShareAmount, storage::load_project::TxId},
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::{CallApplication, TxnFee},
        contract_account::ContractAccount,
        tx_group::TxGroup,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::{anyhow, Result};
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

    // Funds transfer from escrow to creator
    let harvest_tx = &mut TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *central_escrow.address(),
            funds_asset_id.0,
            amount.val(),
            *harvester,
        )
        .build(),
    )
    .build()?;

    app_call_tx.fee = calculate_total_fee(&params, &[app_call_tx, harvest_tx])?;
    TxGroup::assign_group_id(&mut [app_call_tx, harvest_tx])?;

    let signed_harvest_tx = central_escrow.sign(harvest_tx, vec![])?;

    Ok(HarvestToSign {
        app_call_tx: app_call_tx.clone(),
        harvest_tx: signed_harvest_tx,
    })
}

pub fn harvest_app_call_tx(
    app_id: u64,
    params: &SuggestedTransactionParams,
    sender: &Address,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(params, CallApplication::new(*sender, app_id).build()).build()?;
    Ok(tx)
}

pub async fn submit_harvest(algod: &Algod, signed: &HarvestSigned) -> Result<TxId> {
    log::debug!("Submit harvest..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.app_call_tx_signed.clone(), signed.harvest_tx.clone()];
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "central_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Harvest tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

// consider making this function private and renaming - normally should be investor_can_harvest_amount_calc,
// which takes into account the already harvested amount

pub fn calculate_entitled_harvest(
    central_received_total: FundsAmount,
    share_supply: ShareAmount,
    share_count: ShareAmount,
    precision: u64,
    investors_part: ShareAmount,
) -> Result<FundsAmount> {
    log::debug!("Calculating entitled harvest, central_received_total: {central_received_total:?}, share_supply: {share_supply:?}, share_count: {share_count:?}, precision: {precision:?}, investors_part: {investors_part:?}");

    // for easier understanding we use the same arithmetic as in TEAL

    let investors_share_fractional_percentage = investors_part
        .as_decimal()
        .checked_div(share_supply.as_decimal())
        .ok_or_else(|| {
            anyhow!("investors_part: {investors_part} / share_supply: {share_supply} errored")
        })?;

    ///////////////////////////////////////////////////
    // Calculate entitled_total
    // intermediate steps per operation to map to clear error messages (containing the operands)

    let mul1 = (share_count
        .val()
        .checked_mul(precision)
        .ok_or_else(|| anyhow!("share_count: {share_count} * precision: {precision} errored"))?)
    .as_decimal();

    let percentage_mul_precision = investors_share_fractional_percentage
            .checked_mul(precision.as_decimal())
            .ok_or_else(|| anyhow!("investors_share_fractional_percentage: {investors_share_fractional_percentage} * precision: {precision} errored"))?;

    let mul2 = mul1.checked_mul(percentage_mul_precision).ok_or_else(|| {
        anyhow!("mul1: {mul1} * percentage_mul_precision: {percentage_mul_precision} errored")
    })?;

    let entitled_percentage = (mul2
        .checked_div(share_supply.as_decimal())
        .ok_or_else(|| anyhow!("mul2: {mul2} * share_supply: {share_supply} errored"))?)
    .floor();

    let precision_square = precision
        .as_decimal()
        .checked_mul(precision.as_decimal())
        .ok_or_else(|| anyhow!("precision: {precision} * precision: {precision} errored"))?;

    let mul3 = central_received_total
                .as_decimal()
                .checked_mul(entitled_percentage)
                .ok_or_else(|| anyhow!("central_received_total: {central_received_total} * entitled_percentage: {entitled_percentage} errored"))?;

    let entitled_total = mul3
        .checked_div(precision_square)
        .ok_or_else(|| anyhow!("mul3: {mul3} * precision_square: {precision_square} errored"))?
        .floor();
    ///////////////////////////////////////////////////

    Ok(FundsAmount::new(
        entitled_total
            .to_u128()
            .ok_or_else(|| anyhow!("Couldn't convert entitled_total to u128"))?
            .try_into()?,
    ))
}

pub fn investor_can_harvest_amount_calc(
    central_received_total: FundsAmount,
    harvested_total: FundsAmount,
    share_amount: ShareAmount,
    share_supply: ShareAmount,
    precision: u64,
    investors_part: ShareAmount,
) -> Result<FundsAmount> {
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
    )?;
    Ok(FundsAmount::new(
        entitled_total
            .val()
            .checked_sub(harvested_total.val())
            .ok_or_else(|| {
                anyhow!(
                    "entitled_total: {entitled_total} - harvested_total: {harvested_total} errored"
                )
            })?,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestToSign {
    pub app_call_tx: Transaction,
    pub harvest_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarvestSigned {
    pub app_call_tx_signed: SignedTransaction,
    pub harvest_tx: SignedTransaction,
}
