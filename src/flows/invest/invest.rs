use super::model::{InvestResult, InvestSigned, InvestToSign};
use crate::{
    flows::create_project::{
        model::Project, share_amount::ShareAmount, storage::load_project::ProjectId,
    },
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{
        builder::CallApplication, contract_account::ContractAccount, tx_group::TxGroup,
        AcceptAsset, Pay, Transaction, TransferAsset, TxnBuilder,
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

    let total_price = share_price.0.checked_mul(share_amount.0).ok_or(anyhow!(
        "Share price: {share_price} multiplied by share amount: {share_amount} caused an overflow."
    ))?;

    let mut central_app_investor_setup_tx = central_app_investor_setup_tx(
        &params,
        central_app_id,
        shares_asset_id,
        *investor,
        project_id,
    )?;

    let mut pay_price_tx = TxnBuilder::with(
        params.clone(),
        TransferAsset::new(
            *investor,
            funds_asset_id.0,
            total_price,
            *project.central_escrow.address(),
        )
        .build(),
    )
    .build();

    // TODO: review including this payment in send_algos_tx (to not have to pay a new fee? or can the fee here actually be 0, since group?: research)
    // note that a reason to _not_ include it is to show it separately to the user, when signing. It can help with clarity (review).
    let mut pay_escrow_fee_tx = TxnBuilder::with(
        params.clone(),
        Pay::new(
            *investor,
            *project.invest_escrow.address(),
            params.fee.max(params.min_fee),
        )
        .build(), // shares xfer
    )
    .build();

    let mut shares_optin_tx = TxnBuilder::with(
        params.clone(),
        AcceptAsset::new(*investor, project.shares_asset_id).build(),
    )
    .build();

    let mut receive_shares_asset_tx = TxnBuilder::with(
        params,
        TransferAsset::new(
            *project.invest_escrow.address(),
            project.shares_asset_id,
            share_amount.0,
            *locking_escrow.address(),
        )
        .build(),
    )
    .build();

    let txs_for_group = vec![
        &mut central_app_investor_setup_tx,
        &mut pay_price_tx,
        &mut shares_optin_tx,
        &mut receive_shares_asset_tx,
        &mut pay_escrow_fee_tx,
    ];
    TxGroup::assign_group_id(txs_for_group)?;

    let receive_shares_asset_signed_tx = project
        .invest_escrow
        .sign(&receive_shares_asset_tx, vec![])?;

    Ok(InvestToSign {
        project: project.to_owned(),
        central_app_setup_tx: central_app_investor_setup_tx,
        payment_tx: pay_price_tx,
        shares_asset_optin_tx: shares_optin_tx,
        pay_escrow_fee_tx,
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
        params.to_owned(),
        CallApplication::new(investor, app_id)
            .foreign_assets(vec![shares_asset_id])
            .app_arguments(vec![project_id.bytes().to_vec()])
            .build(),
    )
    .build();
    Ok(tx)
}

pub async fn submit_invest(algod: &Algod, signed: &InvestSigned) -> Result<InvestResult> {
    let txs = vec![
        signed.central_app_setup_tx.clone(),
        signed.payment_tx.clone(),
        signed.shares_asset_optin_tx.clone(),
        signed.shares_xfer_tx.clone(),
        signed.pay_escrow_fee_tx.clone(),
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
        pay_escrow_fee_tx: signed.pay_escrow_fee_tx.clone(),
        shares_xfer_tx: signed.shares_xfer_tx.clone(),
    })
}
