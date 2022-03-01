use super::model::{InvestResult, InvestSigned, InvestToSign};
use crate::{
    algo_helpers::calculate_total_fee,
    flows::create_project::{
        model::Project, share_amount::ShareAmount, storage::load_project::ProjectId,
    },
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{
        builder::{CallApplication, TxnFee},
        contract_account::ContractAccount,
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
    project: &Project,
    investor: &Address,
    locking_escrow: &ContractAccount,
    central_app_id: u64,
    shares_asset_id: u64,
    share_amount: ShareAmount,
    funds_asset_id: FundsAssetId,
    share_price: FundsAmount,
    project_id: &ProjectId,
) -> Result<InvestToSign> {
    log::debug!("Investing in project: {:?}", project);

    let params = algod.suggested_transaction_params().await?;

    let total_price = share_price
        .val()
        .checked_mul(share_amount.val())
        .ok_or(anyhow!(
        "Share price: {share_price} multiplied by share amount: {share_amount} caused an overflow."
    ))?;

    let central_app_investor_setup_tx = &mut central_app_investor_setup_tx(
        &params,
        central_app_id,
        shares_asset_id,
        *investor,
        project_id,
    )?;

    let pay_price_tx = &mut TxnBuilder::with(
        &params,
        TransferAsset::new(
            *investor,
            funds_asset_id.0,
            total_price,
            *project.central_escrow.address(),
        )
        .build(),
    )
    .build()?;

    let shares_optin_tx = &mut TxnBuilder::with(
        &params,
        AcceptAsset::new(*investor, project.shares_asset_id).build(),
    )
    .build()?;

    let receive_shares_asset_tx = &mut TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *project.invest_escrow.address(),
            project.shares_asset_id,
            share_amount.val(),
            *locking_escrow.address(),
        )
        .build(),
    )
    .build()?;

    central_app_investor_setup_tx.fee = calculate_total_fee(
        &params,
        &[central_app_investor_setup_tx, receive_shares_asset_tx],
    )?;
    TxGroup::assign_group_id(vec![
        central_app_investor_setup_tx,
        pay_price_tx,
        shares_optin_tx,
        receive_shares_asset_tx,
    ])?;

    let receive_shares_asset_signed_tx = project
        .invest_escrow
        .sign(&receive_shares_asset_tx, vec![])?;

    Ok(InvestToSign {
        project: project.to_owned(),
        central_app_setup_tx: central_app_investor_setup_tx.clone(),
        payment_tx: pay_price_tx.clone(),
        shares_asset_optin_tx: shares_optin_tx.clone(),
        shares_xfer_tx: receive_shares_asset_signed_tx,
    })
}

pub fn central_app_investor_setup_tx(
    params: &SuggestedTransactionParams,
    app_id: u64,
    shares_asset_id: u64,
    investor: Address,
    project_id: &ProjectId,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        &params,
        CallApplication::new(investor, app_id)
            .foreign_assets(vec![shares_asset_id])
            .app_arguments(vec![project_id.bytes().to_vec()])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_invest(algod: &Algod, signed: &InvestSigned) -> Result<InvestResult> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.central_app_setup_tx.clone(),
        signed.payment_tx.clone(),
        signed.shares_asset_optin_tx.clone(),
        signed.shares_xfer_tx.clone(),
    ];

    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "investing_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    Ok(InvestResult {
        tx_id: res.tx_id.parse()?,
        project: signed.project.clone(),
        central_app_investor_setup_tx: signed.central_app_setup_tx.clone(),
        payment_tx: signed.payment_tx.clone(),
        shares_asset_optin_tx: signed.shares_asset_optin_tx.clone(),
        shares_xfer_tx: signed.shares_xfer_tx.clone(),
    })
}
