use super::model::{InvestResult, InvestSigned, InvestToSign};
use crate::{
    algo_helpers::calculate_total_fee,
    flows::create_dao::{
        model::Dao,
        share_amount::ShareAmount,
        storage::load_dao::{DaoAppId, DaoId},
    },
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{
        builder::{CallApplication, TxnFee},
        tx_group::TxGroup,
        AcceptAsset, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::{anyhow, Result};

/// Requires investor to opt in to the app first,
/// we can't do it here: setting local state errors if during opt-in
#[allow(clippy::too_many_arguments)]
pub async fn invest_txs(
    algod: &Algod,
    dao: &Dao,
    investor: &Address,
    locking_escrow: &Address,
    app_id: DaoAppId,
    shares_asset_id: u64,
    share_amount: ShareAmount,
    funds_asset_id: FundsAssetId,
    share_price: FundsAmount,
) -> Result<InvestToSign> {
    log::debug!("Investing in dao: {:?}", dao);

    let params = algod.suggested_transaction_params().await?;

    let total_price = share_price
        .val()
        .checked_mul(share_amount.val())
        .ok_or(anyhow!(
        "Share price: {share_price} multiplied by share amount: {share_amount} caused an overflow."
    ))?;

    let mut central_app_investor_setup_tx =
        dao_app_investor_setup_tx(&params, app_id, shares_asset_id, *investor, dao.id())?;

    let mut pay_price_tx = TxnBuilder::with(
        &params,
        TransferAsset::new(
            *investor,
            funds_asset_id.0,
            total_price,
            *dao.central_escrow.address(),
        )
        .build(),
    )
    .build()?;

    let mut shares_optin_tx = TxnBuilder::with(
        &params,
        AcceptAsset::new(*investor, dao.shares_asset_id).build(),
    )
    .build()?;

    let mut receive_shares_asset_tx = TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *dao.invest_escrow.address(),
            dao.shares_asset_id,
            share_amount.val(),
            *locking_escrow,
        )
        .build(),
    )
    .build()?;

    central_app_investor_setup_tx.fee = calculate_total_fee(
        &params,
        &[&central_app_investor_setup_tx, &receive_shares_asset_tx],
    )?;
    TxGroup::assign_group_id(&mut [
        &mut central_app_investor_setup_tx,
        &mut receive_shares_asset_tx,
        &mut pay_price_tx,
        &mut shares_optin_tx,
    ])?;

    let receive_shares_asset_signed_tx = dao.invest_escrow.sign(receive_shares_asset_tx, vec![])?;

    Ok(InvestToSign {
        dao: dao.to_owned(),
        central_app_setup_tx: central_app_investor_setup_tx.clone(),
        payment_tx: pay_price_tx.clone(),
        shares_asset_optin_tx: shares_optin_tx.clone(),
        shares_xfer_tx: receive_shares_asset_signed_tx,
    })
}

pub fn dao_app_investor_setup_tx(
    params: &SuggestedTransactionParams,
    app_id: DaoAppId,
    shares_asset_id: u64,
    investor: Address,
    dao_id: DaoId,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(investor, app_id.0)
            .foreign_assets(vec![shares_asset_id])
            .app_arguments(vec!["invest".as_bytes().to_vec(), dao_id.bytes().to_vec()])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_invest(algod: &Algod, signed: &InvestSigned) -> Result<InvestResult> {
    log::debug!("Submitting investing txs..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.central_app_setup_tx.clone(),
        signed.shares_xfer_tx.clone(),
        signed.payment_tx.clone(),
        signed.shares_asset_optin_tx.clone(),
    ];

    // crate::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "investing_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    Ok(InvestResult {
        tx_id: res.tx_id.parse()?,
        dao: signed.dao.clone(),
        central_app_investor_setup_tx: signed.central_app_setup_tx.clone(),
        payment_tx: signed.payment_tx.clone(),
        shares_asset_optin_tx: signed.shares_asset_optin_tx.clone(),
        shares_xfer_tx: signed.shares_xfer_tx.clone(),
    })
}
