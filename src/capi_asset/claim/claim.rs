use crate::{
    capi_asset::{capi_app_id::CapiAppId, capi_asset_id::CapiAssetAmount},
    decimal_util::AsDecimal,
    flows::create_dao::storage::load_dao::TxId,
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::{anyhow, Result};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn claim(
    algod: &Algod,
    claimer: &Address,
    capi_app_id: CapiAppId,
    funds_asset: FundsAssetId,
) -> Result<ClaimToSign> {
    log::debug!(
        "Generating capi claim txs, claimer: {:?}, capi_app_id: {:?}",
        claimer,
        capi_app_id,
    );
    let params = algod.suggested_transaction_params().await?;

    // App call to update user's local state with claimed amount
    let mut app_call_tx = claim_app_call_tx(capi_app_id, &params, claimer, funds_asset)?;

    // pay the fee for the dividend xfer inner tx
    app_call_tx.fee = app_call_tx.fee * 2;

    Ok(ClaimToSign {
        app_call_tx: app_call_tx.clone(),
    })
}

pub fn claim_app_call_tx(
    app_id: CapiAppId,
    params: &SuggestedTransactionParams,
    sender: &Address,
    funds_asset: FundsAssetId,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*sender, app_id.0)
            .app_arguments(vec!["claim".as_bytes().to_vec()])
            .foreign_assets(vec![funds_asset.0])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_claim(algod: &Algod, signed: &ClaimSigned) -> Result<TxId> {
    log::debug!("Submit capi claim..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.app_call_tx_signed.clone()];
    // crate::teal::debug_teal_rendered(&txs, "capi_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Claim tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

fn calculate_capi_entitled_claim(
    received_total: FundsAmount,
    supply: CapiAssetAmount,
    locked_amount: CapiAssetAmount,
    precision: u64,
) -> Result<FundsAmount> {
    // for easier understanding we use the same arithmetic as in TEAL

    let mul1 = locked_amount
        .val()
        .checked_mul(precision)
        .ok_or_else(|| {
            anyhow!("locked_amount: {locked_amount:?} * precision: {precision} errored")
        })?
        .as_decimal();

    let entitled_percentage = mul1
        .checked_div(supply.as_decimal())
        .ok_or_else(|| anyhow!("mul1: {mul1} / supply: {supply:?} errored"))?
        .floor();

    let mul2 = received_total
        .as_decimal()
        .checked_mul(entitled_percentage)
        .ok_or_else(|| {
            anyhow!("received_total: {received_total:?} * entitled_percentage: {entitled_percentage} errored")
        })?;

    let entitled_total = mul2
        .checked_div(precision.as_decimal())
        .ok_or_else(|| anyhow!("mul2: {mul2:?} * precision: {precision} errored"))?
        .floor();

    Ok(FundsAmount::new(entitled_total.to_u64().ok_or_else(
        || anyhow!("Couldn't convert entitled_total to u64"),
    )?))
}

pub fn claimable_capi_dividend(
    app_received_total: FundsAmount,
    claimed_total: FundsAmount,
    locked_amount: CapiAssetAmount,
    supply: CapiAssetAmount,
    precision: u64,
) -> Result<FundsAmount> {
    // Note that this assumes that investor can't unlock only a part of their shares
    // otherwise, the smaller share count would render a small entitled_total_count which would take a while to catch up with claimed_total, which remains unchanged.
    // the easiest solution is to expect the investor to unlock all their shares
    // if they want to sell only a part, they've to opt-in again with the shares they want to keep.

    let entitled_total =
        calculate_capi_entitled_claim(app_received_total, supply, locked_amount, precision)?;

    Ok(FundsAmount::new(
        entitled_total
            .val()
            .checked_sub(claimed_total.val())
            .ok_or_else(|| {
                anyhow!("entitled_total: {entitled_total} - claimed_total: {claimed_total} errored")
            })?,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimToSign {
    pub app_call_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimSigned {
    pub app_call_tx_signed: SignedTransaction,
}
