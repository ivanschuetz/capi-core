use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::{anyhow, Result};
use mbase::{
    models::{
        dao_app_id::DaoAppId,
        funds::{FundsAmount, FundsAssetId},
        share_amount::ShareAmount,
        shares_percentage::SharesPercentage,
        tx_id::TxId,
    },
    util::decimal_util::AsDecimal,
};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn claim(
    algod: &Algod,
    claimer: &Address,
    app_id: DaoAppId,
    funds_asset: FundsAssetId,
) -> Result<ClaimToSign> {
    log::debug!(
        "Generating claim txs, claimer: {:?}, central_app_id: {:?}",
        claimer,
        app_id,
    );
    let params = algod.suggested_transaction_params().await?;

    // claim and update user's local state with claimed amount
    let mut app_call_tx = claim_app_call_tx(app_id, &params, claimer, funds_asset)?;
    // pay the send divident inner tx fee
    app_call_tx.fee = app_call_tx.fee * 2;

    Ok(ClaimToSign { app_call_tx })
}

pub fn claim_app_call_tx(
    app_id: DaoAppId,
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
    log::debug!("Submit claim..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.app_call_tx_signed.clone()];

    // crate::dryrun_util::dryrun_all(algod, &txs).await?;
    // mbase::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Claim tx id: {:?}", res.tx_id);
    res.tx_id.parse()
}

/// The amount the investor is entitled to claim, based on locked shares, the investor's % defined by the dao,
/// the share supply, and the total received global state ("income").
///
/// Does not account for already claimed funds.
///
/// Formula: received_total * (locked * investors_% / supply)
/// where:
/// received_total: the global state that tracks all the income the app has ever received
/// locked: the shares amount locked by the investor
/// investors_%: the share of income reserved to investors, defined when creating the dao
/// supply: the share asset supply
fn total_entitled_dividend(
    central_received_total: FundsAmount,
    share_supply: ShareAmount,
    locked_amount: ShareAmount,
    precision: u64,
    investors_share: SharesPercentage,
) -> Result<FundsAmount> {
    log::debug!("Calculating entitled claim, central_received_total: {central_received_total:?}, share_supply: {share_supply:?}, locked_amount: {locked_amount:?}, precision: {precision:?}, investors_share: {investors_share:?}");

    // for easier understanding we use the same arithmetic as in TEAL

    ///////////////////////////////////////////////////
    // Calculate entitled_total
    // intermediate steps per operation to map to clear error messages (containing the operands)

    let mul1 = (locked_amount.val().checked_mul(precision).ok_or_else(|| {
        anyhow!("locked_amount: {locked_amount} * precision: {precision} errored")
    })?)
    .as_decimal();

    let percentage_mul_precision = investors_share.value()
            .checked_mul(precision.as_decimal())
            .ok_or_else(|| anyhow!("investors_share_fractional_percentage: {investors_share:?} * precision: {precision} errored"))?;

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

    Ok(FundsAmount::new(entitled_total.to_u64().ok_or_else(
        || anyhow!("Couldn't convert entitled_total to u64"),
    )?))
}

/// The max amount an investor can claim, based on locked shares, total received global state and the already claimed amount.
pub fn claimable_dividend(
    central_received_total: FundsAmount,
    claimed_total: FundsAmount,
    share_supply: ShareAmount,
    share_amount: ShareAmount,
    precision: u64,
    investors_share: SharesPercentage,
) -> Result<FundsAmount> {
    // Note that this assumes that investor can't unlock only a part of their shares
    // otherwise, the smaller share count would render a small entitled_total_count which would take a while to catch up with claimed_total, which remains unchanged.
    // the easiest solution is to expect the investor to unlock all their shares
    // if they want to sell only a part, they've to opt-in again with the shares they want to keep.

    let entitled_total = total_entitled_dividend(
        central_received_total,
        share_supply,
        share_amount,
        precision,
        investors_share,
    )?;

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
