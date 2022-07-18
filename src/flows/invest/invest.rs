use crate::flows::create_dao::model::Dao;

use super::model::{InvestResult, InvestSigned, InvestToSign};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{
        builder::CallApplication, tx_group::TxGroup, AcceptAsset, Transaction, TransferAsset,
        TxnBuilder,
    },
};
use anyhow::{anyhow, Result};
use mbase::models::{
    dao_app_id::DaoAppId,
    funds::{FundsAmount, FundsAssetId},
    share_amount::ShareAmount,
};

/// Requires investor to opt in to the app first,
/// we can't do it here: setting local state errors if during opt-in
#[allow(clippy::too_many_arguments)]
pub async fn invest_txs(
    algod: &Algod,
    dao: &Dao,
    investor: &Address,
    app_id: DaoAppId,
    shares_asset_id: u64,
    share_amount: ShareAmount,
    funds_asset_id: FundsAssetId,
    // TODO remove: share_price is in the dao
    share_price: FundsAmount,
) -> Result<InvestToSign> {
    log::debug!("Investing in dao: {:?}", dao);

    let params = algod.suggested_transaction_params().await?;

    let total_price = FundsAmount::new(
        share_price
            .val()
            .checked_mul(share_amount.val())
            .ok_or_else(|| {
                anyhow!(
        "Share price: {share_price} multiplied by share amount: {share_amount} caused an overflow."
    )
            })?,
    );

    let mut central_app_investor_setup_tx =
        dao_app_investor_setup_tx(&params, app_id, shares_asset_id, *investor, share_amount)?;

    let mut shares_optin_tx = TxnBuilder::with(
        &params,
        AcceptAsset::new(*investor, dao.shares_asset_id).build(),
    )
    .build()?;

    let mut pay_price_tx = TxnBuilder::with(
        &params,
        TransferAsset::new(
            *investor,
            funds_asset_id.0,
            total_price.val(),
            dao.app_address(),
        )
        .build(),
    )
    // DON'T CHANGE NOTE - to not apply fee when fetching received_payments with the indexer
    // TODO solve this in a different way - note is shown on the UI
    .note("Invest".as_bytes().to_vec())
    .build()?;

    // pay for the inner xfer (shares to investor) fee
    // note that we don't pay for the rest of the tx's fees as they are regular txs signed by the user
    central_app_investor_setup_tx.fee = central_app_investor_setup_tx.fee * 2;
    TxGroup::assign_group_id(&mut [
        &mut shares_optin_tx,
        &mut central_app_investor_setup_tx,
        &mut pay_price_tx,
    ])?;

    Ok(InvestToSign {
        dao: dao.to_owned(),
        central_app_setup_tx: central_app_investor_setup_tx,
        payment_tx: pay_price_tx,
        shares_asset_optin_tx: shares_optin_tx,
        total_price,
    })
}

pub fn dao_app_investor_setup_tx(
    params: &SuggestedTransactionParams,
    app_id: DaoAppId,
    shares_asset_id: u64,
    investor: Address,
    share_amount: ShareAmount,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(investor, app_id.0)
            .foreign_assets(vec![shares_asset_id])
            .app_arguments(vec![
                "invest".as_bytes().to_vec(),
                share_amount.val().to_be_bytes().to_vec(),
            ])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_invest(algod: &Algod, signed: &InvestSigned) -> Result<InvestResult> {
    log::debug!("Submitting investing txs..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.shares_asset_optin_tx.clone(),
        signed.central_app_setup_tx.clone(),
        signed.payment_tx.clone(),
    ];

    // mbase::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    Ok(InvestResult {
        tx_id: res.tx_id.parse()?,
        dao: signed.dao.clone(),
        central_app_investor_setup_tx: signed.central_app_setup_tx.clone(),
        payment_tx: signed.payment_tx.clone(),
        shares_asset_optin_tx: signed.shares_asset_optin_tx.clone(),
    })
}
