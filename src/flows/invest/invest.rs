use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CallApplication, tx_group::TxGroup, AcceptAsset, Pay,
        Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;

use crate::flows::create_project::model::Project;

use super::model::{InvestResult, InvestSigned, InvestToSign};

// TODO no constant
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

/// Requires investor to opt in to the app first,
/// we can't do it here: setting local state errors if during opt-in
#[allow(clippy::too_many_arguments)]
pub async fn invest_txs(
    algod: &Algod,
    project: &Project,
    investor: &Address,
    staking_escrow: &ContractAccount,
    central_app_id: u64,
    shares_asset_id: u64,
    asset_count: u64,
    asset_price: MicroAlgos,
) -> Result<InvestToSign> {
    log::debug!("Investing in project: {:?}", project);

    let params = algod.suggested_transaction_params().await?;

    let mut central_app_investor_setup_tx =
        central_app_investor_setup_tx(&params, central_app_id, shares_asset_id, *investor)?;

    // TODO why is this sending the algos to the invest escrow instead of to the central? why not caught by tests yet?
    // should be most likely the central as that's where we withdraw funds from
    let mut send_algos_tx = TxnBuilder::with(
        params.clone(),
        Pay::new(
            *investor,
            project.invest_escrow.address,
            asset_price * asset_count,
        )
        .build(),
    )
    .build();

    // TODO: review including this payment in send_algos_tx (to not have to pay a new fee? or can the fee here actually be 0, since group?: research)
    // note that a reason to _not_ include it is to show it separately to the user, when signing. It can help with clarity (review).
    let mut pay_escrow_fee_tx = TxnBuilder::with(
        params.clone(),
        Pay::new(*investor, project.invest_escrow.address, FIXED_FEE).build(), // shares xfer
    )
    .build();

    let mut shares_optin_tx = TxnBuilder::with(
        params.clone(),
        AcceptAsset::new(*investor, project.shares_asset_id).build(),
    )
    .build();

    let mut receive_shares_asset_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        TransferAsset::new(
            project.invest_escrow.address,
            project.shares_asset_id,
            asset_count,
            staking_escrow.address,
        )
        .build(),
    )
    .build();

    let txs_for_group = vec![
        &mut central_app_investor_setup_tx,
        &mut send_algos_tx,
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
        payment_tx: send_algos_tx,
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
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(investor, app_id)
            .foreign_assets(vec![shares_asset_id])
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
        tx_id: res.tx_id,
        project: signed.project.clone(),
        central_app_investor_setup_tx: signed.central_app_setup_tx.clone(),
        payment_tx: signed.payment_tx.clone(),
        shares_asset_optin_tx: signed.shares_asset_optin_tx.clone(),
        pay_escrow_fee_tx: signed.pay_escrow_fee_tx.clone(),
        shares_xfer_tx: signed.shares_xfer_tx.clone(),
    })
}
